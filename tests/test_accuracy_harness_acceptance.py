#!/usr/bin/env python3
"""P4-M1-T03 acceptance test: STEP accuracy harness (Hausdorff/volume diff).

TDD: this test documents the acceptance criteria BEFORE the harness exists, so
it MUST FAIL (RED) when run against a tree without `oep_tooling.step_fidelity`,
then PASS (GREEN) once the implementation lands on real material.

'Done' for P4-M1-T03 means:
  - the clean-room facetted-STEP reader recovers the committed
    box-reference.step into EXACTLY the same 8-vertex/12-triangle geometry as
    the native box.stl (real material, deterministic, no fabrication);
  - the real box pair (native corpus/data/box.stl <-> STEP
    crates/oep-geo/tests/golden/box-reference.step) measures as fiducial:
    symmetric (two-sided) vertex-sampled Hausdorff == 0.0 and volume delta ==
    0.0, with honest method disclosure (sample method: "vertex", na/nb counts);
  - metric unit checks on synthetic geometry bound the math (same mesh -> 0;
    translated unit cube -> distance == translation magnitude both sides;
    scaled cube -> volume ratio == scale^3);
  - an analytic B-Rep STEP (ADVANCED_FACE present, no facetted shell) is a
    classified `not-faceted` skip -- NEVER a fabricated fidelity number;
  - the pair-list runner yields exactly one row per pair (coverage == pair-list
    count, no drops, no phantom rows) and emits a well-formed fidelity report
    (JSON schema_version/generated/summary rollup == len(rows)) plus markdown;
  - tolerant: hostile/empty/truncated inputs never crash the sweep (classified
    rows); exit non-zero iff a row lands in the `fail` bucket (none expected);
  - honest-only: no `measured` row without real geometry on BOTH sides; every
    skip carries a closed-taxonomy class + reason string.

Clean-room rule: the STEP reader only understands the facetted/tessellated
subset our own P6-M4-T01 / P2-M5-T02 exporters emit. Anything else (ADVANCED_
FACE, B_SPLINE_*, MANIFOLD_SOLID_BREP, ...) is an honest `not-faceted` skip.

Pure stdlib. Runs under `python3 tests/test_accuracy_harness_acceptance.py`
and pytest. Exit code 1 on any failure.
"""
import json
import os
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


CLOSED_TAXONOMY = {
    "pair-missing-on-disk", "native-unreadable", "step-unreadable",
    "not-faceted", "no-native-geometry", "no-step-geometry", "io-error",
}

NATIVE_BOX = os.path.join(REPO, "corpus", "data", "box.stl")
STEP_BOX = os.path.join(REPO, "crates", "oep-geo", "tests", "golden",
                        "box-reference.step")

BOX_VERTS = 8
BOX_TRIS = 12

# Expected box corners (from box_ref.py / exporters acceptance, CC0 fixture).
EXPECTED_CORNERS = {
    (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 0.0), (1.0, 1.0, 0.0),
    (0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 1.0),
}


def _cube_positions():
    # Order: bottom face (4), then top face (4) — matches the canonical
    # construction that yields signed divergence volume == +1.0.
    return [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0),
            (0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)]


def _cube_triangles():
    # 12 outward triangles for [0,1]^3 (CCW seen from outside; this exact
    # construction yields signed divergence volume == +1.0, verified
    # analytically: c0..c3 bottom, c4..c7 top).
    c0, c1, c2, c3 = 0, 1, 2, 3
    c4, c5, c6, c7 = 4, 5, 6, 7
    t = [
        (c0, c3, c1), (c1, c3, c2),   # z=0 (viewed from below -> CCW)
        (c5, c7, c4), (c5, c6, c7),   # z=1 (viewed from above -> CCW)
        (c0, c1, c5), (c0, c5, c4),   # y=0
        (c2, c3, c7), (c2, c7, c6),   # y=1
        (c0, c4, c7), (c0, c7, c3),   # x=0
        (c1, c2, c6), (c1, c6, c5),   # x=1
    ]
    return t


