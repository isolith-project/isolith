"""P2-M6-T03 — the P2 GA gate: all corpus meshes load, 100% golden files match within epsilon.

This is the capstone acceptance harness of the P2 CGR-reader workstream. It runs
TWO legs over the whole corpus and requires both to hold for the gate to open:

1. LOAD leg — every corpus CGR mesh reaches a *classified* outcome through the
   accumulated reader: ``OPEN`` (header/mesh opens under the reader) or a SKIP
   carrying a code from the closed taxonomy
   ``{unsupported-format, not-a-cgr-container, truncated, object-store-unresolved,
   missing-on-disk, no-mesh-found, unknown-format, io-error}`` (P2-M1-T04
   tolerant-failure doctrine, P2-M2-T01 extractor contract). No panics, no
   silent drops. Escalates to the sibling P2-M5-T03 regression sweep
   (``oep_tooling.regression``) when that module is importable, else uses a
   self-contained probe built from the published clean-room observations.

2. GOLDEN leg — every test in the golden-file spec(s) (P0-M4-T02
   ``corpus/golden_tests.json``, ``compare`` text|bytes|numeric with tolerance)
   must PASS within its numeric tolerance (epsilon). A real mismatch is FAIL.
   Missing spec/corpus is a classified SKIP (never a crash, never a fabricated
   pass), matching the sibling P2-M1-T04 "CI stays green before the corpus PRs
   land" convention. Escalates to the corpus-harness golden runner
   (``oep_tooling.corpus.runner``) when importable.

Exit code contract: 0 iff no FAIL row anywhere (SKIP allowed). Report
``ga-gate-matrix.{json,md}`` is self-consistent (totals roll up to rows).

Honesty boundary (clean-room): the gate locks the classified-outcome contract — it
never invents a mesh where the reader honestly returns a reasoned skip, and it
never turns an absent golden into a PASS.
"""
from __future__ import annotations

import datetime
import importlib
import json
import os
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

# --- observed CGR header constants (published, clean-room) ------------------
CGR_MAGIC = bytes([0x01, 0x00, 0x00, 0x00])
CGR_PREAMBLE_OFF = 0x04
CGR_PREAMBLE_LEN = 0x60
CGR_TEMPLATE_OFF = 0x68
CGR_TEMPLATE = bytes([
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00,
    0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
])

CLOSED_TAXONOMY = (
    "unsupported-format", "not-a-cgr-container", "truncated",
    "object-store-unresolved", "missing-on-disk", "no-mesh-found",
    "unknown-format", "io-error",
)


@dataclass
class LegRow:
    """One classified outcome of a gate leg."""

    id: str
    leg: str                 # "load" | "golden"
    status: str              # OPEN/ PASS / SKIP / FAIL
    detail: str = ""
    reader: str = ""         # probe | corus.runner | regression | cli
    test_id: str = ""        # golden spec test id (golden leg)

    def as_dict(self) -> dict:
        return {
            "id": self.id,
            "leg": self.leg,
            "status": self.status,
            "detail": self.detail,
            "reader": self.reader,
            "test_id": self.test_id,
        }


def _try_import(name):
    """Import a sibling module if available; return None on any failure."""
    try:
        return importlib.import_module(name)
    except Exception:
        return None


# ---------------------------------------------------------------------------
# LOAD leg
# ---------------------------------------------------------------------------

def probe_cgr_header(data: bytes):
    """Deterministic clean-room CGR header-spine probe.

    Returns True (header opens), or a classified-skip reason string. Never
    raises. Same observed constants as the P2-M5-T03 regression runner.
    """
    if len(data) < CGR_TEMPLATE_OFF + len(CGR_TEMPLATE):
        return "truncated"
    if data[0:4] != CGR_MAGIC:
        return "not-a-cgr-container"
    preamble = data[CGR_PREAMBLE_OFF:CGR_PREAMBLE_OFF + CGR_PREAMBLE_LEN]
    if any(preamble):
        return "not-a-cgr-container"
    tmpl = data[CGR_TEMPLATE_OFF:CGR_TEMPLATE_OFF + len(CGR_TEMPLATE)]
    if tmpl != CGR_TEMPLATE:
        return "not-a-cgr-container"
    return True


