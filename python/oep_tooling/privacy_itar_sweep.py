#!/usr/bin/env python3
"""Isolith P1-M1-T06: reusable privacy/ITAR sweep for corpus samples.

Pure stdlib, no third-party deps (tooling-workspace discipline). Scans CAD
sample files -- CATPart/CATProduct, CGR, 3DXML (zip or ASCII), STL, XML, JSON,
markdown -- for export-control (ITAR/USML/ECCN), classified (CUI/FOUO/etc),
and personal-data (email/SSN/phone) indicators, anchored to the file's sha256.

False-positive discipline (learned from the real 2026-10-15 corpus probe):
  * binary float / tessellation noise must not trip classified or contact
    markers, so short classifier tokens (CUI, FOUO) and phone patterns are only
    evaluated against text and UTF-16 metadata runs, not raw binary ASCII runs;
  * version strings like "1@1.0" are not emails: the email regex requires a
    TLD of 2+ alpha characters.

CLI:
  python3 -m oep_tooling.privacy_itar_sweep --corpus DIR [--corpus DIR...] \
      --out REPORT.json [--reference DIR...]
  exit code: 1 if any HIGH finding is present (and not dispositioned), else 0.

API: scan_bytes(name, data) -> list[Finding]; sweep_file(path) -> dict;
     verdict_for(path) -> dict (sha256 + verdict). Used by the acceptance test.
"""
import hashlib
import json
import os
import re
import sys
import zipfile

# --------------------------------------------------------------------------
# Findings
# --------------------------------------------------------------------------

class Finding:
    __slots__ = ("severity", "match", "kind", "context", "disposition")

    def __init__(self, severity, match, kind, context="", disposition=""):
        self.severity = severity      # HIGH | MEDIUM | INFO
        self.match = match            # matched text snippet
        self.kind = kind              # e.g. ITAR | CLASSIFIED | EMAIL
        self.context = context        # where it was found (text/utf16/binary)
        self.disposition = disposition  # human review note for HIGH findings

    def to_dict(self):
        d = {"severity": self.severity, "match": self.match[:200],
             "kind": self.kind, "context": self.context}
        if self.disposition:
            d["disposition"] = self.disposition
        return d


# --------------------------------------------------------------------------
# Rules
# --------------------------------------------------------------------------

# ASCII word boundaries: Python \b is Unicode-aware, so a CJK glyph rendered
# from binary noise blocks \b before an ASCII token. We only care that a token
# is not embedded in a longer ASCII word (e.g. "GITAR"/"GitARB" must not
# match), so use explicit ASCII lookarounds.
_WB = r"(?<![A-Za-z0-9_])"
_WE = r"(?![A-Za-z0-9_])"

# Export-control / ITAR / EAR markers. Word-boundary, case-insensitive.
ITAR_RE = re.compile(
    _WB + r"(it\s?ar|usml|eccn|ear99|munitions list|defense article|"
    r"specially designed|export control(?:led)?|ccats|commodity jurisdiction|"
    r"dd[_-]?tc|600-series|proscribed)" + _WE,
    re.IGNORECASE)
CFR_RE_STRICT = re.compile(r"22\s?cfr\s?[0-9]+")

# Classified / controlled-unclassified markers (long tokens; safe anywhere).
CLASSIFIED_RE = re.compile(
    _WB + r"(top secret|secret//|confidential//|controlled unclassified|"
    r"fouo|noforn|no foreign|//rel|for official use only)" + _WE, re.IGNORECASE)
# Short token "CUI" -- only meaningful in text / UTF-16 metadata runs, because
# binary float renderings sporadically spell "cUI" in noise.
CUI_RE = re.compile(_WB + r"CUI" + _WE)

# Personal data
EMAIL_RE = re.compile(
    r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9\-]+(\.[A-Za-z]{2,})+\b")
SSN_RE = re.compile(r"(?<!\d)\d{3}-\d{2}-\d{4}(?!\d)")
PHONE_RE = re.compile(r"(?<!\d)(\(?\d{3}\)?[\s.\-]?\d{3}[\s.\-]\d{4})(?!\d)")


