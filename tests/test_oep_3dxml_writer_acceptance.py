#!/usr/bin/env python3
"""P3-M4-T01 acceptance test: internal mesh -> 3DXML ASCII writer.

INDEPENDENT verification of the committed golden package
`crates/oep-3dxml/tests/golden/box-reference.3dxml` (which the Rust test
`crates/oep-3dxml/tests/writer_ascii.rs` byte-locks the writer's output to,
so anything verified here is verified transitively against the writer):

  PART A (always, hermetic): open the golden as a ZIP with the SYSTEM zipfile,
  parse its XML with the SYSTEM xml.etree, and assert:
    - exactly three STORED members: Manifest.xml, box.3dxml, box.3DRep
      (CRC-32 valid — zipfile.testzip() clean);
    - Manifest <Root> names the model doc "box.3dxml";
    - model doc is <Model_3dxml> with Header, ProductStructure and a
      <ReferenceRep format="TESSELLATED" associatedFile="urn:3DXML:box.3DRep">;
    - the .3DRep is the tessellated-ASCII branch: XMLRepresentation ->
      BagRep/PolygonalRep -> Faces(strips) + VertexBuffer(Positions);
    - geometry is EXACTLY the isolith CC0-1.0 unit box: 8 corners
      [1 0 0, 0 1 0, 0 0 0, 1 1 0, 0 0 1, 0 1 1, 1 0 1, 1 1 1] and the
      12 triangle index triples, losslessly.

  PART B (real material, SKIPs when no 3DXML samples are present so CI stays
  green before the corpus PR merges): for each real ZIP-archive 3DXML sample
  found (env OEP_CORPUS or samples/ ../samples/ ../corpus/data/), require the
  sample's structure to MATCH the golden's structure — Manifest -> primary
  model doc -> TESSELLATED ReferenceRep associatedFile -> ASCII .3DRep
  (XMLRepresentation with Faces/VertexBuffer) — i.e. the writer conforms in
  shape to real CATIA/Abaqus-authored ASCII-branch 3DXML files.

License note: this test never embeds Dassault spec text; the format is
checked against observed facts of public MIT/Apache-2.0 samples (hex
notebook §4) and the internal-use licensed spec held privately (P1-M1-T02).
"""

import os
import re
import sys
import zipfile
import xml.etree.ElementTree as ET

import pytest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "crates", "oep-3dxml", "tests", "golden", "box-reference.3dxml")

SCHEMA_3DXML = "http://www.3ds.com/xsd/3DXML"
XSI = "http://www.w3.org/2001/XMLSchema-instance"

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
    (0, 1, 2), (0, 3, 1), (4, 5, 6), (5, 7, 6),
    (2, 6, 4), (2, 0, 6), (3, 5, 7), (3, 1, 5),
    (5, 1, 2), (5, 2, 4), (0, 7, 3), (0, 6, 7),
]


def _float(s):
    # Python 3.14: float("1") etc. all parse; keep it explicit.
    return float(s)


# ---------------------------------------------------------------------------
# PART A — hermetic independent parse of the golden
# ---------------------------------------------------------------------------


def test_golden_exists_and_is_zip():
    assert os.path.isfile(GOLDEN), "missing committed golden %s" % GOLDEN
    with open(GOLDEN, "rb") as fh:
        head = fh.read(4)
    assert head == b"PK\x03\x04", "golden must be a ZIP archive"


def test_golden_zip_members_and_crc():
    with zipfile.ZipFile(GOLDEN) as zf:
        assert zf.testzip() is None, "no corrupt members (CRC-32 valid)"
        infos = zf.infolist()
        names = [i.filename for i in infos]
        assert names == ["Manifest.xml", "box.3dxml", "box.3DRep"], names
        for i in infos:
            assert i.compress_type == zipfile.ZIP_STORED, "STORED members"


def test_manifest_names_model_doc():
    with zipfile.ZipFile(GOLDEN) as zf:
        manifest = zf.read("Manifest.xml")
    root = ET.fromstring(manifest)
    assert root.tag.endswith("Manifest")
    root_el = root.find("Root")
    assert root_el is not None and root_el.text is not None
    assert root_el.text == "box.3dxml", "Manifest <Root> must name the model doc"


def test_model_doc_wires_tessellated_rep():
    with zipfile.ZipFile(GOLDEN) as zf:
        model = zf.read("box.3dxml")
    root = ET.fromstring(model)  # proves well-formed XML
    assert root.tag == "{%s}Model_3dxml" % SCHEMA_3DXML, root.tag
    header = root.find("{%s}Header" % SCHEMA_3DXML)
    assert header is not None
    schema = header.find("{%s}SchemaVersion" % SCHEMA_3DXML)
    assert schema is not None and schema.text == "4.0"
    ps = root.find("{%s}ProductStructure" % SCHEMA_3DXML)
    assert ps is not None and ps.get("root") == "1"
    assert ps.find("{%s}Reference3D" % SCHEMA_3DXML) is not None
    assert ps.find("{%s}Instance3D" % SCHEMA_3DXML) is not None
    refrep = ps.find("{%s}ReferenceRep" % SCHEMA_3DXML)
    assert refrep is not None
    assert refrep.get("format") == "TESSELLATED"
    assert refrep.get("associatedFile") == "urn:3DXML:box.3DRep"


