#!/usr/bin/env python3
"""P11-T02: dependency SBOM generation + CVE gate for Isolith releases.

Emit SPDX / CycloneDX BOMs for the Rust crate dependency tree (from the
committed Cargo.lock) and for the container images the release embeds (base
images declared in the repo's Dockerfiles), then scan the lockfile against
the RustSec advisory database (source of truth for `cargo audit`) and GATE
the release on no unfixed critical/high vulnerabilities unless explicitly
waived in workstreams/security/waivers.toml.

Design notes (clean-room invariant: scanning is lawful inventory/analysis,
never copying):

  * Lockfile-driven. The SBOM is derived from the committed Cargo.lock (the
    exact thing pinned into every release build), not from a re-resolution,
    so the SBOM matches what got built - reproducible output (P11-T06).
  * Advisory-db as source of truth. clone of rustsec/advisory-db (the same
    data cargo-audit consumes) is parsed locally; severity comes from the
    advisory's declared severity or a CVSS v3.1 base score computed from the
    advisory CVSS vector. No third-party compiler install in CI.
  * Deterministic artifacts. CycloneDX serialNumbers and SPDX namespaces are
    derived from a content hash so re-running on the same ref emits
    byte-identical BOMs.
  * Gate policy is explicit and in-repo. Unfixed critical/high findings
    block (exit 1) unless they carry a committed waiver in workstreams/security/waivers.toml.
  * CI-safe degraded legs. The authoritative lockfile gate always runs.
    An optional OSV cross-check and a trivy pass over built images are
    reported when their prerequisites are reachable; if not, their leg is
    classified SKIP (not FAIL) - the same honest classification the P8/P9/
    P10/P12 gates use for legs that need a live LAN/docker.

Exit code contract: 0 iff the gate reports no blocking (unfixed, unrevoked,
critical/high) finding; 1 otherwise (release must not proceed).

Usage:
  python3 -m oep_tooling.sbom_cve gate --repo DIR [--ref REF] [--out DIR]
        [--advisory-db DIR] [--waivers PATH] [--with-osv] [--offline]
        [--block-severities critical,high]
  python3 -m oep_tooling.sbom_cve sbom  --repo DIR [--ref REF] [--out DIR]
  python3 -m oep_tooling.sbom_cve scan  --repo DIR [--advisory-db DIR] ...
  python3 -m oep_tooling.sbom_cve trivy-image --image TAG --out DIR   # host proof leg
"""
from __future__ import annotations

import argparse
import datetime as _dt
import hashlib
import json
import os
import re
import subprocess
import sys
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

try:  # py>=3.11
    import tomllib
except Exception:  # pragma: no cover  (CI image is bookworm -> 3.11)
    tomllib = None

TOOL_NAME = "isolith-oep-sbom"
TOOL_VERSION = "0.1.0"
PROJECT_NAME = "isolith-oep"          # SPDX-neutral workspace component name
ADVISORY_DB_DEFAULT = "https://github.com/rustsec/advisory-db.git"
SEVERITY_ORDER = ["critical", "high", "moderate", "low", "informational"]

# --------------------------------------------------------------------------
# Rust semver (stdlib-only, the subset used by advisory version ranges)
# --------------------------------------------------------------------------

_PRE_RE = re.compile(r"^(?P<core>\d+\.\d+\.\d+)(?:-(?P<pre>[0-9A-Za-z.-]+))?(?:\+(?P<build>[0-9A-Za-z.-]+))?$")


class Version:
    """Minimal Rust semver-comparable version (numeric core + pre-release)."""

    __slots__ = ("major", "minor", "patch", "pre", "text")

    def __init__(self, text: str):
        m = _PRE_RE.match(text.strip())
        if not m:
            raise ValueError(f"not a semver: {text!r}")
        self.major, self.minor, self.patch = (int(x) for x in m.group("core").split("."))
        self.pre = m.group("pre")
        self.text = text.strip()

    def _pre_key(self):
        # Rust ordering: a version with a pre-release sorts BELOW the same
        # version with no pre-release (so "1.0.2-alpha" < "1.0.2").
        if self.pre is None:
            return (1, ())
        parts = []
        for tok in self.pre.split("."):
            if tok.isdigit():
                parts.append((0, int(tok), ""))
            else:
                parts.append((1, 0, tok))
        return (0, tuple(parts))

    def __lt__(self, other):
        a = (self.major, self.minor, self.patch)
        b = (other.major, other.minor, other.patch)
        if a != b:
            return a < b
        return self._pre_key() < other._pre_key()

    def __eq__(self, other):
        return (self.major, self.minor, self.patch, self._pre_key()) == (
            other.major, other.minor, other.patch, other._pre_key())

    def __le__(self, other):
        return self < other or self == other

    def __gt__(self, other):
        return not (self <= other)

    def __ge__(self, other):
        return not (self < other)

    def __repr__(self):  # pragma: no cover
        return f"Version({self.text!r})"


def _parse_exact(expr: str) -> Version:
    return Version(expr)


