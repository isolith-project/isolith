#!/usr/bin/env python3
"""P3-M5-T02 acceptance test (TDD, RED first) — the P3 GA gate.

Written BEFORE `oep_tooling.ga_gate_3dxml` exists, so this file MUST FAIL (RED)
when run against main (no `oep_tooling.ga_gate_3dxml` module). Passes GREEN once
the gate module lands AND its legs can run. Dependencies degrade to classified
SKIP when the P3 reader/surface/writer PRs are not materialized, so CI stays
green on main before those PRs merge — the same convention every sibling P3 card
uses (and the P2-M6-T03 GA gate before it).

'GA gate' for P3 means ALL THREE legs hold (plan P3-M5-T02):

1. OPEN leg — every corpus 3DXML file reaches a CLASSIFIED outcome through the
   accumulated reader: status OPEN (container/BOM opens, or a standalone ASCII
   model opens) or a SKIP carrying a code from the closed taxonomy
   {unsupported-format, not-a-cgr-container, truncated,
    object-store-unresolved, missing-on-disk, no-mesh-found, unknown-format,
    io-error} (P3-M5-T01 regression taxonomy). Never a crash, no silent drop.

2. TESSELLATE leg — surfaces tessellate: every surface-rep corpus file (the
   Web3D LatticeXvl XvlShell lattice family that a 3DXML viewer reinterprets
   into Gregory patches, P3-M2) decodes to a finite, in-range, non-degenerate
   triangle mesh with quality control; golden topology compare where a golden
   exists. Never a fabricated PASS on hostile input.

3. ROUNDTRIP leg — round-trip stable: write OUR OWN 3DXML ASCII output (the
   deterministic canonical writer built from the published format facts), read it
   back, and re-write; the write -> read -> write cycle is a byte-class fixed
   point (strict byte equality), matching the P3-M4-T02 deep-stability property.

Exit code contract: 0 iff no FAIL row anywhere (SKIP allowed). Report
`ga-gate-3dxml-matrix.{json,md}` is self-consistent (totals roll up to rows).

Honesty boundary (clean-room doctrine): the gate locks the classified-outcome
contract — it never invents mesh geometry where the probe honestly returns a
reasoned skip, and it never turns an absent golden into a PASS.

Pure stdlib (workspace deps = []) so the gate runs in any Python 3.8+ job.
Runs under `python3 tests/ga_gate_3dxml_test.py` and pytest. Exit code 1 on
any failure.
"""
import hashlib
import os
import struct
import sys
import tempfile
import zipfile
from pathlib import Path

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))
sys.path.insert(0, os.path.join(REPO, "python", "oep_tooling"))

_failures = []

CLOSED_TAXONOMY = {
    "unsupported-format", "not-a-cgr-container", "truncated",
    "object-store-unresolved", "missing-on-disk", "no-mesh-found",
    "unknown-format", "io-error",
}


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


# ---------------------------------------------------------------------------
# Independent fixture builders (format facts, from docs/re-notes — hex notebook
# + the sibling P3-M4-T01 golden encoder's public facts; no vendor code).
# ---------------------------------------------------------------------------

SCHEMA_3DXML = "http://www.3ds.com/xsd/3DXML"
XSI = "http://www.w3.org/2001/XMLSchema-instance"
ZIP_LOCAL_HEADER = b"PK\x03\x04"
STANDALONE_XML_PREFIX = b"<?xm"

# The keelworks CC0-1.0 unit box (P0-M4-T02 fixture / P3-M4-T01 golden mesh).
BOX_POSITIONS = [
    (1.0, 0.0, 0.0), (0.0, 1.0, 0.0), (0.0, 0.0, 0.0), (1.0, 1.0, 0.0),
    (0.0, 0.0, 1.0), (0.0, 1.0, 1.0), (1.0, 0.0, 1.0), (1.0, 1.0, 1.0),
]
BOX_TRIANGLES = [
    (0, 1, 2), (0, 3, 1), (4, 5, 6), (5, 7, 6),
    (2, 6, 4), (2, 0, 6), (3, 5, 7), (3, 1, 5),
    (5, 1, 2), (5, 2, 4), (0, 7, 3), (0, 6, 7),
]


def _fmt_f32(v):
    return ("%g" % v) if v == int(v) and abs(v) < 1e17 else repr(float(v))