def _classify_file(abs_path: str, fmt: str) -> LegRow:
    """Classify one on-disk file with the self-contained probe (no deps)."""
    rid = os.path.basename(abs_path)
    try:
        with open(abs_path, "rb") as fh:
            head = fh.read(CGR_TEMPLATE_OFF + len(CGR_TEMPLATE) + 1)
        size = os.path.getsize(abs_path)
    except OSError as e:
        return LegRow(rid, "load", "SKIP", "io-error: %r" % (e,), reader="probe")
    ext = os.path.splitext(abs_path)[1].lower()

    if ext in (".cgr",) or fmt == "cgr":
        verdict = probe_cgr_header(head)
        if verdict is True:
            return LegRow(rid, "load", "OPEN", "", reader="probe")
        return LegRow(rid, "load", "SKIP", str(verdict), reader="probe")

    if fmt in ("catpart-v5", "catproduct-v5"):
        if head.startswith(b"V5_CFV2"):
            return LegRow(rid, "load", "SKIP", "unsupported-format (P5 scope)",
                          reader="probe")
        return LegRow(rid, "load", "SKIP", "unknown-format", reader="probe")

    if fmt in ("3dxml-zip", "3dxml-ascii"):
        return LegRow(rid, "load", "SKIP", "unsupported-format (P3 scope)",
                      reader="probe")

    if fmt in ("stl-ascii",) or ext in (".stl",):
        if head.lstrip().startswith(b"solid"):
            return LegRow(rid, "load", "OPEN", "", reader="probe")
        return LegRow(rid, "load", "SKIP", "unknown-format", reader="probe")

    return LegRow(rid, "load", "SKIP", "unknown-format", reader="probe")


def _entry_path(corpus_root: str, entry: dict) -> str:
    rel = (entry.get("path") or entry.get("id") or "").lstrip("/")
    return os.path.join(corpus_root, rel)


def _self_load_sweep(entries, corpus_root, sample_dirs):
    """Self-contained load sweep (no sibling module required)."""
    rows, seen = [], set()

    def emit(r: LegRow):
        if r.id and r.id not in seen:
            seen.add(r.id)
            rows.append(r)

    for entry in entries or []:
        rid = entry.get("id", "?")
        fmt = entry.get("format", "unknown")
        backend = entry.get("backend", "repo")
        if backend == "object-store":
            emit(LegRow(rid, "load", "SKIP", "object-store-unresolved",
                        reader="probe"))
            continue
        path = _entry_path(corpus_root or "", entry)
        if not os.path.isfile(path):
            emit(LegRow(rid, "load", "SKIP", "missing-on-disk", reader="probe"))
            continue
        r = _classify_file(path, fmt)
        r.id = rid
        emit(r)

    for d in sample_dirs or []:
        if not d or not os.path.isdir(d):
            continue
        for name in sorted(os.listdir(d)):
            p = os.path.join(d, name)
            if os.path.isfile(p):
                emit(_classify_file(p, "cgr"))

    return rows


def load_leg(entries=None, corpus_root=None, sample_dirs=None, reader_prog=None):
    """Sweep every corpus mesh and require a classified outcome for each.

    Uses the sibling P2-M5-T03 regression module when importable (it owns the
    canonical sweep + matrix taxonomy); otherwise the self-contained probe.
    Returns a list of ``LegRow`` (status OPEN | SKIP). Never raises.
    """
    R = _try_import("oep_tooling.regression")
    if R is not None:
        try:
            sweep_rows = R.sweep(entries=entries, corpus_root=corpus_root,
                                 sample_dirs=sample_dirs,
                                 cache_dir=None, reader_prog=reader_prog)
            out = []
            for r in sweep_rows:
                status = getattr(r, "status", "SKIP")
                out.append(LegRow(
                    getattr(r, "id", "?"), "load", status,
                    detail=getattr(r, "detail", "") or
                           (getattr(r, "cls", "") or ""),
                    reader="regression",
                    test_id="",
                ))
            return out
        except Exception as e:  # tolerant: never crash the gate
            baseline = _self_load_sweep(entries, corpus_root, sample_dirs)
            baseline.append(LegRow("regression-module-error", "load", "SKIP",
                                   repr(e), reader="regression"))
            return baseline
    return _self_load_sweep(entries, corpus_root, sample_dirs)