def _match_single_constraint(comp, op, version: Version) -> bool:
    if op in (">=",):
        return version >= _parse_exact(comp)
    if op in (">",):
        return version > _parse_exact(comp)
    if op in ("<",):
        return version < _parse_exact(comp)
    if op in ("<=",):
        return version <= _parse_exact(comp)
    if op in ("=", "=="):
        return version == _parse_exact(comp)
    raise ValueError(f"unsupported comparator {op!r}")


def _caret_matches(expr: str, version: Version) -> bool:
    # ^x.y.z : >= x.y.z and < next breaking range
    v = _parse_exact(expr)
    lo = (v.major, v.minor, v.patch)
    if v.major > 0:
        hi = (v.major + 1, 0, 0)
    elif v.minor > 0:
        hi = (0, v.minor + 1, 0)
    else:
        hi = (0, 0, v.patch + 1)
    cur = (version.major, version.minor, version.patch)
    return cur >= lo and cur < hi


def _tilde_matches(expr: str, version: Version) -> bool:
    # ~x.y.z : >= x.y.z and < x.(y+1).0  (caret-like on minor)
    v = _parse_exact(expr)
    cur = (version.major, version.minor, version.patch)
    return cur >= (v.major, v.minor, v.patch) and cur < (v.major, v.minor + 1, 0)


def _as_range(expr: str):
    """Normalise a bare '1.2.3' into a caret range tuple list."""
    v = _parse_exact(expr)
    if v.major > 0:
        return [(">=", f"{v.major}.{v.minor}.{v.patch}-0"), ("<", f"{v.major + 1}.0.0-0")]
    if v.minor > 0:
        return [(">=", f"{v.major}.{v.minor}.{v.patch}-0"), ("<", f"0.{v.minor + 1}.0-0")]
    return [(">=", f"{v.major}.{v.minor}.{v.patch}-0"), ("<", f"0.0.{v.patch + 1}-0")]


def version_req_matches(req: str, version_text: str) -> bool:
    """Does ``version_text`` satisfy a Rust advisory version requirement?

    Handles the req forms used across the RustSec advisory database: exact
    ("=1.2.3"), comparator ranges (">=1.2.3, <2.0.0"), caret ("^1.2.3"),
    tilde ("~1.2.3"), a bare "1.2.3" (caret semantics) and "*" (any).
    """
    version = Version(version_text)
    if req.strip() in ("*", ""):
        return True
    for clause in req.split(","):
        clause = clause.strip()
        if not clause:
            continue
        if clause == "*":
            continue
        m = re.match(r"^(>=|<=|==|=|>|<|~|\^)?\s*(.+)$", clause)
        op = m.group(1) or ""
        comp = m.group(2).strip()
        if op == "":
            for o2, c2 in _as_range(comp):
                if not _match_single_constraint(c2, o2, version):
                    return False
        elif op == "~":
            if not _tilde_matches(comp, version):
                return False
        elif op == "^":
            if not _caret_matches(comp, version):
                return False
        else:
            if not _match_single_constraint(comp, op, version):
                return False
    return True


# --------------------------------------------------------------------------
# CVSS v3.1 base score (only used when an advisory omits declared severity)
# --------------------------------------------------------------------------

_CVSS3_METRICS = {
    "AV": {"N": 0.85, "A": 0.62, "L": 0.55, "P": 0.2},
    "AC": {"L": 0.77, "H": 0.44},
    "UI": {"N": 0.85, "R": 0.62},
    "PR": {  # depends on scope + UI (mitigating)
        "U": {"N": 0.85, "L": 0.62, "H": 0.27, "R-N": 0.85, "R-L": 0.68, "R-H": 0.5},
        "C": {"N": 0.85, "L": 0.68, "H": 0.5, "R-N": 0.85, "R-L": 0.68, "R-H": 0.5},
    },
    "C": {"H": 0.56, "L": 0.22, "N": 0.0},
    "I": {"H": 0.56, "L": 0.22, "N": 0.0},
    "A": {"H": 0.56, "L": 0.22, "N": 0.0},
}


def cvss31_base_score(vector: str):
    """Return (score, severity) for a CVSS v3.1 vector string, else (None,None)."""
    if not vector:
        return None, None
    parts = {}
    for chunk in vector.split("/"):
        if ":" not in chunk:
            continue
        k, v = chunk.split(":", 1)
        parts[k] = v.upper()
    try:
        av = _CVSS3_METRICS["AV"][parts.get("AV", "N")]
        ac = _CVSS3_METRICS["AC"][parts.get("AC", "L")]
        ui = _CVSS3_METRICS["UI"][parts.get("UI", "N")]
        scope = parts.get("S", "U")
        c = _CVSS3_METRICS["C"][parts.get("C", "N")]
        i = _CVSS3_METRICS["I"][parts.get("I", "N")]
        a = _CVSS3_METRICS["A"][parts.get("A", "N")]
        pr_value = parts.get("PR", "N")
        pr_key = f"R-{pr_value}" if ui == "R" else pr_value
        pr = _CVSS3_METRICS["PR"][scope][pr_key]
    except (KeyError, TypeError):
        return None, None
    iss = 1 - ((1 - c) * (1 - i) * (1 - a))
    if scope == "C":
        impact = 7.52 * (iss - 0.029) - 3.25 * (iss - 0.02) ** 15
    else:
        impact = 6.42 * iss
    exploit = 8.22 * av * ac * pr * ui
    if scope == "C":
        base = min(impact + exploit, 10.0)
    else:
        base = min(1.08 * (impact + exploit), 10.0)
    base = max(0.0, base)
    base = 10 if base >= 10 else base
    import math
    base = math.ceil(base * 10) / 10
    if base >= 9.0:
        sev = "critical"
    elif base >= 7.0:
        sev = "high"
    elif base >= 4.0:
        sev = "moderate"
    elif base > 0.0:
        sev = "low"
    else:
        sev = "informational"
    return base, sev