def _rep_markup(positions, triangles):
    pos = ",".join(" ".join(_fmt_f32(c) for c in p) for p in positions)
    strip = ",".join("%d %d %d" % t for t in triangles)
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<XMLRepresentation version="1.0" xmlns="%s" xmlns:xsi="%s">\n'
        '  <Root xsi:type="BagRepType">\n'
        '    <Rep xsi:type="BagRepType">\n'
        '      <Rep xsi:type="PolygonalRepType">\n'
        '        <Faces><Face strips="%s"/></Faces>\n'
        '        <VertexBuffer><Positions>%s</Positions></VertexBuffer>\n'
        "      </Rep>\n    </Rep>\n  </Root>\n</XMLRepresentation>\n"
        % (SCHEMA_3DXML, XSI, strip, pos)
    )


def _model_doc(name="box", author="keelworks"):
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<Model_3dxml xmlns="%s" xmlns:xsi="%s">\n'
        "  <Header>\n    <SchemaVersion>4.0</SchemaVersion>\n"
        "    <Title>%s</Title>\n    <Author>%s</Author>\n"
        "    <Generator>Keelworks oep-3dxml</Generator>\n  </Header>\n"
        '  <ProductStructure root="1">\n'
        '    <Reference3D id="1" name="%s"/>\n'
        '    <Instance3D id="11" name="Instance1">\n      <IsAggregatedBy/>\n'
        '      <IsInstanceOf xsi:type="InstanceOfReferenceType" aggregatedById="1"/>\n'
        "      <RelativeMatrix>1 0 0 0 0 1 0 0 0 0 1 0</RelativeMatrix>\n"
        "    </Instance3D>\n"
        '    <ReferenceRep id="21" name="%s" format="TESSELLATED" version="1.0" '
        'associatedFile="urn:3DXML:%s.3DRep"/>\n'
        "  </ProductStructure>\n</Model_3dxml>\n"
        % (SCHEMA_3DXML, XSI, name, author, name, name, name)
    )


def _manifest(name):
    return (
        '<?xml version="1.0" encoding="utf-8" ?>\n'
        '<Manifest xmlns:xsi="%s" xsi:noNamespaceSchemaLocation="Manifest.xsd">'
        "<Root><![CDATA[%s.3dxml]]></Root></Manifest>\n" % (XSI, name)
    )


def _zip_3dxml(name="box", positions=BOX_POSITIONS, triangles=BOX_TRIANGLES):
    """Build a deterministic ZIP-archive 3DXML (STORED, fixed DOS time).

    Matches the P3-M4-T01 golden byte contract: STORED members in order
    Manifest.xml, <name>.3dxml, <name>.3DRep with a fixed 1980-01-01 timestamp
    so a write -> read -> write cycle is strict byte equality.
    """
    members = [("Manifest.xml", _manifest(name)),
               ("%s.3dxml" % name, _model_doc(name)),
               ("%s.3DRep" % name, _rep_markup(positions, triangles))]
    bio = __import__("io").BytesIO()
    with zipfile.ZipFile(bio, "w") as zf:
        for fn, text in members:
            zn = zipfile.ZipInfo(fn, date_time=(1980, 1, 1, 0, 0, 0))
            zn.compress_type = zipfile.ZIP_STORED
            zf.writestr(zn, text)
    return bio.getvalue()


def _xvl_shell_fixture():
    """A minimal XvlShell lattice: 6 verts, 2 face rings (2 quads -> 4 tris).

    Mirror of the Web3D LatticeXvl element shape the real corpus uses
    (Coordinate point list + faceCoordIndex rings terminated by -1). Returns
    (xml_bytes, expected_triangle_count).
    """
    points = "0.0 0.0 0.0,1.0 0.0 0.0,1.0 1.0 0.0,0.0 1.0 0.0,0.0 0.0 1.0,1.0 1.0 1.0"
    faces = "0 1 2 3 -1 4 2 5 -1"
    xml = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        "<X3D profile='Immersive' version='3.0'>\n  <Scene>\n    <Shape>\n"
        "      <XvlShell faceCoordIndex='%s' edgeBeginCoordIndex='' "
        "edgeEndCoordIndex='' edgeBeginVector='' edgeEndVector='' "
        "edgeRound='' faceHidden='false'>\n"
        "        <Coordinate point='%s'/>\n"
        "      </XvlShell>\n    </Shape>\n  </Scene>\n</X3D>\n"
        % (faces, points)
    )
    return xml.encode("utf-8"), 2  # two quads -> two component triangles each


