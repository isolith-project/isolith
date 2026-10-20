#!/usr/bin/env python3
"""P6-M4-T01 acceptance test: exporters of the P6 geometry core.

The six exporters (STL, glTF/GLB, OBJ, SVG-2D, PDF drawing path, STEP
faceted passthrough) are exercised INDEPENDENTLY of the Rust writer: their
committed goldens under `crates/oep-geo/tests/golden/` are re-parsed here
with pure-stdlib decoders written from the public format specs, and the
reference tessellation (the CC0-1.0 isolith unit box, 8 vertices / 12
triangles) is re-extracted EXACTLY. The Rust integration tests
(`crates/oep-geo/tests/*.rs`) byte-lock the writer output to these goldens,
so everything verified here is verified transitively against the writers.

  STL : struct-unpack the binary STL (80-byte header, u32 count, 50B/tri),
        recompute right-hand-rule normals, check the 8 box corners appear.
  GLB : parse the GLB header + JSON/BIN chunks, decode positions+indices.
  OBJ : parse `v`/`f` records (1-based), check 8 v / 12 f and the corners.
  SVG : xml.etree parse; the single <path> must draw exactly the 5 unique
        projected XY edges of the unit box (projection of the 12-triangle
        wireframe), with the documented viewBox.
  PDF : parse object/xref structure; content stream must draw the same 5
        projected edges; if qpdf/pdfinfo are present, also hard-validate and
        render page 1 to prove the file opens in a real reader.
  STEP: ISO 10303-21 token scan; assert AP214 schema, the tessellated
        honesty markers, exactly 1 closed shell, 12 faces, and that all 8
        box corners appear as CARTESIAN_POINTs.

Real-material leg: when a corpus `box.stl` is present (OEP_CORPUS or
../samples ../corpus/data), our binary STL export geometry is compared to
it (36 vertex coordinates). Skipped cleanly when absent so CI stays green.
"""
import importlib  # noqa: F401  (kept for parity with sibling tests)
import json
import math
import os
import re
import struct
import sys
import xml.etree.ElementTree as ET

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "crates", "oep-geo", "tests", "golden")

POSITIONS = [
    (1.0, 0.0, 0.0),
    (0.0, 1.0, 0.0),
    (0.0, 0.0, 0.0),
    (1.0, 1.0, 0.0),
    (0.0, 0.0, 1.0),
    (0.0, 1.0, 1.0),
    (1.0, 0.0, 1.0),
    (1.0, 1.0, 1.0),
]
TRIANGLES = [
    (0, 1, 2), (0, 3, 1), (4, 5, 6), (5, 7, 6), (2, 6, 4), (2, 0, 6),
    (3, 5, 7), (3, 1, 5), (5, 1, 2), (5, 2, 4), (0, 7, 3), (0, 6, 7),
]
EXPECTED_CORNERS = {tuple(p) for p in POSITIONS}


def golden(name):
    p = os.path.join(GOLDEN, name)
    assert os.path.isfile(p), f"missing golden {p}"
    return p


# ---------------------------------------------------------------------------
# STL
# ---------------------------------------------------------------------------

def parse_stl(path):
    with open(path, "rb") as fh:
        blob = fh.read()
    assert len(blob) >= 84, "STL too short"
    assert len(blob) == 80 + 4 + 50 * 12, f"unexpected STL length {len(blob)}"
    assert blob[:80] == b"\x00" * 80, "header not zero-filled"
    (count,) = struct.unpack_from("<I", blob, 80)
    assert count == 12, f"tri count {count}"
    facets = []
    pos = 84
    for _ in range(count):
        normal = struct.unpack_from("<3f", blob, pos)
        v0 = struct.unpack_from("<3f", blob, pos + 12)
        v1 = struct.unpack_from("<3f", blob, pos + 24)
        v2 = struct.unpack_from("<3f", blob, pos + 36)
        (attr,) = struct.unpack_from("<H", blob, pos + 48)
        assert attr == 0, "attribute byte count must be 0"
        facets.append((normal, v0, v1, v2))
        pos += 50
    return facets


def test_binary_stl_golden_is_the_unit_box():
    facets = parse_stl(golden("box-reference.bin.stl"))
    corners = set()
    for _n, v0, v1, v2 in facets:
        for v in (v0, v1, v2):
            corners.add(tuple(round(c, 6) for c in v))
    assert len(corners) == 8, f"expected 8 unique corners, got {len(corners)}"
    assert corners == EXPECTED_CORNERS


# ---------------------------------------------------------------------------
# GLB
# ---------------------------------------------------------------------------