# --------------------------------------------------------------------------
# Cargo.lock parsing
# --------------------------------------------------------------------------

def load_cargo_lock(path: Path) -> list[dict]:
    raw = path.read_text()
    if tomllib is None:
        raise RuntimeError("tomllib unavailable (python >= 3.11 required)")
    data = tomllib.loads(raw)
    return data.get("package", [])


def purl_for_cargo(name: str, version: str) -> str:
    # pkg:cargo/<name>@<version>  (percent-encode anything a purl disallows)
    return "pkg:cargo/" + urllib.parse.quote(name, safe=".-_") + "@" + version


# --------------------------------------------------------------------------
# RustSec advisory-db handling
# --------------------------------------------------------------------------

def ensure_advisory_db(db_dir: Path, offline: bool = False, refresh: bool = False) -> Path:
    """Ensure a RustSec advisory-db is available at db_dir.

    Accepts either a git clone (and then optionally refreshes it) or a
    pre-populated non-git tree (e.g. the hermetic test fixture DB). When
    neither exists a shallow git clone is made; that path requires network
    unless ``offline`` is set."""
    if db_dir.exists() and (db_dir / "crates").exists():
        if refresh and (db_dir / ".git").exists() and not offline:
            subprocess.run(
                ["git", "-C", str(db_dir), "fetch", "-q", "--depth", "1", "origin", "main"],
                check=False, capture_output=True,
            )
            subprocess.run(
                ["git", "-C", str(db_dir), "reset", "-q", "--hard", "origin/main"],
                check=False, capture_output=True,
            )
        return db_dir
    if offline:
        raise SystemExit(f"advisory-db not present and --offline set: {db_dir}")
    db_dir.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        ["git", "clone", "-q", "--depth", "1", ADVISORY_DB_DEFAULT, str(db_dir)],
        check=True, capture_output=True,
    )
    return db_dir


def iter_advisories(db_dir: Path):
    """Yield (advisory_dict, doc, path) for every RUSTSEC entry in the DB.

    Current rustsec/advisory-db stores each advisory as ``crates/<pkg>/
    RUSTSEC-YYYY-NNNN.md`` with a fenced TOML frontmatter block; older layouts
    were plain ``.toml`` files. Both are handled."""
    for path in sorted((db_dir / "crates").glob("*/RUSTSEC-*")):
        if path.suffix not in (".toml", ".md"):
            continue
        try:
            text = path.read_text()
            if path.suffix == ".md":
                m = re.search(r"```toml\n(.*?)```", text, re.S)
                if not m:
                    continue
                doc = tomllib.loads(m.group(1))
                body = text[m.end():].strip()
                if body and not doc.get("advisory", {}).get("summary"):
                    # first real sentence/paragraph of the markdown prose
                    first = [ln.strip() for ln in body.splitlines() if ln.strip()]
                    if first:
                        doc["advisory"]["summary"] = first[0].strip("#").strip()[:300]
            else:
                doc = tomllib.loads(text)
        except Exception:
            continue
        adv = doc.get("advisory")
        if adv:
            yield adv, doc, path


_SEVERITY_RANK = {s: i for i, s in enumerate(SEVERITY_ORDER)}


def _advisory_severity(adv) -> str:
    sev = adv.get("severity")
    if sev and sev in _SEVERITY_RANK:
        return sev
    if adv.get("informational"):
        return "informational"
    vector = adv.get("cvss") or ""
    _score, sev2 = cvss31_base_score(vector)
    return sev2 or "informational"


def _advisory_cvss(adv):
    vector = adv.get("cvss") or ""
    return cvss31_base_score(vector)


def _versions_section(adv, doc):
    return doc.get("versions", {}) or {}


def advisory_affects(adv, doc, version_text: str, package: str) -> "str | None":
    """Return the affected-state reason string if ``package@version`` is hit by
    the advisory, else None. Mirrors cargo-audit matching semantics: affected
    unless the version is covered by patched/unaffected, and if an explicit
    ``affected`` list exists the version must be inside it too."""
    if adv.get("package") != package:
        return None
    vs = _versions_section(adv, doc)
    affected = vs.get("affected") or []
    patched = vs.get("patched") or []
    unaffected = vs.get("unaffected") or []
    try:
        if affected and not any(version_req_matches(r, version_text) for r in affected):
            return None
        if any(version_req_matches(r, version_text) for r in patched):
            return None
        if any(version_req_matches(r, version_text) for r in unaffected):
            return None
    except ValueError:
        return None
    return "unfixed"