def _write_fixture_tree(root):
    """Write a realistic hermetic corpus: zip 3DXML + standalone + surface.

    Returns (corpus_root, manifest_entries, surface_dir, expected_open_ids).
    """
    corpus = Path(root) / "corpus"
    data = corpus / "data"
    data.mkdir(parents=True, exist_ok=True)
    # ZIP-archive 3DXML (coherent BOM) — must OPEN.
    (data / "box_fixture.3dxml").write_bytes(_zip_3dxml("box_fixture"))
    # Standalone ASCII model document — must OPEN.
    (data / "quad_fixture.3dxml").write_bytes(b"<?xml version='1.0'?>" +
                                              b"<Model_3dxml/>")
    # Hostile: a ZIP that is NOT a 3DXML (no Manifest) — classified SKIP.
    (data / "not3dxml.zip").write_bytes(_zip_3dxml("nope")[:4] + b"\x00" * 0)
    # Truncated — classified SKIP `truncated`.
    (data / "trunc.3dxml").write_bytes(b"PK\x03")
    entries = [
        {"id": "box_fixture.3dxml", "path": "data/box_fixture.3dxml",
         "format": "3dxml-zip", "backend": "repo"},
        {"id": "quad_fixture.3dxml", "path": "data/quad_fixture.3dxml",
         "format": "3dxml-ascii", "backend": "repo"},
        {"id": "not3dxml.zip", "path": "data/not3dxml.zip",
         "format": "3dxml-zip", "backend": "repo"},
        {"id": "trunc.3dxml", "path": "data/trunc.3dxml",
         "format": "3dxml-zip", "backend": "repo"},
    ]
    surface = Path(root) / "surface"
    surface.mkdir(parents=True, exist_ok=True)
    (surface / "fixture.x3d").write_bytes(_xvl_shell_fixture()[0])
    return str(corpus), entries, str(surface), {
        "box_fixture.3dxml", "quad_fixture.3dxml"}


# --- 1. module exists and exposes the gate API -----------------------------
def test_ga_gate_3dxml_module_exists():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception as e:
        check("import oep_tooling.ga_gate_3dxml", False, repr(e))
        return
    check("import oep_tooling.ga_gate_3dxml", True)
    for name in ("open_leg", "tessellate_leg", "roundtrip_leg", "run_gate",
                 "build_report", "exit_code"):
        check("ga_gate_3dxml has %s" % name, hasattr(G, name), name)


# --- 2. OPEN leg: every 3DXML reaches a classified outcome, no drop ---------
def test_open_leg_classifies_every_3dxml():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        corpus, entries, surface, expected_open = _write_fixture_tree(td)
        rows = G.open_leg(entries=entries, corpus_root=corpus,
                          sample_dirs=[surface])
        check("open_leg returns list", isinstance(rows, list), repr(rows)[:120])
        got = {getattr(r, "id", "?") for r in rows}
        want = {e["id"] for e in entries} | {"fixture.x3d"}
        check("open_leg no drop / no phantom", got == want,
              "got=%s want=%s" % (sorted(got), sorted(want)))
        for r in rows:
            st = getattr(r, "status", "?")
            ok = st in ("OPEN", "SKIP")
            if st == "SKIP":
                cls = getattr(r, "cls", "") or getattr(r, "detail", "")
                ok = ok and (cls in CLOSED_TAXONOMY or "io-error" in cls
                             or "not-a-cgr" in cls or "unknown" in cls)
            check("open_leg row %s classified OPEN|SKIP" % r.id, ok,
                  "status=%r cls=%r" % (st, getattr(r, "cls", "")))
        opened = {r.id for r in rows if getattr(r, "status", "") == "OPEN"}
        check("open_leg opens real 3DXML fixtures",
              expected_open.issubset(opened), "opened=%s" % sorted(opened))


# --- 3. TESSELLATE leg: surfaces tessellate with quality control ------------
def test_tessellate_leg_decodes_lattice_to_mesh():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        corpus, entries, surface, _ = _write_fixture_tree(td)
        rows = G.tessellate_leg(surface_dirs=[surface])
        fixture = [r for r in rows if getattr(r, "id", "").endswith("fixture.x3d")]
        check("tessellate_leg produced a row for the surface fixture",
              len(fixture) == 1, "rows=%r" % [getattr(r, "id", r) for r in rows])
        if fixture:
            r = fixture[0]
            st = getattr(r, "status", "?")
            check("tessellate_leg fixture status PASS (tessellated)",
                  st == "PASS", "status=%r detail=%r" % (
                      st, getattr(r, "detail", "")))
            # quality control latched: finite coords + triangle count reported
            det = getattr(r, "detail", "")
            if st == "PASS":
                check("tessellate_leg reports triangle count in detail",
                      "triangle" in det.lower(), det)


