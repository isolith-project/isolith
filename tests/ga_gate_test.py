#!/usr/bin/env python3
"""P2-M6-T03 acceptance test (TDD, RED first) — the P2 GA gate.

Written BEFORE `oep_tooling.ga_gate` exists, so this file MUST FAIL (RED) when
run against main (no `oep_tooling.ga_gate` module, no `corpus/golden_tests.json`
on the P0-only base). Passes GREEN once the gate module lands AND its legs can
run (dependencies degrade to classified SKIP when the corpus/golden PRs are not
materialized, so CI stays green on main before the corpus PRs merge — the same
convention every P2/P1 sibling card uses).

'GA gate' for P2 means BOTH of:

1. LOAD LEG — every corpus CGR mesh file reaches a CLASSIFIED outcome through
   the accumulated reader sweep: status OPEN (valid mesh / header opens) or a
   SKIP carrying a code from the regression closed taxonomy; never a crash, no
   file silently dropped. This is the P2-M1-T04 tolerant contract and the
   P2-M5-T03 regression runner's guarantee, extended to "all corpus meshes".

2. GOLDEN LEG — 100% of golden-file tests (the P0-M4-T02 `corpus/golden_tests.json`
   spec + committed goldens) PASS within their numeric tolerance (epsilon): zero
   FAIL rows. When the golden spec / corpus is absent the leg classifies SKIP
   (never FAIL) so the gate does not fabricate a pass nor crash.

Exit code contract: 0 iff no FAIL row anywhere (SKIP allowed, like the golden
runner and regression sweep). The gate's own report (ga-gate-matrix.json / .md)
must be self-consistent (totals roll up to row counts).

Honesty boundary (clean-room, P2-M1-T04/P2-M2-T01 doctrine): the gate locks the
classified-outcome contract for real meshes; it NEVER invents a mesh where the
reader honestly returns a reasoned skip. The RE note records the real-material
transcript (0 meshes latched on the old-era corpus is an honest, documented
expectation, not a gate failure).
"""
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))
sys.path.insert(0, os.path.join(REPO, "python", "oep_tooling"))

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


# Observed CGR header constants (published, clean-room; same shape as
# corpus_regression_test.py / golden_header.rs).
CGR_MAGIC = bytes([0x01, 0x00, 0x00, 0x00])
CGR_LANDSCAPE = 0x10CD_3600
CGR_TEMPLATE = bytes([0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00,
                      0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F])


def cgr_header_shape(landscape=CGR_LANDSCAPE, template=CGR_TEMPLATE,
                     badmagic=False):
    b = bytearray(0x78 + 16)
    b[0:4] = bytes([0x99, 0x00, 0x00, 0x00]) if badmagic else CGR_MAGIC
    b[0x64:0x68] = landscape.to_bytes(4, "little")
    b[0x68:0x78] = template
    return bytes(b)


def find_golden_spec(default="corpus/golden_tests.json"):
    """Locate the golden spec: OEP_GOLDEN_SPEC env, repo default, or ''."""
    env = os.environ.get("OEP_GOLDEN_SPEC", "").strip()
    if env:
        return env
    p = Path(REPO) / default
    return str(p) if p.is_file() else ""


def write_ascii_stl(path, tri_verts):
    """Minimal ASCII-STL writer for fixture generation (independent spec impl)."""
    lines = ["solid gate"]
    for (a, b, c) in tri_verts:
        lines.append("  facet normal 0 0 1")
        lines.append("    outer loop")
        for v in (a, b, c):
            lines.append("    vertex %g %g %g" % (v[0], v[1], v[2]))
        lines.append("    endloop")
        lines.append("  endfacet")
    lines.append("endsolid gate")
    Path(path).write_text("\n".join(lines) + "\n", encoding="utf-8")


def box_facets():
    """Reference unit-box corner set (CC0 fixture convention, 12 tris / 8 verts)."""
    verts = [
        (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 0.0), (1.0, 1.0, 0.0),
        (0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 1.0),
    ]
    idx = [
        [0, 1, 2], [0, 3, 1], [4, 5, 6], [5, 7, 6], [2, 6, 4], [2, 0, 6],
        [3, 5, 7], [3, 1, 5], [5, 1, 2], [5, 2, 4], [0, 7, 3], [0, 6, 7],
    ]
    return [[verts[i] for i in t] for t in idx]