def scan_lockfile(cargo_pkgs: list[dict], db_dir: Path):
    """Return findings: [{id, package, version, aliases, severity, cvss, ...}]."""
    findings = []
    by_name = {}
    for p in cargo_pkgs:
        by_name.setdefault(p["name"], []).append(p["version"])
    for adv, doc, path in iter_advisories(db_dir):
        pkg = adv.get("package")
        if pkg not in by_name:
            continue
        for ver in by_name[pkg]:
            if advisory_affects(adv, doc, ver, pkg):
                score, sev = _advisory_cvss(adv)
                findings.append({
                    "id": adv.get("id", path.stem),
                    "package": pkg,
                    "version": ver,
                    "summary": adv.get("summary", ""),
                    "severity": _advisory_severity(adv),
                    "cvss": adv.get("cvss", ""),
                    "cvss_score": score,
                    "aliases": adv.get("aliases", []),
                    "url": adv.get("url", ""),
                    "date": str(adv.get("date", "")),
                    "state": "unfixed",
                })
    findings.sort(key=lambda f: (_SEVERITY_RANK.get(f["severity"], 99), f["id"], f["package"], f["version"]))
    return findings


def trivy_findings_from_json(raw: dict) -> list[dict]:
    """Normalise a `trivy image --format json` report into gate findings.

    Findings are keyed by a stable GROUP id ``image:<distro>:<pkg>`` so a
    committed waiver can cover a package (and all its open CVEs) without
    churning the waiver file on every DB refresh; the individual CVE ids stay
    visible in ``aliases``/``title`` for the human record."""
    out = []
    for result in raw.get("Results", []):
        target = result.get("Target", "")
        # strip the trailing " (debian 12.15)" OS descriptor and the image
        # tag/digest so group ids stay stable across base/point releases
        target = re.split(r"\s+\(", target)[0]
        target = target.split(":")[0].split("@")[0]
        for v in result.get("Vulnerabilities", []):
            pkg = v.get("PkgName", "")
            if not pkg:
                continue
            sev = str(v.get("Severity", "unknown")).lower()
            sev = {"medium": "moderate", "unknown": "informational"}.get(sev, sev)
            if sev not in SEVERITY_ORDER:
                sev = "informational"
            out.append({
                "id": f"image:{target}:{pkg}",
                "package": pkg,
                "version": v.get("InstalledVersion", ""),
                "summary": (v.get("Title") or "")[:300],
                "severity": sev,
                "cvss": "",
                "cvss_score": None,
                "aliases": [v.get("VulnerabilityID", "")],
                "url": v.get("PrimaryURL", ""),
                "date": "",
                "state": "trivy-image",
                "target": target,
            })
    return out


def load_trivy_findings(path: Path) -> "tuple[list[dict], str]":
    if not path.exists():
        return [], "file not present"
    try:
        raw = json.loads(path.read_text())
        # raw trivy JSON, or a pre-normalized findings list
        if isinstance(raw, list):
            return raw, "ok"
        return trivy_findings_from_json(raw), "ok"
    except Exception as e:
        return [], f"parse error ({type(e).__name__})"


# --------------------------------------------------------------------------
# OSV cross-check (informational leg; never the gate's authorizer)
# --------------------------------------------------------------------------

OSV_BATCH = "https://api.osv.dev/v1/querybatch"


def osv_crosscheck(cargo_pkgs: list[dict], timeout: int = 25):
    """Query OSV for every registry crate; return list of OSV records. Leg is
    classified SKIP when the endpoint is unreachable (gate unaffected)."""
    registry = [p for p in cargo_pkgs if str(p.get("source", "")).startswith("registry+")]
    if not registry:
        return [], "no registry packages"
    queries = [{"package": {"ecosystem": "crates.io", "name": p["name"]}, "version": p["version"]} for p in registry]
    payload = {"queries": queries}
    req = urllib.request.Request(
        OSV_BATCH, data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            resp = json.load(r)
    except Exception as e:  # network / rate-limit / block -> SKIP leg
        return [], f"unreachable ({type(e).__name__})"
    records = []
    results = resp.get("results", [])
    for i, res in enumerate(results):
        for v in res.get("vulns", []):
            sev = "unknown"
            for s in v.get("severity", []):
                sc, sev = cvss31_base_score(s.get("score", ""))
                if sc is not None:
                    break
            records.append({
                "id": v.get("id"),
                "package": registry[i]["name"],
                "version": registry[i]["version"],
                "summary": v.get("summary", ""),
                "severity": sev,
                "aliases": v.get("aliases", []),
            })
    return records, "ok"


# --------------------------------------------------------------------------
# SBOM emission (CycloneDX 1.5 + SPDX 2.3)
# --------------------------------------------------------------------------

def _content_digest(*parts) -> str:
    h = hashlib.sha256()
    for p in parts:
        h.update(str(p).encode())
    return h.hexdigest()


def _serial(seed: str) -> str:
    return "urn:uuid:" + str(uuid.UUID(hex=_content_digest(seed)[:32]))


def now_utc() -> str:
    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def emit_rust_cyclonedx(lockfile_txt: str, cargo_pkgs: list[dict], ref: str) -> dict:
    components = []
    for p in sorted(cargo_pkgs, key=lambda q: (q["name"], q["version"])):
        src = str(p.get("source", ""))
        if not src.startswith("registry+") or "crates.io" not in src:
            # path / git deps are still part of the SBOM; keep them listed so
            # the BOM is the complete dependency inventory of the build.
            is_registry = False
        else:
            is_registry = True
        comp = {
            "type": "library",
            "bom-ref": purl_for_cargo(p["name"], p["version"]),
            "name": p["name"],
            "version": p["version"],
            "purl": purl_for_cargo(p["name"], p["version"]),
            "properties": [{"name": "isolith:registry", "value": "true" if is_registry else "false"}],
        }
        if p.get("checksum"):
            comp["hashes"] = [{"alg": "SHA-256", "content": p["checksum"]}]
        components.append(comp)
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "serialNumber": _serial(f"cyclopdx-rust-{ref}-{lockfile_txt}"),
        "metadata": {
            "timestamp": now_utc(),
            "component": {
                "type": "application",
                "bom-ref": purl_for_cargo(PROJECT_NAME, ref),
                "name": PROJECT_NAME,
                "version": ref,
                "purl": purl_for_cargo(PROJECT_NAME, ref),
            },
            "tools": [{"vendor": "Isolith", "name": TOOL_NAME, "version": TOOL_VERSION}],
        },
        "components": components,
    }