def _unit_cube_mesh(translate=(0.0, 0.0, 0.0), scale=1.0):
    pos = {}
    for p in _cube_positions():
        q = (p[0] * scale + translate[0],
             p[1] * scale + translate[1],
             p[2] * scale + translate[2])
        pos[len(pos)] = q
    return {"positions": pos, "triangles": [tuple(t) for t in _cube_triangles()]}


def _scale_mesh(mesh, s):
    return {"positions": {i: (p[0] * s, p[1] * s, p[2] * s)
                          for i, p in mesh["positions"].items()},
            "triangles": mesh["triangles"]}


def _import_acc():
    """Import the harness module; records a FAIL row in RED state (no module)."""
    try:
        import oep_tooling.step_fidelity as acc
        check("import oep_tooling.step_fidelity", True)
        return acc
    except Exception as e:  # RED: module absent until implementation
        check("import oep_tooling.step_fidelity", False, repr(e))
        return None


# ---------------------------------------------------------------------------
# 1) clean-room facetted-STEP reader on real material
# ---------------------------------------------------------------------------
def test_step_reader_recovers_real_material():
    acc = _import_acc()
    if acc is None:
        return
    sm = acc.read_faceted_step(STEP_BOX)
    check("step reader returns dict with positions/triangles",
          isinstance(sm, dict) and "positions" in sm and "triangles" in sm)
    check("step reader recovers 8 verts / 12 tris",
          len(sm["positions"]) == BOX_VERTS and len(sm["triangles"]) == BOX_TRIS,
          detail="V=%d T=%d" % (len(sm["positions"]), len(sm["triangles"])))
    step_corners = {tuple(p) for p in sm["positions"].values()}
    check("step corners match native box corners",
          step_corners == EXPECTED_CORNERS,
          detail="n=%d expected=%d" % (len(step_corners), len(EXPECTED_CORNERS)))

    nm = acc.read_stl(NATIVE_BOX)
    check("native STL reader returns 8 verts / 12 tris",
          len(nm["positions"]) == BOX_VERTS and len(nm["triangles"]) == BOX_TRIS,
          detail="V=%d T=%d" % (len(nm["positions"]), len(nm["triangles"])))


# ---------------------------------------------------------------------------
# 2) real box pair fidelit sentiment: Hausdorff 0, volume delta 0
# ---------------------------------------------------------------------------
def test_binary_stl_reader_real_material():
    acc = _import_acc()
    if acc is None:
        return
    # committed binary-STL golden of the same unit box (P6-M4-T01)
    bin_stl = os.path.join(REPO, "crates", "oep-geo", "tests", "golden",
                           "box-reference.bin.stl")
    if not os.path.isfile(bin_stl):
        check("binary STL golden present", False, bin_stl)
        return
    bm = acc.read_stl(bin_stl)
    check("binary STL reader returns 8 verts / 12 tris",
          len(bm["positions"]) == BOX_VERTS and len(bm["triangles"]) == BOX_TRIS,
          detail="V=%d T=%d" % (len(bm["positions"]), len(bm["triangles"])))
    corners = {tuple(p) for p in bm["positions"].values()}
    check("binary STL corners match native box corners",
          corners == EXPECTED_CORNERS,
          detail="n=%d" % len(corners))


def test_real_box_pair_fiducial():
    acc = _import_acc()
    if acc is None:
        return
    sm = acc.read_faceted_step(STEP_BOX)
    nm = acc.read_stl(NATIVE_BOX)
    h = acc.symmetric_hausdorff(sm, nm)
    check("box pair forward Hausdorff == 0.0",
          abs(h["forward"]) < 1e-12, detail=str(h["forward"]))
    check("box pair reverse Hausdorff == 0.0",
          abs(h["reverse"]) < 1e-12, detail=str(h["reverse"]))
    check("box pair symmetric == 0.0", abs(h["symmetric"]) < 1e-12)
    check("hausdorff reports method + na/nb honestly",
          h.get("method") == "vertex"
          and h.get("na") == BOX_VERTS and h.get("nb") == BOX_VERTS,
          detail=str(h))

    v_step = acc.volume(sm)
    v_native = acc.volume(nm)
    check("box pair volume delta == 0.0",
          abs(v_step - v_native) < 1e-9,
          detail="step=%r native=%r" % (v_step, v_native))