def make_golden_spec(root, tolerance="1e-6"):
    """Write a golden spec + goldens into a temp corpus-like tree.

    Independent spec implementation (not the module under test). Mirrors the
    real P0-M4-T02 spec: a text-compare count test (counts/keywords cannot be
    numeric-tokenized, so the corpus spec uses compare=text) PLUS a pure-numeric
    epsilon test whose output is all floats compared within `tolerance`.
    Returns (spec_path, corpus_root).
    """
    import json as _json
    corpus = Path(root) / "corpus"
    data = corpus / "data"
    golden = corpus / "golden"
    data.mkdir(parents=True, exist_ok=True)
    golden.mkdir(parents=True, exist_ok=True)
    stl = data / "box.stl"
    write_ascii_stl(str(stl), box_facets())
    (golden / "box-triangles.golden").write_text(
        "triangle_count 12\nvertex_count 36\n", encoding="utf-8")
    # numeric golden: only floats (epsilon-comparable); the two coords that the
    # box STL serialization pins (min/max corner magnitudes) -- deterministic.
    (golden / "box-bounds.golden").write_text("0.000000 1.000000\n",
                                              encoding="utf-8")
    count_cmd = (
        "import sys; s=open(sys.argv[-1]).read(); "
        "f=s.count('endfacet'); v=s.count('vertex'); "
        "print('triangle_count %d' % f); print('vertex_count %d' % v)"
    )
    bounds_cmd = (
        "import sys,re; "
        "xs=[float(x) for x in re.findall(r'vertex ([\\-+0-9.eE]+)', "
        "open(sys.argv[-1]).read())]; "
        "print('%.6f %.6f' % (min(xs), max(xs)))"
    )
    spec = {
        "schema_version": 1,
        "tests": [
            {
                "id": "box-triangle-count",
                "corpus_id": "box.stl",
                "command": ["python3", "-c", count_cmd, "{input}"],
                "golden": "corpus/golden/box-triangles.golden",
                "compare": "text",
                "tolerance": 0.0,
            },
            {
                "id": "box-bounds-within-epsilon",
                "corpus_id": "box.stl",
                "command": ["python3", "-c", bounds_cmd, "{input}"],
                "golden": "corpus/golden/box-bounds.golden",
                "compare": "numeric",
                "tolerance": float(tolerance),
            },
        ],
    }
    (corpus / "golden_tests.json").write_text(
        _json.dumps(spec, indent=2) + "\n", encoding="utf-8")
    return str(corpus / "golden_tests.json"), str(corpus)


# --- 1. module exists and exposes the gate API -----------------------------
def test_ga_gate_module_exists():
    try:
        import oep_tooling.ga_gate as G
    except Exception as e:
        check("import oep_tooling.ga_gate", False, repr(e))
        return
    check("import oep_tooling.ga_gate", True)
    for name in ("load_leg", "golden_leg", "run_gate", "build_report",
                 "exit_code"):
        check("ga_gate has %s" % name, hasattr(G, name), name)


# --- 2. golden leg: 100% within epsilon on a real spec, zero FAIL -----------
def test_golden_leg_100_percent_within_epsilon():
    try:
        import oep_tooling.ga_gate as G
    except Exception:
        return  # already reported in test_ga_gate_module_exists
    with tempfile.TemporaryDirectory() as td:
        spec, corpus_root = make_golden_spec(td)
        results = G.golden_leg(spec, corpus_root=corpus_root)
        check("golden_leg returns list", isinstance(results, list),
              repr(results)[:120])
        fails = [r for r in results if getattr(r, "status", "") == "FAIL"]
        passes = [r for r in results if getattr(r, "status", "") == "PASS"]
        check("golden_leg: zero FAIL on matching spec", len(fails) == 0,
              "fails=%d rows=%d" % (len(fails), len(results)))
        check("golden_leg: at least one PASS rows", len(passes) >= 1,
              "passes=%d" % len(passes))