def emit_rust_spdx(lockfile_txt: str, cargo_pkgs: list[dict], ref: str) -> dict:
    packages = []
    rels = []
    for i, p in enumerate(sorted(cargo_pkgs, key=lambda q: (q["name"], q["version"])), start=1):
        sid = f"SPDXRef-Package-{i}"
        pkg = {
            "SPDXID": sid,
            "name": p["name"],
            "versionInfo": p["version"],
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": "NOASSERTION",
            "copyrightText": "NOASSERTION",
            "externalRefs": [{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": purl_for_cargo(p["name"], p["version"]),
            }],
        }
        packages.append(pkg)
        rels.append({"spdxElementId": "SPDXRef-DOCUMENT", "relationshipType": "DESCRIBES", "relatedSpdxElement": sid})
    return {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"{PROJECT_NAME} rust SBOM {ref}",
        "documentNamespace": f"https://isolith.local/spdx/rust-{ref}-{_content_digest(lockfile_txt)[:16]}",
        "creationInfo": {
            "created": now_utc(),
            "creators": [f"Tool: {TOOL_NAME}-{TOOL_VERSION}", "Organization: Isolith"],
        },
        "packages": packages,
        "relationships": rels,
    }


def base_images_from_dockerfiles(repo: Path):
    """Distinct base images (FROM lines without build args) across Dockerfiles.

    Returns sorted [(image, [dockerfile paths])]. A FROM that references a
    build arg (e.g. ``FROM ${BASE}``) yields a synthetic entry so the SBOM is
    the full declared set; the resolved value is an operator decision recorded
    in the runbook, not guessed here."""
    hits = {}
    for df in sorted(repo.glob("**/Dockerfile*")):
        # Skip build/.git mounts and OTHER worktrees, but only when they sit
        # INSIDE the scan root (a repo checked out under .worktrees/ must still
        # see its own Dockerfiles). Relative-path check keeps this true whether
        # the scan root is a canonical checkout or a git worktree.
        rel = str(df.relative_to(repo))
        if "/target/" in rel or "/.git" in rel or "/.worktrees/" in rel:
            continue
        try:
            text = df.read_text()
        except Exception:
            continue
        for line in text.splitlines():
            s = line.strip()
            if not s.upper().startswith("FROM "):
                continue
            rest = s[5:].strip().split()
            if not rest:
                continue
            img = rest[0]
            if "${" in img or "$" in img:
                img = "<build-arg-resolved-at-build>"
            hits.setdefault(img, []).append(str(df))
    return [(img, sorted(set(paths))) for img, paths in sorted(hits.items())]


def emit_images_cyclonedx(repo: Path, ref: str) -> dict:
    bases = base_images_from_dockerfiles(repo)
    comps = []
    for img, paths in bases:
        name, _, tag = img.rpartition(":")
        tag = tag or "latest"
        purl = "pkg:docker/" + urllib.parse.quote(name, safe=".-_/") + ("" if tag in ("latest", "<build-arg-resolved-at-build>") else "@" + tag)
        comps.append({
            "type": "container",
            "bom-ref": purl,
            "name": img,
            "version": tag,
            "externalReferences": [],
            "properties": [{"name": "isolith:dockerfile", "value": ";".join(paths)}],
            "purl": purl,
        })
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "serialNumber": _serial(f"cyclopdx-images-{ref}-{repo}"),
        "metadata": {
            "timestamp": now_utc(),
            "component": {"type": "application", "bom-ref": f"{PROJECT_NAME}@{ref}", "name": PROJECT_NAME, "version": ref},
            "tools": [{"vendor": "Isolith", "name": TOOL_NAME, "version": TOOL_VERSION}],
        },
        "components": comps,
    }


# --------------------------------------------------------------------------
# Waivers + gate policy
# --------------------------------------------------------------------------

def load_waivers(path: Path) -> dict:
    if not path.exists():
        return {"version": "1", "policy": {}, "waiver": []}
    doc = tomllib.loads(path.read_text()) if tomllib else {}
    return doc