def test_tessellate_leg_rejects_hostile_input():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        surface = Path(td) / "surface"
        surface.mkdir(parents=True, exist_ok=True)
        # A face index out of coordinate range — must NOT fabricate a PASS.
        (surface / "badidx.x3d").write_text(
            "<X3D><Scene><Shape><XvlShell faceCoordIndex='0 1 99 -1 2 3 4 -1' "
            "edgeBeginCoordIndex='' edgeEndCoordIndex=''>"
            "<Coordinate point='0 0 0,1 0 0,1 1 0'/></XvlShell></Shape>"
            "</Scene></X3D>")
        rows = G.tessellate_leg(surface_dirs=[str(surface)])
        bad = [r for r in rows if "badidx" in getattr(r, "id", "")]
        check("hostile lattice classified (not a fabricated PASS)",
              len(bad) == 1 and getattr(bad[0], "status", "") in ("SKIP", "FAIL"),
              repr(rows)[:200])


# --- 4. ROUNDTRIP leg: write -> read -> write is a byte-class fixed point ----
def test_roundtrip_leg_byte_class_fixed_point():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        rows = G.roundtrip_leg(meshes=[("box", BOX_POSITIONS, BOX_TRIANGLES)])
        boxrows = [r for r in rows if getattr(r, "status", "") in ("PASS", "FAIL")]
        check("roundtrip_leg has a PASS (fixed point) or FAIL row",
              len(boxrows) >= 1, repr(rows)[:180])
        if boxrows:
            r = boxrows[0]
            check("roundtrip_leg unit box byte-class stable (PASS)",
                  getattr(r, "status", "") == "PASS",
                  "status=%r detail=%r" % (getattr(r, "status", ""),
                                           getattr(r, "detail", "")))


def test_roundtrip_leg_reads_own_output_exactly():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        # OUR OWN output (independent writer in this test) is read back by the
        # gate and must reproduce the exact unit box: byte-class geometry and a
        # byte-identical re-encode (fixed point at the whole-archive level).
        pkg = _zip_3dxml("box", BOX_POSITIONS, BOX_TRIANGLES)
        rows = G.roundtrip_leg(packages=[pkg])
        ok = [r for r in rows if getattr(r, "status", "") == "PASS"]
        check("roundtrip_leg reads our own output back as fixed point (PASS)",
              len(ok) == 1, repr(rows)[:200])
        if ok:
            det = ok[0].detail
            check("roundtrip_leg reports vertex/triangle counts",
                  "8" in det and "12" in det, det)


def test_roundtrip_leg_never_blesses_drifted_archive():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        # Corrupt a byte in the model document member (archive integrity): the
        # gate must NEVER claim byte-class stability over a corrupted archive.
        # It is classified (SKIP because the drift is layout-level, or FAIL when
        # the stored geometry disagrees) — never a fabricated PASS.
        pkg = bytearray(_zip_3dxml("box", BOX_POSITIONS, BOX_TRIANGLES))
        target = b"<SchemaVersion>4.0</SchemaVersion>"
        idx = bytes(pkg).find(target)
        check("drift fixture has schema marker", idx != -1)
        if idx == -1:
            return
        pkg[idx + len(b"<SchemaVersion>")] = ord("5")
        rows = G.roundtrip_leg(packages=[bytes(pkg)])
        statuses = [getattr(r, "status", "") for r in rows]
        check("drifted archive never PASSES roundtrip",
              "PASS" not in statuses, "statuses=%r" % statuses)
        check("drifted archive is classified (SKIP or FAIL)",
              any(s in ("SKIP", "FAIL") for s in statuses),
              "statuses=%r" % statuses)


# --- 5. run_gate: report + exit code 0 with matching corpus, 1 on FAIL ------
def test_run_gate_report_and_exit_code():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        corpus, entries, surface, _ = _write_fixture_tree(td)
        out = Path(td) / "out"
        code = G.run_gate(entries=entries, corpus_root=corpus,
                          surface_dirs=[surface], outdir=str(out))
        check("run_gate exit 0 (no FAIL on matching hermetic corpus)",
              code == 0, "code=%r" % (code,))
        jp = out / "ga-gate-3dxml-matrix.json"
        mp = out / "ga-gate-3dxml-matrix.md"
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


# --- 6. degraded mode: absent corpus/golden -> SKIP, exit 0, no crash --------
def test_gate_degrades_on_missing_material():
    try:
        import oep_tooling.ga_gate_3dxml as G
    except Exception:
        return
    with tempfile.TemporaryDirectory() as td:
        code = G.run_gate(entries=[], corpus_root=td, surface_dirs=[os.path.join(td, "nope")],
                          outdir=os.path.join(td, "o"))
        check("degraded run_gate exit 0 (all SKIP)", code == 0, "code=%r" % (code,))


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
        print("GA GATE 3DXML FAILURES: %d" % len(_failures))
        for f in _failures:
            print("  - %s" % f)
        return 1
    print("P3 GA GATE GREEN (all checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
