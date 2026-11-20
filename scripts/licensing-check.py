#!/usr/bin/env python3
"""LICENSE-FILES-AND-BOUNDARY check (t_fb3a286b).

Enforces the Isolith hybrid licence split in CI. Honest exit-code contract
(same discipline as scripts/format-matrix-check.py): 0 iff no FAIL row; the
check is the evidence, not the document.

Legs (all offline, committed files only):
  A. Crate boundary   - every workspace crate's Cargo.toml [package].license
       equals docs/licensing-boundary.json, and that map covers every workspace
       member (no unknown crates, no orphans).
  B. Boundary doc sync - docs/licensing-boundary.md carries a row for every
       crate/licence in the boundary JSON.
  C. Dependency allow-list - every third-party package in Cargo.lock has an
       exact name@version entry in docs/dependency-licenses.json, its SPDX
       expression resolves within the pure-permissive allow-list, and no
       denied identifier (GPL-3.0, AGPL-3.0, LGPL-2.1-...) is forced.
  D. NOTICE sync      - NOTICE carries attribution (name version - licence)
       for every dependency, plus the native OCCT component.
  E. Root files       - LICENSE, LICENSE-APACHE, LICENSE-AGPL, NOTICE exist at
       the repo root and the LICENSE index names both texts.

--regen rebuilds docs/dependency-licenses.json and NOTICE from cargo metadata
(authoritative crates.io index; maintainer action, run locally before
committing a dependency change; CI never calls it).

Usage:
  scripts/licensing-check.py [--repo ROOT] [--out -]
  scripts/licensing-check.py --regen
"""

import argparse
import json
import os
import re
import subprocess
import sys
import tomllib

SCHEMA_VERSION = 1

# --- policy -----------------------------------------------------------------
# Pure-permissive SPDX identifiers a dependency may use (OR branches may also
# be permissive; AND conjuncts must ALL be permissive).
# Font licenses are permissive data licenses (no copyleft, no field
# restriction): OFL-1.1 (SIL Open Font License) and Ubuntu-font-1.0 (Ubuntu
# Font Licence) were added 2026-11-20 by the UI-A1 dependency card because the
# egui UI stack (crates/oep-app, editor parity wave A) embeds its built-in
# fonts through epaint_default_fonts, whose SPDX is
# "(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0".
ALLOWED = {
    "Apache-2.0", "MIT", "ISC", "BSL-1.0",
    "BSD-1-Clause", "BSD-2-Clause", "BSD-3-Clause",
    "CC0-1.0", "CC-BY-4.0", "MIT-0", "Unlicense", "Zlib", "Unicode-3.0",
    "OFL-1.1", "Ubuntu-font-1.0",
}
# Identifiers explicitly barred from the delivered tree (defense clean-room
# gate: "No GPL-3.0 in the delivered tree, consistent with the cleanroom
# gate"). A denied-id leaf that is the only option fails with a clean-room
# message; an OR expression that also offers a permissive branch stays green.
DENIED = {
    "GPL-2.0", "GPL-2.0-only", "GPL-2.0-or-later", "GPL-3.0", "GPL-3.0-only",
    "GPL-3.0-or-later", "AGPL-1.0", "AGPL-3.0", "AGPL-3.0-only",
    "AGPL-3.0-or-later", "LGPL-2.1", "LGPL-2.1-only", "LGPL-2.1-or-later",
    "LGPL-3.0", "LGPL-3.0-only", "LGPL-3.0-or-later",
}

# Native components that are NOT resolved through Cargo.lock (foreign build
# systems, e.g. the OCCT C++ kernel) but must appear in NOTICE all the same.
NATIVE = [
    {
        "component": "Open CASCADE Technology (OCCT)",
        "license": "LGPL-2.1 with the OCCT/Open CASCADE Exception 1.0",
        "role": "geometry kernel bound via crates/oep-engine-occ (fetched/built "
                "by scripts/kernel; never vendored in-tree)",
    },
]