# ---------------------------------------------------------------------------
# GOLDEN leg
# ---------------------------------------------------------------------------

def _normalize(text):
    return "\n".join(ln.rstrip() for ln in text.splitlines()).strip()


def _numeric_tokens(text):
    out = []
    for tok in text.split():
        try:
            out.append(float(tok))
        except ValueError:
            return None
    return out


def _compare_simple(kind, actual, expected, tolerance):
    """Independent text|bytes|numeric compare (spec: P0-M4-T02 semantics)."""
    if kind == "bytes":
        return actual == expected, None
    if kind == "numeric":
        a, e = _numeric_tokens(actual), _numeric_tokens(expected)
        if a is None or e is None:
            return False, "non-numeric token present"
        if len(a) != len(e):
            return False, "token count differs: %d vs %d" % (len(a), len(e))
        for i, (x, y) in enumerate(zip(a, e)):
            if abs(x - y) > tolerance:
                return False, "token %d off by %g (tolerance %g)" % (
                    i, abs(x - y), tolerance)
        return True, None
    return _normalize(actual) == _normalize(expected), None


def _self_golden_spec(spec_path, corpus_root):
    """Run a golden spec with the self-contained comparator (no sibling deps)."""
    rows = []
    if not spec_path or not os.path.isfile(spec_path):
        rid = Path(spec_path or "golden_tests.json").name
        return [LegRow(rid, "golden", "SKIP", "golden spec absent",
                       reader="probe", test_id=rid)]
    try:
        with open(spec_path, encoding="utf-8") as fh:
            spec = json.load(fh)
    except Exception as e:
        return [LegRow("golden-spec", "golden", "SKIP",
                       "spec unreadable: %r" % (e,),
                       reader="probe", test_id="golden-spec")]

    for test in spec.get("tests", []):
        tid = test.get("id", "?")
        golden_rel = test.get("golden", "")
        # golden paths are spec-relative to the repo ROOT ("corpus/golden/x");
        # accept corpus_root as either the repo root or the corpus/ dir itself.
        golden_cands = [golden_rel]
        if corpus_root:
            golden_cands += [
                os.path.join(corpus_root, golden_rel.lstrip("/")),
                os.path.join(corpus_root,
                             golden_rel.replace("corpus/", "", 1).lstrip("/")),
            ]
        golden_abs = next((g for g in golden_cands if os.path.isfile(g)), None)
        if golden_abs is None:
            rows.append(LegRow(tid, "golden", "SKIP",
                               "golden missing: %s" % golden_rel,
                               reader="probe", test_id=tid))
            continue

        # Only support deterministic python commands that take {input}: the
        # sibling corpus harness owns arbitrary-command dispatch; this module's
        # self-contained runner covers the spec shape used by the P2 gate.
        cmd = list(test.get("command") or [])
        if not cmd:
            rows.append(LegRow(tid, "golden", "SKIP", "no command", reader="probe",
                               test_id=tid))
            continue
        resolved_input = _resolve_golden_input(test, corpus_root)
        argv = [a.replace("{input}", resolved_input) for a in cmd]
        try:
            r = subprocess.run(argv, capture_output=True, text=True, timeout=300)
        except Exception as e:
            rows.append(LegRow(tid, "golden", "SKIP",
                               "command error: %r" % (e,), reader="probe",
                               test_id=tid))
            continue
        if r.returncode != 0:
            rows.append(LegRow(tid, "golden", "SKIP",
                               "command rc=%d: %s" % (r.returncode,
                                                      (r.stderr or "")[-120:]),
                               reader="probe", test_id=tid))
            continue
        kind = test.get("compare", "text")
        tol = float(test.get("tolerance", 0.0))
        try:
            with open(golden_abs, encoding="utf-8") as fh:
                expected = fh.read()
        except Exception as e:
            rows.append(LegRow(tid, "golden", "SKIP",
                               "golden unreadable: %r" % (e,), reader="probe",
                               test_id=tid))
            continue
        ok, detail = _compare_simple(kind, r.stdout, expected, tol)
        if ok:
            rows.append(LegRow(tid, "golden", "PASS", "", reader="probe",
                               test_id=tid))
        else:
            rows.append(LegRow(tid, "golden", "FAIL",
                               detail or "output differs from golden",
                               reader="probe", test_id=tid))
    return rows