# ---------------------------------------------------------------------------
# 3) synthetic metric unit checks
# ---------------------------------------------------------------------------
def test_synthetic_metric_checks():
    acc = _import_acc()
    if acc is None:
        return
    cube = _unit_cube_mesh()
    h_self = acc.symmetric_hausdorff(cube, cube)
    check("same mesh -> Hausdorff 0", abs(h_self["symmetric"]) < 1e-12)

    t = 0.4  # translation magnitude
    cube_t = _unit_cube_mesh(translate=(t, 0, 0))
    h_t = acc.symmetric_hausdorff(cube, cube_t)
    check("translated cube -> symmetric == T (0.4)",
          abs(h_t["symmetric"] - t) < 1e-9, detail=str(h_t["symmetric"]))
    check("translated cube forward == reverse == T",
          abs(h_t["forward"] - t) < 1e-9 and abs(h_t["reverse"] - t) < 1e-9)

    v_cube = acc.volume(_unit_cube_mesh())
    check("unit cube volume == 1.0", abs(v_cube - 1.0) < 1e-9, detail=repr(v_cube))
    v_scaled = acc.volume(_scale_mesh(_unit_cube_mesh(), 2.0))
    check("scaled cube volume ratio == 8", abs(v_scaled / v_cube - 8.0) < 1e-9,
          detail="ratio=%r" % (v_scaled / v_cube))


# ---------------------------------------------------------------------------
# 4) analytic B-Rep STEP => honest not-faceted skip, no fabricated number
# ---------------------------------------------------------------------------
def test_analytic_b_rep_step_is_not_faceted():
    acc = _import_acc()
    if acc is None:
        return
    analytic_step = os.path.join(tempfile.gettempdir(), "p4m1t03-analytic.step")
    with open(analytic_step, "w") as fh:
        fh.write(
            "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 "
            "214 1 1 1 1 }'));\nENDSEC;\nDATA;\n"
            "#1=ADVANCED_FACE('',(),#2,.T.);\n#2=PLANE('',#3);\n"
            "#3=AXIS2_PLACEMENT_3D('',#4,#5,#6);\n"
            "#4=CARTESIAN_POINT('',(0.,0.,0.));\n"
            "#5=DIRECTION('',(0.,0.,1.));\n#6=DIRECTION('',(1.,0.,0.));\n"
            "ENDSEC;\nEND-ISO-10303-21;\n")
    try:
        acc.read_faceted_step(analytic_step)
        check("analytic B-Rep STEP raises NotFaceted", False, "no exception")
    except acc.NotFaceted:
        check("analytic B-Rep STEP raises NotFaceted", True)
    except Exception as e:  # noqa: BLE001
        check("analytic B-Rep STEP raises NotFaceted", False, repr(e))