# crates.io-verified SPDX for the dev/build toolchain deps cargo metadata
# cannot see unless it builds the bridge (verified via the crates.io API on
# 2026-10-27). Maintained in lockstep with the pinned versions in Cargo.lock.
_REGEN_OVERRIDES = {
    "anstyle@1.0.14": "MIT OR Apache-2.0",
    "clap@4.6.4": "MIT OR Apache-2.0",
    "clap_builder@4.6.2": "MIT OR Apache-2.0",
    "clap_lex@1.1.0": "MIT OR Apache-2.0",
    "cxxbridge-cmd@1.0.205": "MIT OR Apache-2.0",
    "strsim@0.11.1": "MIT",
}


# --- minimal SPDX expression evaluator -------------------------------------
def _split_top(expr, word):
    """Split expr on `word` (case-sensitive) at parenthesis depth 0."""
    parts, depth, cur = [], 0, ""
    i = 0
    n = len(expr)
    while i < n:
        ch = expr[i]
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if (
            depth == 0 and expr[i:i + len(word)] == word
            and (i == 0 or expr[i - 1].isspace())
            and (i + len(word) == n or expr[i + len(word)].isspace())
        ):
            parts.append(cur.strip())
            cur = ""
            i += len(word)
            continue
        cur += ch
        i += 1
    parts.append(cur.strip())
    return [p for p in parts if p]


def _outer_parens_match(expr):
    if not (expr.startswith("(") and expr.endswith(")")):
        return False
    depth = 0
    for i, ch in enumerate(expr):
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0 and i != len(expr) - 1:
                return False
    return depth == 0


def eval_spdx(expr, allowed):
    """True if some legal reading of the SPDX expression uses only `allowed`.
    Legacy "/" is treated as OR. Returns (ok, first_disallowed_leaf)."""
    if expr is None or not str(expr).strip():
        return False, "<missing>"
    e = str(expr).strip().replace("/", " OR ")
    while _outer_parens_match(e):
        e = e[1:-1].strip()
        if not e:
            break
    for word in ("OR", "AND"):
        parts = _split_top(e, word)
        if len(parts) > 1:
            if word == "OR":
                for p in parts:
                    ok, leaf = eval_spdx(p, allowed)
                    if ok:
                        return True, None
                return False, leaf
            # AND: every conjunct must be acceptable
            for p in parts:
                ok, leaf = eval_spdx(p, allowed)
                if not ok:
                    return False, leaf
            return True, None
    base = e.split(" WITH ")[0].strip()
    if base in allowed:
        return True, None
    return False, base


def deny_reason(base):
    return f"denied by clean-room gate (no {base} in the delivered tree)" \
        if base in DENIED else f"not in allow-list"