# --- 3. golden leg: mismatch within spec -> FAIL (gate must catch it) ------
def test_golden_leg_detects_mismatch():
    try:
        import oep_tooling.ga_gate as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        spec, corpus_root = make_golden_spec(td)
        # Corrupt the corpus box.stl: change one vertex so counts still match
        # but numeric compare would diverge only if counts change; instead make
        # it a real mismatch by trimming a facet -> count drops.
        stl = Path(corpus_root) / "data" / "box.stl"
        text = stl.read_text()
        stl.write_text(text.replace("endfacet", "", 1))  # 11 facets now
        results = G.golden_leg(spec, corpus_root=corpus_root)
        fails = [r for r in results if getattr(r, "status", "") == "FAIL"]
        check("golden_leg catches corrupted corpus (FAIL row)", len(fails) >= 1,
              "fails=%d rows=%d" % (len(fails), len(results)))


# --- 4. load leg: every CGR reaches a classified outcome, no drop ----------
def test_load_leg_classifies_every_mesh():
    try:
        import oep_tooling.ga_gate as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        corpus = Path(td) / "corpus"
        data = corpus / "data"
        data.mkdir(parents=True, exist_ok=True)
        (data / "FIXTURE.cgr").write_bytes(cgr_header_shape())          # OPEN
        (data / "legacy.cgr").write_bytes(                                # SKIP
            cgr_header_shape(template=bytes(CGR_TEMPLATE[:1] +
                                            bytes([CGR_TEMPLATE[0] ^ 0xFF]) +
                                            CGR_TEMPLATE[2:])))
        entries = [
            {"id": "FIXTURE.cgr", "path": "data/FIXTURE.cgr", "format": "cgr",
             "geometry_class": "prism", "backend": "repo"},
            {"id": "legacy.cgr", "path": "data/legacy.cgr", "format": "cgr",
             "geometry_class": "prism", "backend": "repo"},
        ]
        rows = G.load_leg(entries=entries, corpus_root=str(corpus))
        check("load_leg returns classified rows", isinstance(rows, list) and len(rows) == 2,
              "rows=%s" % repr(rows)[:160])
        for r in rows:
            check("load_leg row %s classified (OPEN|SKIP)" % r.id,
                  getattr(r, "status", "") in ("OPEN", "SKIP"),
                  "status=%r" % getattr(r, "status", None))
        ids = sorted(r.id for r in rows)
        check("load_leg no drop / no phantom", ids == sorted(e["id"] for e in entries),
              "got=%s" % ids)


# --- 5. run_gate: report + exit code 0 with matching corpus, 1 on mismatch --
def test_run_gate_report_and_exit_code():
    try:
        import oep_tooling.ga_gate as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        spec, corpus_root = make_golden_spec(td)
        out = Path(td) / "out"
        code = G.run_gate(spec=spec, corpus_root=corpus_root,
                          sample_dirs=[], outdir=str(out))
        check("run_gate exit 0 when all golden within epsilon (no corpus)",
              code == 0, "code=%r" % (code,))
        jp = out / "ga-gate-matrix.json"
        mp = out / "ga-gate-matrix.md"
        check("gate report json written", jp.is_file())
        check("gate report md written", mp.is_file())
        if jp.is_file():
            import json as _json
            m = _json.load(open(jp))
            summary = m.get("summary", {})
            rows = m.get("rows", [])
            check("report schema_version==1", m.get("schema_version") == 1)
            check("report totals roll up", summary.get("total") == len(rows),
                  "summary.total=%r rows=%d" % (summary.get("total"), len(rows)))


# --- 6. degraded mode: absent spec yields SKIP, never FAIL/no crash --------
def test_gate_degrades_on_missing_spec():
    try:
        import oep_tooling.ga_gate as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        spec = str(Path(td) / "nope" / "golden_tests.json")
        results = G.golden_leg(spec, corpus_root=td)
        check("golden_leg with missing spec returns SKIP rows (no crash)",
              isinstance(results, list) and
              all(getattr(r, "status", "") == "SKIP" for r in results),
              repr(results)[:140])


# --- main: run all test_* functions, exit 1 on any failure ----------------
def main():
    tests = sorted(n for n in globals() if n.startswith("test_") and callable(globals()[n]))
    for t in tests:
        try:
            globals()[t]()
        except Exception as e:
            check(t, False, "exception: %r" % (e,))
    print("--")
    if _failures:
        print("GA GATE FAILURES: %d" % len(_failures))
        for f in _failures:
            print("  - %s" % f)
        return 1
    print("GA GATE GREEN (all checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