def _add(results, rx, kind, text, context, severity="HIGH"):
    for m in rx.finditer(text):
        results.append(Finding(severity, m.group(0), kind, context))
    return results


def _printable_runs(data, min_len):
    """ASCII printable runs (bytes -> str), used for binary content."""
    runs, cur = [], []
    for b in data:
        if 32 <= b < 127:
            cur.append(chr(b))
        else:
            if len(cur) >= min_len:
                runs.append("".join(cur))
            cur = []
    if len(cur) >= min_len:
        runs.append("".join(cur))
    return runs


def _utf16_runs(data, min_len=4):
    try:
        txt = data.decode("utf-16-le", errors="ignore")
    except Exception:
        return []
    runs, cur = [], []
    for c in txt:
        if c.isprintable() or c in "\t\n\r ":
            cur.append(c)
        else:
            if len(cur) >= min_len:
                runs.append("".join(cur))
            cur = []
    if len(cur) >= min_len:
        runs.append("".join(cur))
    return runs


_TEXT_EXT = {".md", ".json", ".xml", ".txt", ".stl", ".dxml", ".3dxml",
             ".csv", ".yaml", ".yml", ".toml"}


def _is_textual(name):
    return os.path.splitext(name)[1].lower() in _TEXT_EXT


def scan_bytes(name, data):
    """Return list[Finding] for a blob, using filename to choose extraction.

    3DXML archives are zips: detect by content FIRST (a .3DXML / .3dxml file
    in Dassault's format is a ZIP, not a plain-text XML), then fall back to
    text-vs-binary for the non-zip files (ASCII 3DXML, STL, JSON, CATPart...).
    """
    findings = []

    def text_pass(text, context, allow_short_classified=True):
        _add(findings, ITAR_RE, "ITAR", text, context)
        if CFR_RE_STRICT.search(text):
            findings.append(Finding("HIGH", "22 CFR reference", "ITAR", context))
        _add(findings, CLASSIFIED_RE, "CLASSIFIED", text, context)
        if allow_short_classified and CUI_RE.search(text):
            # report up to a few CUI hits, dedup-ish
            for m in list(CUI_RE.finditer(text))[:3]:
                findings.append(Finding("HIGH", m.group(0), "CLASSIFIED", context))
        _add(findings, EMAIL_RE, "EMAIL", text, context)
        _add(findings, SSN_RE, "SSN", text, context)
        _add(findings, PHONE_RE, "PHONE", text, context)

    # 3DXML zip: scan text members fully; binary members conservatively.
    import io as _io
    if zipfile.is_zipfile(_io.BytesIO(data)):
        try:
            with zipfile.ZipFile(_io.BytesIO(data)) as z:
                for n in z.namelist():
                    try:
                        m = z.read(n)
                    except Exception:
                        continue
                    if n.lower().endswith((".xml", ".txt", ".md", ".json")):
                        text = m.decode("utf-8", errors="replace")
                        text_pass(text, "zip:%s" % n)
                    else:
                        for run in _printable_runs(m, 6):
                            _add(findings, ITAR_RE, "ITAR", run, "zip:%s" % n)
                            _add(findings, CLASSIFIED_RE, "CLASSIFIED",
                                 run, "zip:%s" % n)
                            _add(findings, EMAIL_RE, "EMAIL", run, "zip:%s" % n)
                        for run in _utf16_runs(m):
                            text_pass(run, "zip:%s" % n)
        except Exception:
            # fall through to generic binary handling if zip parse fails
            pass
        else:
            return findings

    # Non-zip text files (ASCII 3DXML, STL, JSON, markdown, XML): full pass.
    if _is_textual(name):
        try:
            text = data.decode("utf-8", errors="replace")
        except Exception:
            text = data.decode("latin-1", errors="replace")
        text_pass(text, "text")
        return findings

    # Generic binary (CATPart/CATProduct/CGR/3DRep): ASCII runs conservative
    # (no short classified tokens, no phone), UTF-16 runs full pass.
    for run in _printable_runs(data, 8):
        _add(findings, ITAR_RE, "ITAR", run, "binary-ascii")
        if CFR_RE_STRICT.search(run):
            findings.append(Finding("HIGH", "22 CFR reference", "ITAR",
                                    "binary-ascii"))
        _add(findings, CLASSIFIED_RE, "CLASSIFIED", run, "binary-ascii")
        _add(findings, EMAIL_RE, "EMAIL", run, "binary-ascii")
    for run in _utf16_runs(data, 4):
        text_pass(run, "binary-utf16")
    return findings