# --- helpers ----------------------------------------------------------------
def load_json(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def safe_read(path):
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except OSError:
        return ""


def workspace_crates(repo):
    """{crate_name: crate_dir} from the root Cargo.toml members."""
    with open(os.path.join(repo, "Cargo.toml"), "rb") as fh:
        root = tomllib.load(fh)
    crates = {}
    for member in root["workspace"]["members"]:
        with open(os.path.join(repo, member, "Cargo.toml"), "rb") as fh:
            ct = tomllib.load(fh)
        crates[ct["package"]["name"]] = member
    return crates


def lock_third_party(repo, workspace_names):
    """[(name, version)] for every non-workspace package in Cargo.lock."""
    with open(os.path.join(repo, "Cargo.lock"), "rb") as fh:
        lock = tomllib.load(fh)
    out = []
    for p in lock["package"]:
        if p["name"] in workspace_names:
            continue
        out.append((p["name"], p["version"]))
    return out


def classify(rows, anchor, status, detail, source):
    rows.append({"id": anchor, "status": status, "detail": detail,
                 "source": source})
    return rows


# --- legs -------------------------------------------------------------------
def leg_crate_boundary(rows, repo, crates):
    try:
        bmap = load_json(os.path.join(repo, "docs", "licensing-boundary.json"))
    except (OSError, ValueError) as e:
        return classify(rows, "boundary-json", "FAIL",
                        f"docs/licensing-boundary.json unreadable: {e}",
                        "docs/licensing-boundary.json")
    if bmap.get("schema_version") != SCHEMA_VERSION:
        classify(rows, "boundary-schema", "FAIL",
                 f"schema_version {bmap.get('schema_version')} != {SCHEMA_VERSION}",
                 "docs/licensing-boundary.json")
    else:
        classify(rows, "boundary-schema", "PASS",
                 f"schema_version {SCHEMA_VERSION}",
                 "docs/licensing-boundary.json")

    mapped = {r["crate"]: r for r in bmap.get("crates", [])}
    unknown = sorted(set(mapped) - set(crates))
    orphan = sorted(set(crates) - set(mapped))
    bad = []
    for name in unknown:
        bad.append(f"boundary map names unknown crate {name!r}")
    for name in orphan:
        bad.append(f"workspace crate {name!r} missing from boundary map")
    for name, member in sorted(crates.items()):
        ct_path = os.path.join(member, "Cargo.toml")
        with open(ct_path, "rb") as fh:
            declared = tomllib.load(fh)["package"].get("license")
        expected = mapped.get(name, {}).get("license")
        if declared != expected:
            bad.append(
                f"{name}: Cargo.toml declares {declared!r}, boundary map "
                f"requires {expected!r} ({ct_path})")
    if bad:
        classify(rows, "crate-boundary", "FAIL", "; ".join(bad), "Cargo.toml/*")
    else:
        classify(rows, "crate-boundary", "PASS",
                 f"{len(crates)} crates match the boundary map",
                 "Cargo.toml/*")


def leg_boundary_doc(rows, repo, crates):
    md = safe_read(os.path.join(repo, "docs", "licensing-boundary.md"))
    if not md:
        return classify(rows, "boundary-doc-sync", "FAIL",
                        "docs/licensing-boundary.md absent",
                        "docs/licensing-boundary.md")
    norm = re.sub(r"\s+", " ", md.lower())
    # Strip table separators / emphasis so a table row "| oep-cgr | Apache-2.0 |"
    # reads as the adjacent tokens "oep-cgr apache-2.0".
    norm = re.sub(r"[\|`*_]", " ", norm)
    norm = re.sub(r"\s+", " ", norm)
    try:
        bmap = load_json(os.path.join(repo, "docs", "licensing-boundary.json"))
    except (OSError, ValueError) as e:
        return classify(rows, "boundary-doc-sync", "FAIL",
                        f"JSON unreadable for sync check: {e}", "boundary")
    bad = []
    for r in bmap.get("crates", []):
        key = f"{r['crate']} {r['license']}".lower()
        if key not in norm:
            bad.append(f"licensing-boundary.md missing row for {r['crate']} "
                       f"({r['license']})")
    if bad:
        classify(rows, "boundary-doc-sync", "FAIL", "; ".join(bad),
                 "docs/licensing-boundary.md")
    else:
        classify(rows, "boundary-doc-sync", "PASS",
                 f"{len(bmap.get('crates', []))} rows in sync",
                 "docs/licensing-boundary.md")


def leg_dependencies(rows, repo, third):
    try:
        dep = load_json(os.path.join(repo, "docs", "dependency-licenses.json"))
    except (OSError, ValueError) as e:
        return classify(rows, "dependency-allowlist", "FAIL",
                        f"docs/dependency-licenses.json unreadable: {e}",
                        "docs/dependency-licenses.json")
    manifest = dep.get("dependencies", {})
    bad, n = [], 0
    for name, version in sorted(third):
        key = f"{name}@{version}"
        entry = manifest.get(key)
        if entry is None:
            bad.append(f"{key}: not declared in dependency-licenses.json "
                       f"(run scripts/licensing-check.py --regen)")
            continue
        n += 1
        lic = entry.get("license")
        ok, leaf = eval_spdx(lic, ALLOWED)
        if not ok:
            bad.append(f"{key}: licence {lic!r} {deny_reason(leaf)}")
    # a manifest entry that no longer resolves in the lock is stale/removed
    locked = {f"{n}@{v}" for n, v in third}
    for key in manifest:
        if key not in locked:
            bad.append(f"{key}: manifest entry not in Cargo.lock "
                       f"(run scripts/licensing-check.py --regen)")
    if bad:
        classify(rows, "dependency-allowlist", "FAIL", "; ".join(bad[:6]),
                 "docs/dependency-licenses.json")
    else:
        classify(rows, "dependency-allowlist", "PASS",
                 f"{n} dependencies within the permissive allow-list",
                 "docs/dependency-licenses.json")


def leg_notice(rows, repo, third):
    notice = safe_read(os.path.join(repo, "NOTICE"))
    if not notice:
        return classify(rows, "notice-sync", "FAIL", "NOTICE absent at root",
                        "NOTICE")
    try:
        dep = load_json(os.path.join(repo, "docs", "dependency-licenses.json"))
    except (OSError, ValueError) as e:
        return classify(rows, "notice-sync", "FAIL",
                        f"dependency manifest unreadable: {e}", "NOTICE")
    norm = re.sub(r"\s+", " ", notice.lower())
    bad = []
    for name, version in sorted(third):
        key = f"{name}@{version}"
        entry = dep.get("dependencies", {}).get(key)
        if not entry:
            continue  # dependency leg already fails for it
        token = f"{name} {entry['version']} - {entry['license']}".lower()
        if token not in norm:
            bad.append(f"NOTICE missing/retyped line for {name} {version}")
    for nat in NATIVE:
        token = f"{nat['component'].lower()} - {nat['license'].lower()}"
        if token not in norm:
            bad.append(f"NOTICE missing OCCT attribution "
                       f"({nat['component']} - {nat['license']})")
    if bad:
        classify(rows, "notice-sync", "FAIL", "; ".join(bad[:6]), "NOTICE")
    else:
        classify(rows, "notice-sync", "PASS",
                 f"{len(third)} dependency lines + {len(NATIVE)} native entries",
                 "NOTICE")


def leg_root_files(rows, repo):
    required = ["LICENSE", "LICENSE-APACHE", "LICENSE-AGPL", "NOTICE"]
    bad = [f for f in required
           if not os.path.isfile(os.path.join(repo, f))]
    idx = safe_read(os.path.join(repo, "LICENSE"))
    for tok in ("LICENSE-APACHE", "LICENSE-AGPL", "licensing-boundary"):
        if tok not in idx:
            bad.append(f"LICENSE index does not mention {tok}")
    if bad:
        classify(rows, "root-files", "FAIL", "; ".join(bad), "repo root")
    else:
        classify(rows, "root-files", "PASS",
                 f"{len(required)} licence files at root, index in sync",
                 "repo root")


# --- regen ------------------------------------------------------------------
def regen(repo):
    """Rebuild docs/dependency-licenses.json and NOTICE from cargo metadata."""
    import shutil
    cargo = shutil.which("cargo")
    if not cargo:
        raise SystemExit("cargo not on PATH; --regen needs cargo metadata")
    res = subprocess.run(
        [cargo, "metadata", "--format-version", "1", "--all-features"],
        cwd=repo, capture_output=True, text=True)
    if res.returncode != 0:
        raise SystemExit(f"cargo metadata failed (rc {res.returncode}):\n"
                         + res.stderr[-1200:])
    meta = json.loads(res.stdout)
    with open(os.path.join(repo, "Cargo.toml"), "rb") as fh:
        members = tomllib.load(fh)["workspace"]["members"]
    ws_names = set()
    for member in members:
        with open(os.path.join(repo, member, "Cargo.toml"), "rb") as fh:
            ws_names.add(tomllib.load(fh)["package"]["name"])
    lic_by = {}
    for p in meta["packages"]:
        if p["name"] in ws_names:
            continue
        lic_by[(p["name"], p["version"])] = p.get("license")

    with open(os.path.join(repo, "Cargo.lock"), "rb") as fh:
        lock = tomllib.load(fh)
    deps = {}
    unknown = []
    for p in lock["package"]:
        if p["name"] in ws_names:
            continue
        key = f"{p['name']}@{p['version']}"
        lic = _REGEN_OVERRIDES.get(key) or lic_by.get((p["name"], p["version"]))
        if not lic:
            unknown.append(key)
            lic = "UNKNOWN-TRIAGE"
        deps[key] = {"name": p["name"], "version": p["version"],
                     "license": lic}

    doc = {
        "schema_version": 1,
        "generated": __import__("datetime").date.today().isoformat(),
        "source": "cargo metadata --format-version 1 --all-features "
                  "(crates.io index); toolchain-only deps via crates.io API",
        "count": len(deps),
        "triaged": len(unknown),
        "dependencies": deps,
    }
    def _norm(s):
        return re.sub(r"\s+", " ", s.strip())
    lines = []
    for key in sorted(deps):
        d = deps[key]
        lines.append(f"{_norm(d['name'])} {_norm(d['version'])}"
                     f" - {_norm(d['license'])}")
    notice = (NOTICE_HEADER +
              "\n\nThird-party dependencies (from Cargo.lock)\n"
              "-------------------------------------------\n\n" +
              "\n".join(lines) + "\n\n" + NOTICE_NATIVE_HEADER + "\n\n" +
              "\n".join(
                  f"- {nat['component']} - {nat['license']}\n"
                  f"  ({nat['role']})" for nat in NATIVE) + "\n")

    with open(os.path.join(repo, "docs", "dependency-licenses.json"), "w",
              encoding="utf-8") as fh:
        json.dump(doc, fh, indent=2, sort_keys=True)
        fh.write("\n")
    with open(os.path.join(repo, "NOTICE"), "w", encoding="utf-8") as fh:
        fh.write(notice)
    print(f"regen: {len(deps)} dependencies + {len(NATIVE)} native -> "
          f"docs/dependency-licenses.json, NOTICE")
    if unknown:
        print("UNKNOWN licenses left as UNKNOWN-TRIAGE (check will FAIL):")
        for k in unknown:
            print("  " + k)
        return 1
    return 0


NOTICE_HEADER = """Isolith / Open Engineering Platform
=================================
Third-party NOTICE

This software includes third-party open-source components. Each component is
the work of its respective copyright holders and is used under its own
licence. This NOTICE is generated from the committed Cargo.lock by
scripts/licensing-check.py --regen and verified in CI; do not edit by hand -
run the generator.

Isolith itself is dual-licensed:
  - Apache-2.0 (see LICENSE-APACHE) for applications and libraries.
  - AGPL-3.0 (see LICENSE-AGPL) for the server, PDM and P2P orchestration.
See LICENSE and docs/licensing-boundary.md for the per-crate boundary.
"""

NOTICE_NATIVE_HEADER = """Native components not resolved by Cargo
---------------------------------------
These are fetched/built outside Cargo (foreign build systems) and so do not
appear in Cargo.lock; they are named here because they ship in the linked
software."""


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--repo", default=None)
    ap.add_argument("--out", default=None, help="proof JSON path "
                    "(default: docs/proof/licensing-check.json; '-' stdout)")
    ap.add_argument("--regen", action="store_true",
                    help="rebuild docs/dependency-licenses.json + NOTICE "
                         "from cargo metadata, then exit")
    args = ap.parse_args()

    here = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.abspath(args.repo) if args.repo \
        else os.path.dirname(here)
    if args.regen:
        return regen(repo)

    rows = []
    crates = workspace_crates(repo)          # {name: member_dir}
    third = lock_third_party(repo, set(crates))
    leg_root_files(rows, repo)
    leg_crate_boundary(rows, repo, crates)
    leg_boundary_doc(rows, repo, crates)
    leg_dependencies(rows, repo, third)
    leg_notice(rows, repo, third)

    states = {}
    for r in rows:
        states.setdefault(r["status"], []).append(r["id"])
    fail_rows = states.get("FAIL", [])
    summary = {"total": len(rows), "pass": len(states.get("PASS", [])),
               "fail": len(fail_rows), "skip": len(states.get("SKIP", []))}
    report = {
        "schema_version": 1,
        "generated": __import__("datetime").date.today().isoformat(),
        "task": "t_fb3a286b license-files-and-boundary",
        "exit": 0 if not fail_rows else 1,
        "summary": summary,
        "rows": rows,
    }
    out_path = args.out
    if out_path == "-":
        json.dump(report, sys.stdout, indent=2)
        sys.stdout.write("\n")
    else:
        if not out_path:
            out_dir = os.path.join(repo, "docs", "proof")
            os.makedirs(out_dir, exist_ok=True)
            out_path = os.path.join(out_dir, "licensing-check.json")
        else:
            os.makedirs(os.path.dirname(os.path.abspath(out_path)),
                        exist_ok=True)
        with open(out_path, "w", encoding="utf-8") as fh:
            json.dump(report, fh, indent=2)
        print(f"licensing check: total={summary['total']} "
              f"pass={summary['pass']} fail={summary['fail']} "
              f"skip={summary['skip']} | exit={report['exit']}")
        print(f"report: {out_path}")
        for r in rows:
            print(f"  [{r['status']:4}] {r['id']}: {r['detail'][:140]}")
    return report["exit"]


if __name__ == "__main__":
    sys.exit(main())