def finding_waived(waivers: dict, f: dict) -> "dict | None":
    for w in waivers.get("waiver", []):
        if w.get("revoked", False):
            continue
        if w.get("id", "").lower() != f["id"].lower():
            continue
        if w.get("package") and w["package"] != f["package"]:
            continue
        if w.get("affected_version") and w["affected_version"] != f["version"]:
            continue
        return w
    return None


def run_gate(findings, waivers: dict, block_severities) -> tuple[list, int]:
    """Return (blockers, exit_code). exit 0 iff no unfixed, unrevoked finding
    at a blocking severity; blocked entries carry waiver metadata."""
    blockers = []
    for f in findings:
        row = dict(f)
        row["waived"] = bool(finding_waived(waivers, f))
        if f["severity"] in block_severities and not row["waived"]:
            blockers.append(row)
    return blockers, (0 if not blockers else 1)


# --------------------------------------------------------------------------
# Report emission
# --------------------------------------------------------------------------

def fmt_md(report: dict, ref: str) -> str:
    lines = []
    lines.append(f"# Isolith SBOM + CVE scan report — {ref}")
    lines.append("")
    lines.append(f"- generated by {TOOL_NAME} {TOOL_VERSION}")
    lines.append(f"- timestamp: {report.get('timestamp', '')}")
    lines.append(f"- gate decision: **{report.get('gate', 'unknown')}**")
    lines.append(f"- scopes: {', '.join(report.get('scopes', []))}")
    lines.append("")
    lines.append("## Findings")
    lines.append("")
    rows = report.get("index", [])
    lines.append(f"| # | id | package | version | severity | state |")
    lines.append("|---|---|---|---|---|---|")
    for r in rows:
        state = "WAIVED" if r.get("waived") else ("blocked" if r in [b for b in report.get("blocking", [])] and not r.get("waived") else r.get("state", "report"))
        lines.append(f"| {r['id']} | {r['package']} | {r['version']} | {r['severity']} | {state} |")
    lines.append("")
    counts = report.get("counts", {})
    lines.append(f"total={counts.get('total', 0)} critical={counts.get('critical', 0)} "
                 f"high={counts.get('high', 0)} moderate={counts.get('moderate', 0)} "
                 f"low={counts.get('low', 0)} informational={counts.get('informational', 0)} "
                 f"waived={counts.get('waived', 0)} blockers={counts.get('blockers', 0)}")
    for leg in report.get("legs", []):
        lines.append("")
        lines.append(f"## Leg: {leg['name']} ({leg['state']})")
        if leg.get("note"):
            lines.append("")
            lines.append(leg["note"])
    return "\n".join(lines) + "\n"


def build_report(ref, findings, waivers, block_severities, legs, sbom_files, gate_exit, waivers_path=None) -> dict:
    blockers, _ = run_gate(findings, waivers, block_severities)
    counts = {"total": len(findings), "critical": 0, "high": 0,
              "moderate": 0, "low": 0, "informational": 0, "waived": 0, "blockers": len(blockers)}
    index = []
    for f in sorted(findings, key=lambda x: (_SEVERITY_RANK.get(x["severity"], 99), x["id"])):
        w = finding_waived(waivers, f)
        row = dict(f)
        row["waived"] = bool(w)
        if w:
            row["waiver_reason"] = w.get("reason", "")
        row["blocking"] = f["severity"] in block_severities and not row["waived"]
        counts[f["severity"]] = counts.get(f["severity"], 0) + 1
        if row["waived"]:
            counts["waived"] += 1
        index.append(row)
    return {
        "ref": ref,
        "timestamp": now_utc(),
        "tool": {"name": TOOL_NAME, "version": TOOL_VERSION},
        "gate": "PASS" if gate_exit == 0 else "FAIL",
        "block_severities": block_severities,
        "counts": counts,
        "index": index,
        "blocking": [dict(b) for b in blockers],
        "legs": legs,
        "sbom_artifacts": sbom_files,
        "waivers_file": str(waivers_path) if waivers_path else "workstreams/security/waivers.toml",
    }


def waivers_path_of(repo):
    if repo is None:
        return Path("workstreams/security/waivers.toml")
    return Path(repo) / "workstreams/security" / "waivers.toml"


def _resolve_waivers(args, repo: Path) -> Path:
    if getattr(args, "waivers", None):
        return Path(args.waivers)
    return waivers_path_of(repo)


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------

def _parse_lockfile_and_pkgs(repo: Path):
    lock = repo / "Cargo.lock"
    if not lock.exists():
        raise SystemExit(f"Cargo.lock not found in {repo} - not a Rust workspace root?")
    return lock.read_text(), load_cargo_lock(lock)


def _resolve_ref(repo: Path, ref: str | None) -> str:
    if ref:
        return ref
    try:
        out = subprocess.run(["git", "-C", str(repo), "describe", "--tags", "--always", "--dirty"],
                             capture_output=True, text=True, timeout=10).stdout.strip()
        if out:
            return out
    except Exception:
        pass
    return "dev"


def _default_blocks():
    return ["critical", "high"]


def _out_dir(repo: Path, out: str | None, ref: str) -> Path:
    if out:
        d = Path(out)
    else:
        d = repo / "workstreams/security" / "artifacts" / (ref.split("-", 1)[0])
    d.mkdir(parents=True, exist_ok=True)
    return d