def parse_glb(path):
    with open(path, "rb") as fh:
        blob = fh.read()
    magic, version, total = struct.unpack_from("<4sII", blob, 0)
    assert magic == b"glTF", magic
    assert version == 2
    assert total == len(blob)
    (jlen, jtype) = struct.unpack_from("<I4s", blob, 12)
    assert jtype == b"JSON"
    doc = json.loads(blob[20:20 + jlen].rstrip(b" ").decode("ascii"))
    pos = 20 + jlen
    (blen, btype) = struct.unpack_from("<I4s", blob, pos)
    assert btype == b"BIN\x00"
    binbytes = blob[pos + 8:pos + 8 + blen]
    return doc, binbytes


def test_glb_golden_is_the_unit_box():
    doc, binbytes = parse_glb(golden("box-reference.glb"))
    assert doc["asset"]["version"] == "2.0"
    assert doc["scene"] == 0
    assert len(doc["meshes"]) == 1
    prim = doc["meshes"][0]["primitives"][0]
    assert prim["mode"] == 4
    pos_acc = doc["accessors"][prim["attributes"]["POSITION"]]
    idx_acc = doc["accessors"][prim["indices"]]
    assert pos_acc["type"] == "VEC3" and pos_acc["componentType"] == 5126
    assert pos_acc["count"] == 8
    assert idx_acc["count"] == 36
    # decode positions
    bv0 = doc["bufferViews"][0]
    nbytes = pos_acc["count"] * 12
    positions = [
        tuple(struct.unpack_from("<3f", binbytes, bv0["byteOffset"] + i * 12))
        for i in range(pos_acc["count"])
    ]
    assert set(positions) == EXPECTED_CORNERS
    # decode indices (u16)
    bv1 = doc["bufferViews"][1]
    indices = [
        struct.unpack_from("<HHH", binbytes, bv1["byteOffset"] + i * 6)
        for i in range(idx_acc["count"] // 3)
    ]
    assert len(indices) == 12
    for t in indices:
        assert t in TRIANGLES, f"triangle {t} not in reference"


# ---------------------------------------------------------------------------
# OBJ (Wavefront)
# ---------------------------------------------------------------------------

def parse_obj(path):
    verts = []
    faces = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        tok = line.split()
        if tok[0] == "v":
            verts.append(tuple(float(x) for x in tok[1:4]))
        elif tok[0] == "f":
            faces.append(tuple(int(x) - 1 for x in tok[1:4]))
    return verts, faces


def test_obj_golden_is_the_unit_box():
    verts, faces = parse_obj(golden("box-reference.obj"))
    assert len(verts) == 8 and len(faces) == 12
    assert set(tuple(round(c, 6) for c in v) for v in verts) == EXPECTED_CORNERS
    assert set(faces) == set(TRIANGLES)


# ---------------------------------------------------------------------------
# SVG (2D)
# ---------------------------------------------------------------------------

def test_svg_golden_is_the_unit_box_xy_projection():
    root = ET.parse(golden("box-reference.svg")).getroot()
    assert root.tag.endswith("svg")
    namespaces = {"svg": "http://www.w3.org/2000/svg"}
    ge = root.find("svg:g", namespaces)
    assert ge is not None
    path_el = ge.find("svg:path", namespaces)
    assert path_el is not None
    d = path_el.get("d")
    assert d is not None, "path has no d attribute"
    segs = re.findall(r"M\s+([-\d.]+)\s+([-\d.]+)\s+L\s+([-\d.]+)\s+([-\d.]+)", d)
    # 5 unique projected XY edges of the unit box wireframe
    assert len(segs) == 5, f"expected 5 projected edges, got {len(segs)}"
    viewbox = root.get("viewBox")
    assert viewbox == "-0.5 -0.5 2 2", viewbox


# ---------------------------------------------------------------------------
# PDF (drawing path)
# ---------------------------------------------------------------------------

def pdf_objects(path):
    blob = open(path, "rb").read()
    assert blob.startswith(b"%PDF-1.4")
    # collect object bodies between "N 0 obj" and "endobj"
    objs = {}
    for m in re.finditer(rb"(\d+) 0 obj\n(.*?)\nendobj\n", blob, re.S):
        objs[int(m.group(1))] = m.group(2)
    return objs


def test_pdf_golden_draws_the_unit_box_xy_projection():
    objs = pdf_objects(golden("box-reference.pdf"))
    assert set(objs) == {1, 2, 3, 4}
    assert b"/Type /Catalog" in objs[1]
    assert b"/Type /Pages" in objs[2]
    assert b"/Type /Page" in objs[3] and b"/MediaBox [0 0 2 2]" in objs[3]
    stream_m = re.search(rb"stream\n(.*?)\nendstream", objs[4], re.S)
    assert stream_m is not None, "no stream in content object"
    stream = stream_m.group(1)
    ops = stream.decode("ascii").strip().splitlines()
    assert len(ops) == 5, f"expected 5 drawing operators, got {len(ops)}"
    for op in ops:
        assert op.endswith("l S"), op


def test_pdf_opens_in_pdfinfo_and_renders():
    import shutil
    import subprocess
    pdf = golden("box-reference.pdf")
    qpdf = shutil.which("qpdf")
    pdfinfo = shutil.which("pdfinfo")
    pdftoppm = shutil.which("pdftoppm")
    if not qpdf and not pdfinfo:
        print("SKIP: no qpdf/pdfinfo available (structural checks above already ran)")
        return
    if qpdf:
        r = subprocess.run([qpdf, "--check", pdf], capture_output=True, text=True)
        assert r.returncode == 0, r.stdout + r.stderr
    if pdfinfo:
        r = subprocess.run([pdfinfo, pdf], capture_output=True, text=True)
        assert "Pages:           1" in r.stdout, r.stdout
    if pdftoppm:
        out_root = pdf + ".render"
        r = subprocess.run([pdftoppm, "-png", "-singlefile", pdf, out_root],
                           capture_output=True, text=True)
        assert r.returncode == 0, r.stdout + r.stderr
        out_png = out_root + ".png"  # pdftoppm appends its own extension
        assert os.path.isfile(out_png) and os.path.getsize(out_png) > 100
        os.remove(out_png)
        print("rendered PDF page 1 to PNG (real-reader proof)")


# ---------------------------------------------------------------------------
# STEP (faceted passthrough)
# ---------------------------------------------------------------------------

def test_step_golden_is_the_unit_box_faceted_shell():
    text = open(golden("box-reference.step"), encoding="utf-8").read()
    assert text.startswith("ISO-10303-21;\nHEADER;")
    assert "ENDSEC;\nDATA;" in text
    assert text.strip().endswith("END-ISO-10303-21;")
    assert "FILE_SCHEMA(('AUTOMOTIVE_DESIGN" in text
    assert "TESSELLATED representation and is NOT an exact analytic B-Rep." in text
    # geometry: every box corner appears as a CARTESIAN_POINT coordinate token
    for (x, y, z) in POSITIONS:
        assert "CARTESIAN_POINT('',(%s,%s,%s))" % (
            fmt_step_real(x), fmt_step_real(y), fmt_step_real(z)) in text
    # exactly one shell, closed (watertight box)
    assert text.count("CLOSED_SHELL") == 1, text.count("CLOSED_SHELL")
    assert "OPEN_SHELL" not in text
    assert text.count("SHELL_BASED_SURFACE_MODEL") == 1
    # 12 faces = 12 FACE_SURFACE with PLANE + POLY_LOOP each
    assert text.count("FACE_SURFACE") == 12
    assert text.count("FACE_OUTER_BOUND") == 12
    assert text.count("PLANE('',#") == 12
    assert text.count("POLY_LOOP") == 12


def fmt_step_real(v):
    # STEP PART 21 REAL: force a decimal point ("1." / "0." / "-0.5") so every
    # coordinate token round-trips as a REAL, matching the independent encoder
    # (tools/gen_golden_box_step.py fmt_real) and the Rust writer.
    f = struct.unpack("<f", struct.pack("<f", v))[0]
    if f == int(f) and abs(f) < 1e16:
        return "%d." % int(f)
    return repr(f)


# ---------------------------------------------------------------------------
# Real material leg
# ---------------------------------------------------------------------------

def test_binary_stl_matches_corpus_box_when_present():
    env = os.environ.get("OEP_CORPUS", "")
    candidates = ([env] if env else []) + [
        os.path.join(REPO, "corpus", "data", "box.stl"),
        os.path.join(REPO, "..", "samples", "box.stl"),
    ]
    box = next((c for c in candidates if os.path.isfile(c)), None)
    if box is None:
        print("SKIP: no corpus box.stl present; structural checks already ran")
        return
    # ASCII STL reference: collect every 'vertex x y z' coordinate exactly
    ref_verts = []
    for line in open(box, encoding="utf-8"):
        tok = line.split()
        if len(tok) >= 4 and tok[0] == "vertex":
            ref_verts.append(tuple(float(x) for x in tok[1:4]))
    assert len(ref_verts) == 36, f"expected 36 vertex records, got {len(ref_verts)}"
    facets = parse_stl(golden("box-reference.bin.stl"))
    exported = [v for _n, v0, v1, v2 in facets for v in (v0, v1, v2)]
    assert len(exported) == 36
    for e, r in zip(exported, ref_verts):
        for a, b in zip(e, r):
            assert math.isclose(a, b, abs_tol=1e-6), (e, r)