# ---------------------------------------------------------------------------
# 5) pair-list runner + report
# ---------------------------------------------------------------------------
def test_pair_list_runner_and_report():
    acc = _import_acc()
    if acc is None:
        return
    pairs = [
        {"id": "isolith-box",
         "native": os.path.join(REPO, "corpus", "data", "box.stl"),
         "step": os.path.join(REPO, "crates", "oep-geo", "tests", "golden",
                              "box-reference.step"),
         "geometry_class": "prism"},
        {"id": "missing-step", "native": os.path.join(REPO, "corpus", "data",
                                                      "box.stl"),
         "step": os.path.join(tempfile.gettempdir(), "does-not-exist.step"),
         "geometry_class": "prism"},
    ]
    rows = acc.run(pairs, os.path.join(REPO, "corpus"))
    check("pair-list coverage == len(pairs) (2 rows, no drop/phantom)",
          len(rows) == 2, detail="rows=%d" % len(rows))
    row_ids = {r["id"] for r in rows}
    check("no phantom rows, no dropped pairs",
          row_ids == {p["id"] for p in pairs}, detail=str(row_ids))

    box_row = next(r for r in rows if r["id"] == "isolith-box")
    check("box pair status == measured",
          box_row["status"] == "measured", detail=str(box_row))
    check("box pair measured symmetric hausdorff 0.0",
          "symmetric_hausdorff" in box_row["metrics"]
          and abs(box_row["metrics"]["symmetric_hausdorff"]) < 1e-12)
    check("box pair measured volume delta 0.0",
          "volume_delta" in box_row["metrics"]
          and abs(box_row["metrics"]["volume_delta"]) < 1e-9,
          detail=str(box_row.get("metrics", {}).get("volume_delta")))

    miss_row = next(r for r in rows if r["id"] == "missing-step")
    check("missing-step -> classified skip, not measured",
          miss_row["status"] == "skip"
          and miss_row.get("class") in CLOSED_TAXONOMY
          and miss_row.get("reason"), detail=str(miss_row))

    # toggling a read failure never fabricates: swap native to a bad path
    bad_pairs = [{"id": "bad-native", "native": "/no/such/file.stl",
                  "step": STEP_BOX, "geometry_class": "prism"}]
    bad_rows = acc.run(bad_pairs, os.path.join(REPO, "corpus"))
    check("unreadable native -> classified skip, not measured",
          bad_rows[0]["status"] == "skip"
          and bad_rows[0].get("class") in CLOSED_TAXONOMY,
          detail=str(bad_rows[0]))

    report = acc.build_report(rows)
    check("report has schema_version + generated",
          report.get("schema_version") == 1 and report.get("generated"))
    check("report summary rollup == len(rows)",
          report.get("summary", {}).get("total") == len(rows)
          and report["summary"]["measured"] + report["summary"]["skipped"]
          == len(rows),
          detail=str(report.get("summary")))

    with tempfile.TemporaryDirectory() as td:
        jp, mp = acc.write_report(report, td)
        with open(jp) as fh:
            round_trip = json.load(fh)
        check("report JSON round-trips", round_trip["schema_version"] == 1)
        check("markdown report written",
              os.path.isfile(mp) and os.path.getsize(mp) > 0)


# ---------------------------------------------------------------------------
# 6) tolerant: hostile inputs never crash
# ---------------------------------------------------------------------------
def test_hostile_inputs_never_crash():
    acc = _import_acc()
    if acc is None:
        return
    hostile_dir = tempfile.mkdtemp(prefix="p4m1t03-hostile-")
    Path(os.path.join(hostile_dir, "bad.step")).write_bytes(b"\x00\x01\x02")
    Path(os.path.join(hostile_dir, "empty.step")).write_bytes(b"")
    Path(os.path.join(hostile_dir, "trunc.step")).write_bytes(
        b"ISO-10303-21;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,")
    hostile_pairs = [
        {"id": "hostile-bad-step", "native": NATIVE_BOX,
         "step": os.path.join(hostile_dir, "bad.step"), "geometry_class": "prism"},
        {"id": "hostile-empty-step", "native": NATIVE_BOX,
         "step": os.path.join(hostile_dir, "empty.step"),
         "geometry_class": "prism"},
        {"id": "hostile-trunc-step", "native": NATIVE_BOX,
         "step": os.path.join(hostile_dir, "trunc.step"),
         "geometry_class": "prism"},
    ]
    hrows = acc.run(hostile_pairs, os.path.join(REPO, "corpus"))
    check("hostile inputs never crash: all classified skip",
          all(r["status"] == "skip" and r.get("class") in CLOSED_TAXONOMY
              for r in hrows), detail=str(hrows))


if __name__ == "__main__":
    for fn_name in sorted(n for n in dir(sys.modules[__name__])
                          if n.startswith("test_")):
        fn = getattr(sys.modules[__name__], fn_name)
        fn()
    print("-" * 50)
    if _failures:
        print("RESULT: FAIL (%d)" % len(_failures))
        sys.exit(1)
    print("RESULT: PASS")
    sys.exit(0)