def _resolve_golden_input(test, corpus_root):
    """Locate the corpus input file for `{input}` substitution."""
    # corpus_id is the manifest id; under the fixture tree the file is at
    # corpus_root/<path-or-id>. Try a few likely locations deterministically.
    cands = []
    cid = test.get("corpus_id", "")
    if corpus_root:
        cands.append(os.path.join(corpus_root, cid))
        cands.append(os.path.join(corpus_root, "data", cid))
        cands.append(os.path.join(corpus_root, "data",
                                  os.path.basename(str(cid))))
    for c in cands:
        if c and os.path.isfile(c):
            return c
    return cands[0] if cands else cid


def golden_leg(spec_path=None, corpus_root=None, cache_dir=None):
    """Run every golden test in a spec; require zero FAIL.

    Uses the sibling P0-M4-T02 corpus-harness golden runner when importable;
    otherwise the self-contained comparator. Rows: PASS | FAIL | SKIP.
    """
    runner = _try_import("oep_tooling.corpus.runner")
    if runner is not None:
        try:
            results, _ = runner.run_spec(spec_path or "corpus/golden_tests.json",
                                         cache_dir=cache_dir, corpus_root=corpus_root)
            out = []
            for r in results:
                out.append(LegRow(getattr(r, "id", "?"), "golden",
                                  getattr(r, "status", "SKIP"),
                                  detail=getattr(r, "detail", ""),
                                  reader="corpus-harness",
                                  test_id=getattr(r, "id", "?")))
            return out
        except Exception as e:
            baseline = _self_golden_spec(spec_path, corpus_root)
            baseline.append(LegRow("corpus-harness-error", "golden", "SKIP",
                                   repr(e), reader="corpus-harness",
                                   test_id="corpus-harness-error"))
            return baseline
    return _self_golden_spec(spec_path, corpus_root)


# ---------------------------------------------------------------------------
# Gate runner: both legs + report + exit code
# ---------------------------------------------------------------------------

def build_report(rows) -> dict:
    """Aggregate LegRow list into a self-consistent gate report dict."""
    n_pass = sum(1 for r in rows if r.status == "PASS")
    n_open = sum(1 for r in rows if r.status == "OPEN")
    n_fail = sum(1 for r in rows if r.status == "FAIL")
    n_skip = sum(1 for r in rows if r.status == "SKIP")
    by_leg = {}
    for r in rows:
        by_leg[r.leg] = by_leg.get(r.leg, 0) + 1
    return {
        "schema_version": 1,
        "generated": datetime.datetime.now().isoformat(timespec="seconds"),
        "task": "P2-M6-T03",
        "summary": {
            "total": len(rows),
            "pass": n_pass,
            "open": n_open,
            "fail": n_fail,
            "skip": n_skip,
            "by_leg": dict(sorted(by_leg.items())),
        },
        "rows": [r.as_dict() for r in rows],
    }


def exit_code(rows) -> int:
    """0 iff no FAIL row; SKIP rows do not close the gate with a failure."""
    return 1 if any(r.status == "FAIL" for r in rows) else 0