# --------------------------------------------------------------------------
# Per-file verdicts
# --------------------------------------------------------------------------

def _sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def sweep_file(path, relative_path=None):
    rel = relative_path or path
    with open(path, "rb") as fh:
        data = fh.read()
    findings = scan_bytes(os.path.basename(path), data)
    high = [f for f in findings if f.severity == "HIGH"]
    verdict = "CLEAN" if not high else "REVIEW"
    return {
        "relative_path": rel,
        "size": len(data),
        "sha256": _sha256(path),
        "verdict": verdict,
        "findings": [f.to_dict() for f in findings],
    }


def verdict_for(path):
    """dict with sha256 + verdict, plus findings (used by acceptance test)."""
    return sweep_file(path, relative_path=os.path.basename(path))


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------

def main(argv=None):
    import argparse
    ap = argparse.ArgumentParser(description="Isolith privacy/ITAR sweep")
    ap.add_argument("--corpus", action="append", required=True,
                    help="corpus dir to sweep (repeatable)")
    ap.add_argument("--out", required=True, help="JSON report output path")
    ap.add_argument("--reference", action="append", default=[],
                    help="additional dir swept as reference-only (not gating)")
    ap.add_argument("--schema-version", default="1.0")
    args = ap.parse_args(argv)

    def collect(dirs):
        files = []
        for d in dirs:
            d = os.path.abspath(d)
            for root, _sub, names in os.walk(d):
                for n in sorted(names):
                    if n.startswith("."):
                        continue
                    p = os.path.join(root, n)
                    rel = os.path.relpath(p, d)  # relative to corpus root
                    files.append((p, rel.replace(os.sep, "/")))
        return files

    report_files = []
    for p, rel in collect(args.corpus):
        report_files.append(sweep_file(p, relative_path=rel))

    reference_files = []
    for p, rel in collect(args.reference):
        try:
            reference_files.append(sweep_file(p, relative_path=rel))
        except Exception as exc:
            reference_files.append({"relative_path": rel, "error": str(exc)})

    high = [f for f in report_files if f["verdict"] == "REVIEW"
            and any(h["severity"] == "HIGH" and not h.get("disposition")
                    for h in f["findings"])]
    report = {
        "schema_version": args.schema_version,
        "tool": "p1-m1-t06/privacy-itar-sweep",
        "generated": "2026-10-15",
        "generated_by": "isolith worker (default profile)",
        "policy": ("Public/open-licensed samples only. Detector flags ITAR/USML/"
                   "ECCN, classified/CUI/FOUO markers, and personal contact data. "
                   "HIGH findings must carry a disposition note before a sample "
                   "may enter the shared repo."),
        "files": report_files,
        "reference_sweeps": reference_files,
        "summary": {
            "total": len(report_files),
            "clean": sum(1 for f in report_files if f["verdict"] == "CLEAN"),
            "review": sum(1 for f in report_files if f["verdict"] == "REVIEW"),
            "high_findings": len(sum(
                ([h for h in f["findings"] if h["severity"] == "HIGH"]
                 for f in report_files), [])),
            "gate": "PASS" if not high else "BLOCK",
        },
    }
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(report, fh, indent=2)
        fh.write("\n")
    print("swept %d files; summary=%s" % (len(report_files), report["summary"]))
    return 1 if high else 0


if __name__ == "__main__":
    sys.exit(main())
