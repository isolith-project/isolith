#!/usr/bin/env python3
"""P5-M3-T02 acceptance test: geometry accuracy harness vs external converter
reference (test-only ground truth).

TDD: this test documents the acceptance criteria BEFORE the harness exists, so
it MUST FAIL (RED) when run against a tree without `oep_tooling.native_accuracy`,
then PASS (GREEN) once the implementation lands on real material.

'Done' for P5-M3-T02 means:
  - a corpus pair list exists (`corpus/native-accuracy-pairs.json`, schema v1)
    mapping native sample -> external-converter reference (the same native file
    read by an independent converter, e.g. FreeCAD/OCC via `corpus/reference/`);
  - the clean-room native STL reader recovers the committed box.stl into EXACTLY
    the same 8-vertex / 12-triangle geometry as the committed FreeCAD reference
    (real material, deterministic, no fabrication);
  - the real box pair (native corpus/data/box.stl <-> external-converter
    reference corpus/reference/box-reference-freecad.json) measures as fiducial:
    symmetric (two-sided) vertex-sampled Hausdorff == 0.0, volume delta == 0.0,
    and a tolerance DISTRIBUTION that is 100% inside the tightest bin (the
    acceptance asks for a tolerance distribution, not a single number);
  - the harness reports the tolerance distribution honestly: per-vertex distance
    percentiles (p50/p90/p99/p100) plus the fraction of sampled vertices inside
    each epsilon bin (relative-to-bbox thresholds), with method disclosure;
  - metric unit checks on synthetic geometry bound the math (same mesh -> 0;
    translated unit cube -> every distance == translation magnitude both sides;
    scaled cube -> volume ratio == scale^3);
  - a real native CATPart (committed corpus/data/catpart/*.CATPart) with no
    external-converter reference yet is a classified `reference-unavailable`
    skip -- NEVER a fabricated fidelity number;
  - the pair-list runner yields exactly one row per pair (coverage == pair-list
    count, no drops, no phantom rows) and emits a well-formed report
    (JSON schema_version/generated/summary rollup == len(rows)) plus markdown;
  - tolerant: hostile/empty/truncated inputs never crash the sweep (classified
    rows); exit non-zero iff a row lands in the `fail` bucket (none expected);
  - honest-only: no `measured` row without real geometry on BOTH sides; every
    skip carries a closed-taxonomy class + reason string.

Clean-room rule: the reference file is the OUTPUT of an external, independent
converter (FreeCAD / Open CASCADE kernel via the committed generator) reading the
same native sample -- used STRICTLY as test-only ground truth per P5-M1-T03 §6,
never as a source of format internals, never shipped.

Pure stdlib. Runs under `python3 tests/test_native_accuracy_harness_acceptance.py`
and pytest. Exit code 1 on any failure.
"""
import json
import os
import sys
import tempfile
from pathlib import Path

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

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
    "pair-missing-on-disk", "native-unreadable", "reference-unreadable",
    "reference-unavailable", "no-native-geometry", "no-reference-geometry",
    "not-a-mesh", "io-error",
}

NATIVE_BOX = os.path.join(REPO, "corpus", "data", "box.stl")
REF_BOX = os.path.join(REPO, "corpus", "reference", "box-reference-freecad.json")
NATIVE_CATPART = os.path.join(REPO, "corpus", "data", "catpart", "hex-nut.CATPart")

BOX_VERTS = 8
BOX_TRIS = 12

EXPECTED_CORNERS = {
    (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 0.0), (1.0, 1.0, 0.0),
    (0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 1.0),
}


def _cube_positions():
    return [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0),
            (0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)]


def _cube_triangles():
    c0, c1, c2, c3 = 0, 1, 2, 3
    c4, c5, c6, c7 = 4, 5, 6, 7
    return [
        (c0, c3, c1), (c1, c3, c2), (c5, c7, c4), (c5, c6, c7),
        (c0, c1, c5), (c0, c5, c4), (c2, c3, c7), (c2, c7, c6),
        (c0, c4, c7), (c0, c7, c3), (c1, c2, c6), (c1, c6, c5),
    ]


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
    try:
        import oep_tooling.native_accuracy as acc
        check("import oep_tooling.native_accuracy", True)
        return acc
    except Exception as e:  # RED: module absent until implementation
        check("import oep_tooling.native_accuracy", False, repr(e))
        return None