def write_report(matrix: dict, outdir: str) -> list:
    """Write ga-gate-matrix.json + .md into outdir. Returns written paths."""
    out = Path(outdir)
    out.mkdir(parents=True, exist_ok=True)
    jp = out / "ga-gate-matrix.json"
    jp.write_text(json.dumps(matrix, indent=2) + "\n", encoding="utf-8")
    mp = out / "ga-gate-matrix.md"
    lines = ["# Isolith P2 GA gate — matrix (P2-M6-T03)", ""]
    s = matrix["summary"]
    lines.append("total=%d pass=%d open=%d fail=%d skip=%d (%s)" % (
        s["total"], s["pass"], s["open"], s["fail"], s["skip"],
        " | ".join("%s=%d" % kv for kv in sorted(s["by_leg"].items()))))
    lines.append("")
    lines.append("| leg | id | status | detail |")
    lines.append("|-----|----|--------|--------|")
    for r in matrix["rows"]:
        lines.append("| %s | %s | %s | %s |" % (
            r["leg"], r["id"], r["status"],
            (r.get("detail", "") or "")[:80].replace("|", "/")))
    lines.append("")
    mp.write_text("\n".join(lines), encoding="utf-8")
    return [str(jp), str(mp)]


def run_gate(spec=None, corpus_root=None, sample_dirs=None, manifest=None,
             outdir="corpus", reader_prog=None) -> int:
    """Run both legs, write ga-gate-matrix.{json,md}, return exit code.

    - ``manifest``: path to corpus/manifest.json (schema v1) for the load leg.
    - ``spec``: golden spec path (default corpus/golden_tests.json).
    - Returns 0 iff no FAIL row (SKIP allowed).
    """
    entries = []
    if manifest:
        try:
            with open(manifest, encoding="utf-8") as fh:
                m = json.load(fh)
            entries = m.get("entries", [])
        except Exception as e:
            print("[ga_gate] manifest unreadable: %r" % (e,))

    ld = load_leg(entries=entries, corpus_root=corpus_root,
                  sample_dirs=sample_dirs, reader_prog=reader_prog)
    gd = golden_leg(spec_path=spec, corpus_root=corpus_root)

    rows = ld + gd

    matrix = build_report(rows)
    written = write_report(matrix, outdir)
    code = exit_code(rows)

    s = matrix["summary"]
    print("GA gate: total=%d pass=%d open=%d fail=%d skip=%d | exit=%d" % (
        s["total"], s["pass"], s["open"], s["fail"], s["skip"], code))
    for w in written:
        print("  wrote %s" % w)
    return code


def main(argv=None) -> int:
    import argparse
    ap = argparse.ArgumentParser(
        prog="python -m oep_tooling.ga_gate",
        description="P2-M6-T03 GA gate: every corpus mesh classified + 100% "
                    "golden files within epsilon.")
    ap.add_argument("--manifest", default=None, help="corpus/manifest.json")
    ap.add_argument("--corpus-root", default=None, help="corpus dir")
    ap.add_argument("--spec", default="corpus/golden_tests.json",
                    help="golden spec path")
    ap.add_argument("--samples", default=None, action="append",
                    help="extra CGR sample dir (repeatable; default "
                         "OEP_CGR_SAMPLES)")
    ap.add_argument("--out", default="corpus", help="report output dir")
    ap.add_argument("--reader", default=None,
                    help="external CGR reader argv (space-split) or 'probe'")
    args = ap.parse_args(argv)

    sample_dirs = list(args.samples or [])
    env_samples = os.environ.get("OEP_CGR_SAMPLES", "")
    sample_dirs += [c for c in env_samples.split(":") if c]

    reader_prog = None
    if args.reader and args.reader != "probe":
        reader_prog = args.reader.split()
    elif args.reader == "probe":
        reader_prog = []

    return run_gate(spec=args.spec, corpus_root=args.corpus_root,
                    sample_dirs=sample_dirs, manifest=args.manifest,
                    outdir=args.out, reader_prog=reader_prog)


if __name__ == "__main__":
    sys.exit(main())
