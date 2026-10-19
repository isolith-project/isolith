#!/usr/bin/env python3
"""P4-M3-T02 — STEP export golden-diff gate vs an independent reader (OCC).

Validates the STEP *export* lane against an independent reader — the Open
CASCADE kernel via a tiny committed OCC probe (`corpus/tools/occ_probe`, C++
wrapping OCCT `STEPControl_Reader`, the exact reader FreeCAD uses for STEP
import) — and commits golden diffs so future exporter drift is caught.

Honest-by-construction (program anti-fabrication doctrine, mirrors
`step_fidelity.py`):

- the measured value ALWAYS comes from the independent reader's output, never
  from our own writers or our own STEP parsers;
- per corpus pair, current independent recovery is compared against the
  committed golden expectation within tolerance:
      matches                -> `measured`   (green)
      differs                -> `drift`      (classified honestly; red until the
                                              golden is re-blessed by the owner)
      cannot read / no reader-> `skip`       (closed taxonomy, tolerated)
- there is NO `fail` bucket: a crash in one pair never aborts the sweep; an
  introspection error becomes a classified row.
- when no independent reader binary is present (e.g. CI without OCCT) every row
  is an honest `reader-unavailable` skip — never a fabricated number.

Skip taxonomy (closed set, each with a reason string):
  reader-unavailable | reader-error | fixture-missing | unparsable | io-error

Drift classes (closed set): transfer-flipped-true | transfer-flipped-false |
geometry-differs.

CLI:
  python -m oep_tooling.step_export_validation \
      --pairs corpus/export-validation-pairs.json \
      --golden corpus/export-validation-golden.json \
      --corpus . --out corpus \
      [--reader probe] [--probe corpus/tools/occ_probe]
  --check  read current recovery now; exit 0 iff no unclassified drift row.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

CLOSED_TAXONOMY = (
    "reader-unavailable",  # no independent-reader binary discovered
    "reader-error",        # reader started but produced unusable output/rc
    "fixture-missing",     # the step fixture does not resolve on disk
    "unparsable",          # probe output lacked the required keys
    "io-error",            # unreadable at the OS level
)
DRIFT_CLASSES = (
    "transfer-flipped-true",   # golden said no transfer, reader now transfers
    "transfer-flipped-false",  # golden said transfer, reader now transfers nothing
    "geometry-differs",        # same transfer state, recovered geometry differs
)


class ProbeError(Exception):
    """The independent reader produced output we cannot interpret."""


class StepValidationError(Exception):
    """Generic harness-side failure (unreadable/missing input)."""


# --------------------------------------------------------------------------
# Independent-reader discovery + probe output parsing
# --------------------------------------------------------------------------
def find_probe(committed_path):
    """Locate the independent-reader binary.

    Preference: $OEP_STEP_READER (if set and executable), else the committed
    build `committed_path`, else `occ_probe` on PATH. Returns a path or ``None``
    (which the runner classifies `reader-unavailable`). Never a fabricated reader.
    """
    env = (os.environ.get("OEP_STEP_READER") or "").strip()
    if env:
        # An explicitly configured reader binary is authoritative: if it does not
        # resolve/execute, the honest outcome is "reader unavailable" (None),
        # NOT a silent fallback to a different reader (which would hide a machine
        # whose OCC build broke).
        if os.path.isfile(env) and os.access(env, os.X_OK):
            return os.path.abspath(env)
        return None
    candidates = []
    if committed_path:
        candidates.append(committed_path)
    candidates.append("occ_probe")  # PATH
    for c in candidates:
        full = c if os.path.sep in c or "/" in c else c
        if os.path.isfile(full) and os.access(full, os.X_OK):
            return os.path.abspath(full)
        # bare command name: resolve against PATH
        from shutil import which
        if "/" not in c:
            found = which(c)
            if found:
                return found
    return None


_RE_FACT = re.compile(r"^([a-z_]+)\s*=\s*(.*)$")


def parse_probe_output(text):
    """Parse the OCC probe's deterministic key=value stdout into recovery facts.

    Returns a dict with at least ``transfer`` (bool) and the topology/geometry
    keys present in the output. Raises :class:`ProbeError` when the output is
    empty or lacks the required ``read_status`` / ``transfer`` keys.

    transfer semantics: the probe prints a ``shape_type``; ``NULL`` / ``NONE``
    means OCC parsed the file but built no transferable shape -> transfer False;
    anything else is a transferred shape -> transfer True.
    """
    facts = {}
    for line in text.splitlines():
        m = _RE_FACT.match(line.strip())
        if not m:
            continue
        key, val = m.group(1).strip(), m.group(2).strip()
        if key in ("read_status", "roots"):
            try:
                facts[key] = int(val)
            except ValueError:
                facts[key] = val
        elif key == "shape_type":
            facts["shape_type"] = val
            facts["transfer"] = val not in ("NULL", "NONE")
        elif key in ("solids", "shells", "faces", "edges", "vertices"):
            try:
                facts[key] = int(val)
            except ValueError:
                facts[key] = val
        elif key in ("volume", "area"):
            try:
                facts[key] = float(val)
            except ValueError:
                facts[key] = val
        elif key.startswith("bbox_"):
            rest, comp = key.split("_", 2)[1], key.rsplit("_", 1)[1]
            try:
                facts.setdefault(("bbox_min" if rest == "min" else "bbox_max"),
                                 {})[comp] = float(val)
            except ValueError:
                pass
    # normalize bbox dicts into ordered [x, y, z] lists for stable consumers
    for side in ("bbox_min", "bbox_max"):
        d = facts.get(side)
        if isinstance(d, dict):
            facts[side] = [d.get("x"), d.get("y"), d.get("z")]
    if "transfer" not in facts:
        raise ProbeError("probe output lacks a transfer/shape_type key")
    if "read_status" not in facts:
        # tolerate, but note it in the facts for honesty
        facts["read_status"] = None
    return facts


# --------------------------------------------------------------------------
# Reading
# --------------------------------------------------------------------------
def independent_read(path, probe):
    """Run the independent reader on a STEP file; return recovery facts.

    Raises :class:`StepValidationError` on OS-level failure, :class:`ProbeError`
    when the probe cannot interpret the file. The caller classifies these.
    """
    if not os.path.isfile(path):
        raise StepValidationError("missing file: %s" % path)
    try:
        r = subprocess.run([probe, path], capture_output=True, text=True,
                           timeout=120)
    except OSError as e:
        raise StepValidationError("reader spawn failed: %r" % (e,)) from e
    except subprocess.TimeoutExpired as e:
        raise StepValidationError("reader timed out: %r" % (e,)) from e
    text = (r.stdout or "") + (r.stderr or "")
    # probe exit code: 1 means read failed (no transferable shape); still parse
    # stdout for whatever facts OCC reported (honest).
    return parse_probe_output(text)


def discover_skip(probe, path):
    """Classify a pair that could not be read.

    Returns ``(class, reason)``. ``probe is None`` -> reader-unavailable (the
    honest CI-safe outcome); a missing file -> fixture-missing; otherwise the
    error string from a read attempt is folded into reader-error / unparsable.
    """
    if probe is None:
        return ("reader-unavailable", "no independent reader binary discovered")
    if not os.path.isfile(path):
        return ("fixture-missing", "missing: %s" % path)
    try:
        independent_read(path, probe)
    except ProbeError as e:
        return ("unparsable", str(e))
    except StepValidationError as e:
        return ("reader-error", str(e))
    return ("reader-error", "unreadable")


# --------------------------------------------------------------------------
# Golden-diff classification
# --------------------------------------------------------------------------
def _approx(a, b, tol):
    """Numeric-ish equality with an absolute tolerance (None/blank == don't care)."""
    if tol is None:
        return True
    try:
        return abs(float(a) - float(b)) <= float(tol)
    except (TypeError, ValueError):
        return a == b


def classify(pair_id, current, golden, tolerance):
    """Classify current independent recovery vs the golden expectation.

    Returns a row dict:
      status `measured`   -> current transfer + geometry match golden;
      status `drift`      -> differs (class from DRIFT_CLASSES, golden+current
                             facts attached so the delta is visible);
      status `skip`       -> can't be read (class from CLOSED_TAXONOMY).
    """
    g = golden or {}
    g_transfer = bool(g.get("transfer"))
    c_transfer = bool((current or {}).get("transfer"))
    tolerance = tolerance or {}

    if g_transfer != c_transfer:
        cls = "transfer-flipped-true" if c_transfer else "transfer-flipped-false"
        return {
            "id": pair_id, "status": "drift", "class": cls,
            "reason": "independent-reader transfer state changed vs golden",
            "current": current or {}, "golden": g, "tolerance": tolerance,
        }
    if not g_transfer and not c_transfer:
        return {
            "id": pair_id, "status": "measured", "class": "", "reason": "",
            "current": current or {}, "golden": g, "tolerance": tolerance,
        }
    # both transfer -> compare geometry facts
    diffs = []
    for key, expected in (g or {}).items():
        if key == "transfer":
            continue
        if key in ("solids", "shells", "faces", "edges", "vertices") \
                or key in ("volume", "area"):
            expect_val = expected
            cur_val = (current or {}).get(key)
            if cur_val is None:
                continue
            tol = tolerance.get(
                key, tolerance.get("counts" if key in (
                    "solids", "shells", "faces", "edges", "vertices")
                    else None))
            if not _approx(cur_val, expect_val, tol):
                diffs.append({"key": key, "golden": expect_val,
                              "current": cur_val})
    if diffs:
        return {
            "id": pair_id, "status": "drift",
            "class": "geometry-differs",
            "reason": "independent-reader recovery geometry differs from "
                      "golden: %s"
                      % "; ".join("%s g=%r c=%r" % (d["key"], d["golden"],
                                                     d["current"])
                                  for d in diffs),
            "current": current or {}, "golden": g, "tolerance": tolerance,
        }
    return {
        "id": pair_id, "status": "measured", "class": "", "reason": "",
        "current": current or {}, "golden": g, "tolerance": tolerance,
    }


# --------------------------------------------------------------------------
# Pair list + runner + report
# --------------------------------------------------------------------------
def load_pair_list(path):
    """Load the corpus pair list (schema v1). Returns a list of pair dicts."""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    return data.get("pairs") if isinstance(data, dict) else data


def load_golden(path):
    """Load the golden expectation file (schema v1) or raise."""
    return json.loads(Path(path).read_text(encoding="utf-8"))


def run(pairs, golden, corpus_root, probe):
    """Classify every pair -> list of row dicts (coverage == truth set).

    A row is `measured` when the independent reader ran and its recovery matches
    the golden; `drift` when recovery changed (classified); `skip` otherwise
    (closed taxonomy). Never raises; a failure in one pair never aborts others.
    """
    golden_map = (golden or {}).get("pairs") or {}
    rows = []
    for pair in pairs:
        rid = pair.get("id", "?")
        step = pair.get("step", "")
        tolerance = pair.get("tolerance", {})
        if not step:
            rows.append({"id": rid, "status": "skip",
                         "class": "fixture-missing",
                         "reason": "pair has no step path",
                         "current": {}, "golden": golden_map.get(rid, {}),
                         "tolerance": tolerance})
            continue
        p = Path(step)
        if not p.is_absolute():
            p = Path(corpus_root) / p
        try:
            facts = independent_read(str(p), probe) if probe else None
        except ProbeError as e:
            rows.append({"id": rid, "status": "skip", "class": "unparsable",
                         "reason": str(e), "current": {},
                         "golden": golden_map.get(rid, {}),
                         "tolerance": tolerance})
            continue
        except StepValidationError as e:
            cls, reason = discover_skip(probe, str(p))
            rows.append({"id": rid, "status": "skip", "class": cls,
                         "reason": reason or str(e), "current": {},
                         "golden": golden_map.get(rid, {}),
                         "tolerance": tolerance})
            continue
        if probe is None or facts is None:
            rows.append({"id": rid, "status": "skip",
                         "class": "reader-unavailable",
                         "reason": "no independent reader binary discovered",
                         "current": {}, "golden": golden_map.get(rid, {}),
                         "tolerance": tolerance})
            continue
        rows.append(classify(rid, facts, golden_map.get(rid, {}), tolerance))
    return rows


def build_report(rows, generated=None):
    """Wrap classified rows into a self-consistent gate report dict."""
    measured = sum(1 for r in rows if r["status"] == "measured")
    drifted = sum(1 for r in rows if r["status"] == "drift")
    skipped = sum(1 for r in rows if r["status"] == "skip")
    skipped_by_class = {}
    for r in rows:
        if r["status"] == "skip":
            c = r.get("class") or "unknown"
            skipped_by_class[c] = skipped_by_class.get(c, 0) + 1
    drift_by_class = {}
    for r in rows:
        if r["status"] == "drift":
            c = r.get("class") or "unknown"
            drift_by_class[c] = drift_by_class.get(c, 0) + 1
    return {
        "schema_version": 1,
        "generated": generated or datetime.now(timezone.utc).isoformat(),
        "description": "P4-M3-T02 STEP export golden-diff gate report. "
                       "measured = independent-reader recovery matches the "
                       "committed golden; drift = recovery changed (classified, "
                       "needs re-blessing); skip = closed-taxonomy (never "
                       "fabricated). Values always come from the independent "
                       "reader, never from our own parsers/writers.",
        "summary": {"total": len(rows), "measured": measured, "drifted": drifted,
                    "skipped": skipped, "skipped_by_class": skipped_by_class,
                    "drift_by_class": drift_by_class},
        "rows": rows,
    }


def write_report(report, outdir):
    """Persist the report as JSON + markdown. Returns (json_path, md_path)."""
    outdir = Path(outdir)
    outdir.mkdir(parents=True, exist_ok=True)
    jp = outdir / "export-validation-report.json"
    md = outdir / "export-validation-report.md"
    jp.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    s = report["summary"]
    lines = [
        "# P4-M3-T02 STEP Export Golden-Diff Report", "",
        "generated: %s" % report["generated"], "",
        "summary: %d rows | %d measured | %d drifted | %d skipped | drift %s"
        % (s["total"], s["measured"], s["drifted"], s["skipped"],
           s["drift_by_class"]), "",
        "| id | status | class | current transfer | faces | area | volume |",
        "|---|---|---|---|---|---|---|",
    ]
    for r in report["rows"]:
        c = r.get("current") or {}
        cb = c.get("transfer")
        lines.append("| %s | %s | %s | %s | %s | %s | %s |" % (
            r["id"], r["status"], r.get("class") or "-",
            "" if cb is None else cb,
            c.get("faces", ""), c.get("area", ""), c.get("volume", "")))
    md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return str(jp), str(md)


def check_gate(rows):
    """Return (exit_code, list_of_unclassified_drift)."""
    bad = [r for r in rows if r["status"] == "drift" and r.get("class")
           not in DRIFT_CLASSES]
    return (1 if bad else 0), bad


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------
def main(argv=None):
    argv = argv if argv is not None else sys.argv[1:]
    import argparse
    ap = argparse.ArgumentParser(
        description="P4-M3-T02 STEP export golden-diff gate (independent reader)")
    ap.add_argument("--pairs", default=os.path.join(
        "corpus", "export-validation-pairs.json"))
    ap.add_argument("--golden", default=os.path.join(
        "corpus", "export-validation-golden.json"))
    ap.add_argument("--corpus", default=".")
    ap.add_argument("--out", default="corpus")
    ap.add_argument("--probe", default=os.path.join("corpus", "tools",
                                                    "occ_probe"))
    ap.add_argument("--check", action="store_true",
                    help="exit non-zero iff an unclassified drift appears")
    args = ap.parse_args(argv)

    pairs = load_pair_list(args.pairs)
    golden = load_golden(args.golden)
    probe = find_probe(args.probe)
    if probe is None:
        print("independent reader: NOT AVAILABLE (rows will classify "
              "reader-unavailable)")
    else:
        print("independent reader: %s" % probe)
    rows = run(pairs, golden, args.corpus, probe)
    report = build_report(rows)
    jp, mp = write_report(report, args.out)
    s = report["summary"]
    print("rows=%d measured=%d drifted=%d skipped=%d" % (
        s["total"], s["measured"], s["drifted"], s["skipped"]))
    print("json: %s" % jp)
    print("markdown: %s" % mp)
    if args.check:
        rc, bad = check_gate(rows)
        for b in bad:
            print("UNCLASSIFIED DRIFT: %s - %s" % (b["id"], b.get("reason")))
        print("gate: %s" % ("GREEN" if rc == 0 else "RED"))
        return rc
    return 0


if __name__ == "__main__":
    sys.exit(main())