def cmd_sbom(args):
    repo = Path(args.repo).resolve()
    ref = _resolve_ref(repo, args.ref)
    outdir = _out_dir(repo, args.out, ref)
    lock_txt, pkgs = _parse_lockfile_and_pkgs(repo)
    cdx = emit_rust_cyclonedx(lock_txt, pkgs, ref)
    spdx = emit_rust_spdx(lock_txt, pkgs, ref)
    imgs = emit_images_cyclonedx(repo, ref)
    (outdir / "rust.cdx.json").write_text(json.dumps(cdx, indent=2))
    (outdir / "rust.spdx.json").write_text(json.dumps(spdx, indent=2))
    (outdir / "images.cdx.json").write_text(json.dumps(imgs, indent=2))
    print(f"SBOM written to {outdir}")
    print(f"  rust packages: {len(cdx['components'])}  images: {len(imgs['components'])}")
    for f_ in ("rust.cdx.json", "rust.spdx.json", "images.cdx.json"):
        print(f"  {f_}: {outdir / f_}")
    return 0


def cmd_scan(args):
    repo = Path(args.repo).resolve()
    ref = _resolve_ref(repo, args.ref)
    lock_txt, pkgs = _parse_lockfile_and_pkgs(repo)
    db = Path(args.advisory_db).resolve() if args.advisory_db else (repo / "workstreams/security" / ".advisory-db")
    ensure_advisory_db(db, offline=args.offline, refresh=args.refresh)
    findings = scan_lockfile(pkgs, db)
    outdir = _out_dir(repo, args.out, ref) if args.out or True else repo / "workstreams/security" / "artifacts" / ref
    waivers = load_waivers(_resolve_waivers(args, repo))
    blocks = (args.block_severities or ",".join(_default_blocks())).replace(" ", "").split(",")
    legs = [{"name": "rustsec-advisory-db", "state": "ok", "note": f"advisory db: {db}"}]
    if args.with_osv:
        records, state = osv_crosscheck(pkgs)
        legs.append({"name": "osv-crosscheck", "state": "ok" if state == "ok" else "SKIP", "note": state})
        for r in records:
            findings.append({**r, "state": "osv-crosscheck"})
    if getattr(args, "trivy_scan", None):
        tfindings, state = load_trivy_findings(Path(args.trivy_scan))
        if state == "ok":
            findings = findings + tfindings
            legs.append({"name": "trivy-image", "state": "ok",
                         "note": f"folded {len(tfindings)} findings from release-host trivy scan"})
        else:
            legs.append({"name": "trivy-image", "state": "SKIP", "note": f"trivy scan not folded: {state}"})
    _, gate_exit = run_gate(findings, waivers, blocks)
    wpath = _resolve_waivers(args, repo)
    report = build_report(ref, findings, waivers, blocks, legs,
                          ["rust.cdx.json", "rust.spdx.json", "images.cdx.json"], gate_exit, wpath)
    (outdir / "scan-report.json").write_text(json.dumps(report, indent=2))
    (outdir / "scan-report.md").write_text(fmt_md(report, ref))
    print(f"scan + report written to {outdir}")
    print(f"findings={report['counts']['total']} blockers={report['counts']['blockers']} gate={report['gate']}")
    return gate_exit


def cmd_gate(args):
    repo = Path(args.repo).resolve()
    ref = _resolve_ref(repo, args.ref)
    lock_txt, pkgs = _parse_lockfile_and_pkgs(repo)
    outdir = _out_dir(repo, args.out, ref)
    # 1) emit SBOMs
    cdx = emit_rust_cyclonedx(lock_txt, pkgs, ref)
    spdx = emit_rust_spdx(lock_txt, pkgs, ref)
    imgs = emit_images_cyclonedx(repo, ref)
    (outdir / "rust.cdx.json").write_text(json.dumps(cdx, indent=2))
    (outdir / "rust.spdx.json").write_text(json.dumps(spdx, indent=2))
    (outdir / "images.cdx.json").write_text(json.dumps(imgs, indent=2))
    # 2) scan
    db = Path(args.advisory_db).resolve() if args.advisory_db else (repo / "workstreams/security" / ".advisory-db")
    ensure_advisory_db(db, offline=args.offline, refresh=args.refresh)
    findings = scan_lockfile(pkgs, db)
    legs = [{"name": "rustsec-advisory-db", "state": "ok", "note": f"advisory db: {db}"}]
    if args.with_osv:
        records, state = osv_crosscheck(pkgs)
        legs.append({"name": "osv-crosscheck", "state": "ok" if state == "ok" else "SKIP", "note": state})
        for r in records:
            findings.append({**r, "state": "osv-crosscheck"})
    if args.with_trivy and args.trivy_image:
        legs.append({"name": "trivy-image", "state": "SKIP", "note": "run on the release host (docker + trivy); CI-safe skip"})
    if getattr(args, "trivy_scan", None):
        tfindings, state = load_trivy_findings(Path(args.trivy_scan))
        if state != "ok":
            legs.append({"name": "trivy-image", "state": "SKIP", "note": f"trivy scan not folded: {state}"})
        else:
            findings = findings + tfindings
            sev = {}
            for f in tfindings:
                sev[f["severity"]] = sev.get(f["severity"], 0) + 1
            legs.append({"name": "trivy-image", "state": "ok",
                         "note": f"folded {len(tfindings)} findings ({sev}); sourced from release-host trivy scan"})
    waivers = load_waivers(_resolve_waivers(args, repo))
    blocks = (args.block_severities or ",".join(_default_blocks())).replace(" ", "").split(",")
    _, gate_exit = run_gate(findings, waivers, blocks)
    wpath = _resolve_waivers(args, repo)
    report = build_report(ref, findings, waivers, blocks, legs,
                          [str(outdir / f) for f in ("rust.cdx.json", "rust.spdx.json", "images.cdx.json")], gate_exit, wpath)
    (outdir / "scan-report.json").write_text(json.dumps(report, indent=2))
    (outdir / "scan-report.md").write_text(fmt_md(report, ref))
    print(f"gate={report['gate']} findings={report['counts']['total']} "
          f"blockers={report['counts']['blockers']} ref={ref} out={outdir}")
    return gate_exit