# 1) clean-room native STL reader vs committed external-converter reference
def test_native_reader_references_real_material():
    acc = _import_acc()
    if acc is None:
        return
    nm = acc.read_native(NATIVE_BOX)
    check("native STL reader returns 8 verts / 12 tris",
          len(nm["positions"]) == BOX_VERTS and len(nm["triangles"]) == BOX_TRIS,
          detail="V=%d T=%d" % (len(nm["positions"]), len(nm["triangles"])))
    native_corners = {tuple(p) for p in nm["positions"].values()}
    check("native corners match committed box corners",
          native_corners == EXPECTED_CORNERS, detail="n=%d" % len(native_corners))

    rm = acc.read_reference(REF_BOX)
    check("reference reader returns 8 verts / 12 tris",
          len(rm["positions"]) == BOX_VERTS and len(rm["triangles"]) == BOX_TRIS,
          detail="V=%d T=%d" % (len(rm["positions"]), len(rm["triangles"])))
    ref_corners = {tuple(p) for p in rm["positions"].values()}
    check("reference corners match native box corners",
          ref_corners == EXPECTED_CORNERS, detail="n=%d" % len(ref_corners))


# 2) real box pair: fiducial Hausdorff 0, volume 0, tolerance distribution 100%
def test_real_box_pair_fiducial():
    acc = _import_acc()
    if acc is None:
        return
    nm = acc.read_native(NATIVE_BOX)
    rm = acc.read_reference(REF_BOX)
    h = acc.symmetric_hausdorff(nm, rm)
    check("box pair forward Hausdorff == 0.0",
          abs(h["forward"]) < 1e-12, detail=str(h["forward"]))
    check("box pair reverse Hausdorff == 0.0",
          abs(h["reverse"]) < 1e-12, detail=str(h["reverse"]))
    check("box pair symmetric == 0.0", abs(h["symmetric"]) < 1e-12)
    check("hausdorff reports method + na/nb honestly",
          h.get("method") == "vertex"
          and h.get("na") == BOX_VERTS and h.get("nb") == BOX_VERTS,
          detail=str(h))

    v_nat = acc.volume(nm)
    v_ref = acc.volume(rm)
    check("box pair volume delta == 0.0",
          abs(v_nat - v_ref) < 1e-9, detail="native=%r ref=%r" % (v_nat, v_ref))

    tdist = acc.tolerance_distribution(nm, rm)
    check("tolerance distribution reports method + percentiles + bins",
          tdist.get("method") == "vertex"
          and "p50" in tdist and "p99" in tdist and "p100" in tdist
          and "fraction_within" in tdist, detail=str(tdist)[:200])
    check("box pair 100%% inside tightest tolerance bin",
          tdist["fraction_within"][0]["fraction"] == 1.0,
          detail=str(tdist.get("fraction_within")))
    check("box pair p100 == 0.0", abs(tdist.get("p100", 9.0)) < 1e-12)


# 3) synthetic metric unit checks
def test_synthetic_metric_checks():
    acc = _import_acc()
    if acc is None:
        return
    cube = _unit_cube_mesh()
    h_self = acc.symmetric_hausdorff(cube, cube)
    check("same mesh -> Hausdorff 0", abs(h_self["symmetric"]) < 1e-12)

    t = 0.4
    cube_t = _unit_cube_mesh(translate=(t, 0, 0))
    h_t = acc.symmetric_hausdorff(cube, cube_t)
    check("translated cube -> symmetric == T (0.4)",
          abs(h_t["symmetric"] - t) < 1e-9, detail=str(h_t["symmetric"]))
    check("translated cube forward == reverse == T",
          abs(h_t["forward"] - t) < 1e-9 and abs(h_t["reverse"] - t) < 1e-9)

    tdist = acc.tolerance_distribution(cube, cube_t)
    check("tolerance distribution sees translated pair (p100 == 0.4)",
          abs(tdist.get("p100", 9.0) - t) < 1e-9, detail=str(tdist.get("p100")))

    v_cube = acc.volume(_unit_cube_mesh())
    check("unit cube volume == 1.0", abs(v_cube - 1.0) < 1e-9, detail=repr(v_cube))
    v_scaled = acc.volume(_scale_mesh(_unit_cube_mesh(), 2.0))
    check("scaled cube volume ratio == 8",
          abs(v_scaled / v_cube - 8.0) < 1e-9, detail="ratio=%r" % (v_scaled / v_cube))


# 4) real native CATPart (no external-converter reference yet) => honest skip
def test_native_catpart_without_reference_is_honest_skip():
    acc = _import_acc()
    if acc is None:
        return
    if not os.path.isfile(NATIVE_CATPART):
        check("committed native CATPart present", False, NATIVE_CATPART)
        return
    pairs = [{"id": "hex-nut", "geometry_class": "prism",
              "native": NATIVE_CATPART, "reference": ""}]
    rows = acc.run(pairs, os.path.join(REPO, "corpus"))
    check("CATPart-no-reference -> classified skip, not measured",
          len(rows) == 1 and rows[0]["status"] == "skip"
          and rows[0].get("class") in CLOSED_TAXONOMY
          and rows[0].get("class") == "reference-unavailable"
          and rows[0].get("reason"), detail=str(rows[0]))