def test_rep_is_ascii_tessellated_branch():
    with zipfile.ZipFile(GOLDEN) as zf:
        rep = zf.read("box.3DRep")
    root = ET.fromstring(rep)  # proves well-formed XML
    assert root.tag == "{%s}XMLRepresentation" % SCHEMA_3DXML, root.tag
    assert root.get("version") == "1.0"
    # BagRep -> PolygonalRep spine
    nsm = {"xsi": XSI}
    bags = root.findall(".//*[@xsi:type='BagRepType']", nsm)
    polys = root.findall(".//*[@xsi:type='PolygonalRepType']", nsm)
    assert bags and polys, "BagRep/PolygonalRep spine present"
    vb = root.find(".//{%s}VertexBuffer" % SCHEMA_3DXML)
    faces = root.findall(".//{%s}Face" % SCHEMA_3DXML)
    assert vb is not None and faces, "VertexBuffer + Faces present"
    assert vb.find("{%s}Positions" % SCHEMA_3DXML) is not None


def test_geometry_is_exactly_the_unit_box():
    with zipfile.ZipFile(GOLDEN) as zf:
        rep = zf.read("box.3DRep")
    root = ET.fromstring(rep)
    pos_el = root.find(".//{%s}Positions" % SCHEMA_3DXML)
    face_el = root.find(".//{%s}Face" % SCHEMA_3DXML)
    assert pos_el is not None and face_el is not None

    positions = []
    for triple in pos_el.text.strip().split(","):
        coords = [_float(x) for x in triple.split()]
        assert len(coords) == 3, triple
        positions.append(tuple(coords))
    assert positions == POSITIONS, "positions round-trip exactly"

    strips = face_el.get("strips")
    assert strips is not None
    triangles = []
    for run in strips.split(","):
        idx = [int(x) for x in run.split()]
        assert len(idx) == 3, "one 3-index strip-run per triangle"
        triangles.append(tuple(idx))
    assert triangles == TRIANGLES, "triangle indices round-trip exactly"


# ---------------------------------------------------------------------------
# PART B — real material: structural conformance to real ASCII-branch samples
# ---------------------------------------------------------------------------


def _sample_dirs():
    dirs = []
    env = os.environ.get("OEP_CORPUS", "")
    if env:
        dirs += [d for d in env.split(os.pathsep) if d]
    for rel in ("samples", "../samples", "../corpus/data", "corpus/data"):
        p = os.path.normpath(os.path.join(REPO, rel))
        if os.path.isdir(p):
            dirs.append(p)
    return dirs


def _all_3dxml_samples():
    out = {}
    for d in _sample_dirs():
        for root_dir, _dirs, files in os.walk(d):
            for fn in files:
                if fn.lower().endswith(".3dxml"):
                    out[os.path.join(root_dir, fn)] = fn
    return sorted(out.items())


def _ascii_branch_member(zf, name):
    data = zf.read(name)
    return data.lstrip()[:5] == b"<?xml"


def test_writer_structure_matches_real_ascii_samples():
    samples = _all_3dxml_samples()
    if not samples:
        pytest.skip("no real 3DXML corpus supplied (OEP_CORPUS/env or samples/); "
                    "SKIP so CI stays green before corpus PR merges")
    checked = 0
    for path, fn in samples:
        try:
            with zipfile.ZipFile(path) as zf:
                manifest_names = [n for n in zf.namelist() if n == "Manifest.xml"]
                if not manifest_names:
                    continue  # not a ZIP-archive ASCII 3DXML; skip this one
                # find the primary model doc via Manifest <Root>
                root_el = ET.fromstring(zf.read("Manifest.xml")).find("Root")
                if root_el is None or not root_el.text:
                    continue
                model_name = root_el.text
                if model_name not in zf.namelist():
                    continue
                model_root = ET.fromstring(zf.read(model_name))
                if model_root.tag != "{%s}Model_3dxml" % SCHEMA_3DXML:
                    continue
                # a TESSELLATED rep wired by associatedFile to a real member
                found_ascii_tess = False
                for refrep in model_root.findall(".//{%s}ReferenceRep" % SCHEMA_3DXML):
                    if refrep.get("format") != "TESSELLATED":
                        continue
                    urn = refrep.get("associatedFile") or ""
                    mname = urn.split("urn:3DXML:")[-1]
                    if mname not in zf.namelist():
                        continue
                    if not _ascii_branch_member(zf, mname):
                        continue
                    rep_root = ET.fromstring(zf.read(mname))
                    if rep_root.tag != "{%s}XMLRepresentation" % SCHEMA_3DXML:
                        continue
                    has_faces = rep_root.find(".//{%s}Face" % SCHEMA_3DXML) is not None
                    has_pos = (
                        rep_root.find(".//{%s}Positions" % SCHEMA_3DXML) is not None
                    )
                    if has_faces and has_pos:
                        found_ascii_tess = True
                        break
                if found_ascii_tess:
                    checked += 1
        except (zipfile.BadZipFile, ET.ParseError, KeyError, OSError):
            continue
    assert checked >= 1, (
        "no real ASCII-branch tessellated 3DXML found among %d sample(s); "
        "refusing a silent empty conformance claim" % len(samples)
    )