def cmd_trivy_image(args):
    """Host-side proof leg: scan the real built image with trivy (when the
    binary is installed) and emit, under the out dir:
      trivy-image.cdx.json       CycloneDX SBOM of the image (components)
      trivy-image.json           raw `trivy image --format json` findings
      trivy-image-findings.json  findings normalised into gate schema
    The gate consumes the normalised findings via --trivy-scan so a critical/
    high OS CVE in the shipped image BLOCKS unless it carries a committed
    waiver (workstreams/security/waivers.toml). Exit 0 even when trivy is absent: the leg is
    then SKIP and the lockfile gate stays the authorizer."""
    outdir = Path(args.out)
    outdir.mkdir(parents=True, exist_ok=True)
    image = args.image
    trivy = None
    import shutil
    for cand in (os.environ.get("TRIVY", ""), "trivy"):
        found = shutil.which(cand) if cand else None
        if found:
            trivy = found
            break
    if not trivy:
        note = {"leg": "trivy-image", "state": "SKIP", "note": "trivy not installed on this host"}
        (outdir / "trivy-image.json").write_text(json.dumps(note, indent=2))
        print("trivy not present -> image leg SKIP; lockfile gate remains the authorizer")
        return 0
    subprocess.run([trivy, "image", "--format", "cyclonedx",
                    "--output", str(outdir / "trivy-image.cdx.json"), image], check=False)
    subprocess.run([trivy, "image", "--format", "json",
                    "--output", str(outdir / "trivy-image.json"), image], check=False)
    findings, state = load_trivy_findings(outdir / "trivy-image.json")
    (outdir / "trivy-image-findings.json").write_text(json.dumps(findings, indent=2))
    sev_counts = {}
    for f in findings:
        sev_counts[f["severity"]] = sev_counts.get(f["severity"], 0) + 1
    print(f"trivy image scan written to {outdir}/trivy-image.*")
    print(f"findings={len(findings)} severity={sev_counts}")
    return 0


def main(argv=None):
    p = argparse.ArgumentParser(prog="sbom_cve", description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)

    def _common(sp):
        sp.add_argument("--repo", default=".", help="repo root (default cwd)")
        sp.add_argument("--ref", default=None, help="release/ref label (default git describe)")
        sp.add_argument("--out", default=None, help="artifact out dir (default workstreams/security/artifacts/<ref>)")
        sp.add_argument("--advisory-db", default=None, help="path to a rustsec advisory-db clone (default workstreams/security/.advisory-db)")
        sp.add_argument("--waivers", default=None, help="waivers file (default workstreams/security/waivers.toml)")
        sp.add_argument("--block-severities", default=None, help="comma list, default critical,high")
        sp.add_argument("--offline", action="store_true", help="never clone; require local advisory-db")
        sp.add_argument("--refresh", action="store_true", help="git pull the advisory-db before scanning")
        sp.add_argument("--with-osv", action="store_true", help="run OSV cross-check leg (needs network)")
        sp.add_argument("--with-trivy", action="store_true", help="declare trivy image leg (CI-safe)")
        sp.add_argument("--trivy-image", default=None, help="image tag for the trivy leg")
        sp.add_argument("--trivy-scan", default=None,
                        help="path to normalized trivy findings JSON to fold into the gate (release-host proof)")

    sp = sub.add_parser("sbom", help="emit CycloneDX+SPDX BOMs only")
    _common(sp)
    sp = sub.add_parser("scan", help="scan Cargo.lock vs advisory DB + report (no SBOM emission)")
    _common(sp)
    sp = sub.add_parser("gate", help="emit SBOMs + scan + enforce the release gate")
    _common(sp)
    sp = sub.add_parser("trivy-image", help="host-side trivy image scan proof leg")
    sp.add_argument("--image", required=True)
    sp.add_argument("--out", default="workstreams/security/proof")

    args = p.parse_args(argv)
    if args.cmd == "sbom":
        return cmd_sbom(args)
    if args.cmd == "scan":
        return cmd_scan(args)
    if args.cmd == "gate":
        return cmd_gate(args)
    if args.cmd == "trivy-image":
        return cmd_trivy_image(args)
    return 2


if __name__ == "__main__":
    sys.exit(main())