# 5) pair-list runner + report
def test_pair_list_runner_and_report():
    acc = _import_acc()
    if acc is None:
        return
    pairs = [
        {"id": "isolith-native-box", "geometry_class": "prism",
         "native": NATIVE_BOX, "reference": REF_BOX},
        {"id": "missing-ref", "geometry_class": "prism",
         "native": NATIVE_BOX,
         "reference": os.path.join(tempfile.gettempdir(), "no-such-ref.json")},
    ]
    rows = acc.run(pairs, os.path.join(REPO, "corpus"))
    check("pair-list coverage == len(pairs) (2 rows)", len(rows) == 2,
          detail="rows=%d" % len(rows))
    row_ids = {r["id"] for r in rows}
    check("no phantom rows, no dropped pairs",
          row_ids == {p["id"] for p in pairs}, detail=str(row_ids))

    box_row = next(r for r in rows if r["id"] == "isolith-native-box")
    check("box pair status == measured",
          box_row["status"] == "measured", detail=str(box_row.get("status")))
    m = box_row.get("metrics") or {}
    check("box pair measured symmetric hausdorff 0.0",
          abs(m.get("symmetric_hausdorff", 9.0)) < 1e-12)
    check("box pair measured volume delta 0.0",
          abs(m.get("volume_delta", 9.0)) < 1e-9)
    check("measured row carries the tolerance distribution",
          m.get("tolerance_distribution")
          and m["tolerance_distribution"].get("fraction_within"))

    miss_row = next(r for r in rows if r["id"] == "missing-ref")
    check("missing reference -> classified skip, not measured",
          miss_row["status"] == "skip"
          and miss_row.get("class") in CLOSED_TAXONOMY
          and miss_row.get("reason"), detail=str(miss_row))

    report = acc.build_report(rows)
    check("report has schema_version + generated",
          report.get("schema_version") == 1 and report.get("generated"))
    check("report summary rollup == len(rows)",
          report.get("summary", {}).get("total") == len(rows)
          and report["summary"]["measured"] + report["summary"]["skipped"]
          == len(rows), detail=str(report.get("summary")))

    with tempfile.TemporaryDirectory() as td:
        jp, mp = acc.write_report(report, td)
        with open(jp) as fh:
            rt = json.load(fh)
        check("report JSON round-trips", rt["schema_version"] == 1)
        check("markdown report written",
              os.path.isfile(mp) and os.path.getsize(mp) > 0)


# 6) tolerant: hostile inputs never crash
def test_hostile_inputs_never_crash():
    acc = _import_acc()
    if acc is None:
        return
    hostile_dir = tempfile.mkdtemp(prefix="p5m3t02-hostile-")
    Path(os.path.join(hostile_dir, "bad.stl")).write_bytes(b"\xff\xfe\x00\x01")
    Path(os.path.join(hostile_dir, "empty.stl")).write_bytes(b"")
    Path(os.path.join(hostile_dir, "trunc.json")).write_bytes(
        b'{"mesh": {"positions": [[0,0,0]]')
    hostile_pairs = [
        {"id": "hostile-bad-native", "native": os.path.join(hostile_dir, "bad.stl"),
         "reference": REF_BOX, "geometry_class": "prism"},
        {"id": "hostile-empty-native", "native": os.path.join(hostile_dir, "empty.stl"),
         "reference": REF_BOX, "geometry_class": "prism"},
        {"id": "hostile-trunc-ref", "native": NATIVE_BOX,
         "reference": os.path.join(hostile_dir, "trunc.json"),
         "geometry_class": "prism"},
    ]
    hrows = acc.run(hostile_pairs, os.path.join(REPO, "corpus"))
    check("hostile inputs never crash: all classified skip",
          all(r["status"] == "skip" and r.get("class") in CLOSED_TAXONOMY
              for r in hrows), detail=str(hrows))


if __name__ == "__main__":
    for fn_name in sorted(n for n in dir(sys.modules[__name__])
                          if n.startswith("test_")):
        getattr(sys.modules[__name__], fn_name)()
    print("-" * 50)
    if _failures:
        print("RESULT: FAIL (%d)" % len(_failures))
        sys.exit(1)
    print("RESULT: PASS")
    sys.exit(0)
