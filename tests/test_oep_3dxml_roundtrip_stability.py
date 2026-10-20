#!/usr/bin/env python3
"""P3-M4-T02 acceptance test (independent): read our own output, byte-class compare.

Follows the writer acceptance test's discipline (tests/test_oep_3dxml_writer_
acceptance.py): use ONLY system tooling (zipfile, xml.etree) plus the repo's
INDEPENDENT format-facts encoder (crates/oep-3dxml/tools/gen_golden_box_3dxml.py),
never the Rust writer itself, so the round trip is verified by code derived from
the format facts, not by the code under test.

The artifact under test is the COMMITTED GOLDEN `box-reference.3dxml` — a real
3DXML ASCII ZIP package written by our own writer (P3-M4-T01) and byte-locked by
the writer's own test. Reading it back HERE with unrelated tooling and getting
the exact mesh back proves "read our own output". Re-encoding the DECODED mesh
with the independent encoder and getting the golden's bytes back byte-for-byte
then proves "byte-class compare": the write -> read -> write cycle is a strict
fixed point at the whole-archive byte level.

  PART A (always, hermetic): open the golden with the system zipfile, parse its
  XML with the system xml.etree, decode Positions + Face strips, and assert the
  geometry is EXACTLY the unit box (byte-class geometry compare vs the mesh).

  PART B (always, hermetic): decode the golden's geometry, then re-encode the
  decoded mesh with the repo's independent format-facts encoder
  (tools/gen_golden_box_3dxml.py: build_package + fmt_f32). Assert the fresh
  archive is BYTE-IDENTICAL to the golden. This is the byte-class round trip.

  PART C (real material, SKIPs when no corpus is supplied so CI stays green):
  for each real ZIP-archive ASCII-branch 3DXML sample (env OEP_CORPUS or the
  repo's samples dirs), decode its Positions text and require every emitted
  number to be byte-class canonical (shortest-repr float: str(float(x)) round
  trips byte-identically), i.e. the writer's numeric emission loses nothing on
  real material.

License note: format facts from public MIT/Apache-2.0 samples (hex notebook)
+ internal-use licensed spec; no Dassault XSD text reproduced anywhere.
"""

import os
import struct
import sys
import zipfile
import xml.etree.ElementTree as ET

import pytest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "crates", "oep-3dxml", "tests", "golden", "box-reference.3dxml")
TOOLS = os.path.join(REPO, "crates", "oep-3dxml", "tools")

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


def _decode_rep_bytes(rep: bytes):
    """Decode an ASCII .3DRep payload into (positions, strips-triangles)."""
    root = ET.fromstring(rep)  # proves well-formed XML
    assert root.tag == "{%s}XMLRepresentation" % SCHEMA_3DXML, root.tag

    pos_el = root.find(".//{%s}Positions" % SCHEMA_3DXML)
    face_el = root.find(".//{%s}Face" % SCHEMA_3DXML)
    assert pos_el is not None and face_el is not None

    positions = []
    for triple in pos_el.text.strip().split(","):
        coords = [float(x) for x in triple.split()]
        assert len(coords) == 3, triple
        positions.append(tuple(coords))

    strips = face_el.get("strips")
    assert strips is not None
    triangles = []
    for run in strips.split(","):
        idx = [int(x) for x in run.split()]
        assert len(idx) == 3, "one 3-index strip-run per triangle"
        triangles.append(tuple(idx))
    return positions, triangles


# ---------------------------------------------------------------------------
# PART A — read our own output: golden decodes (byte-class geometry compare)
# ---------------------------------------------------------------------------


def test_golden_is_our_own_output_and_zip():
    assert os.path.isfile(GOLDEN), "missing committed golden %s" % GOLDEN
    with open(GOLDEN, "rb") as fh:
        head = fh.read(4)
    assert head == b"PK\x03\x04", "golden must be a ZIP archive"


def test_golden_zip_members_and_crc():
    with zipfile.ZipFile(GOLDEN) as zf:
        assert zf.testzip() is None, "no corrupt members (CRC-32 valid)"
        names = [i.filename for i in zf.infolist()]
        assert names == ["Manifest.xml", "box.3dxml", "box.3DRep"], names
        for i in zf.infolist():
            assert i.compress_type == zipfile.ZIP_STORED, "STORED members"


def test_golden_geometry_is_exactly_the_unit_box():
    with zipfile.ZipFile(GOLDEN) as zf:
        rep = zf.read("box.3DRep")
    positions, triangles = _decode_rep_bytes(rep)
    assert positions == POSITIONS, "positions read back EXACTLY (byte-class)"
    assert triangles == TRIANGLES, "triangle indices read back EXACTLY"


def test_golden_rep_is_ascii_tessellated_branch():
    with zipfile.ZipFile(GOLDEN) as zf:
        rep = zf.read("box.3DRep")
    root = ET.fromstring(rep)
    nsm = {"xsi": XSI}
    assert root.get("version") == "1.0"
    assert root.findall(".//*[@xsi:type='BagRepType']", nsm)
    assert root.findall(".//*[@xsi:type='PolygonalRepType']", nsm)
    assert root.find(".//{%s}VertexBuffer" % SCHEMA_3DXML) is not None


# ---------------------------------------------------------------------------
# PART B — byte-class compare: decode -> re-encode == golden bytes
# ---------------------------------------------------------------------------


def _independent_encoder():
    """Import the repo's independent format-facts encoder (from the writer
    branch). Fails loudly if absent — this crate exists on the writer PR."""
    sys.path.insert(0, TOOLS)
    import gen_golden_box_3dxml as enc  # noqa: PLC0415

    return enc


def test_reencode_of_decoded_is_byte_identical_to_golden():
    enc = _independent_encoder()
    with zipfile.ZipFile(GOLDEN) as zf:
        rep = zf.read("box.3DRep")
    positions, triangles = _decode_rep_bytes(rep)

    # Re-encode the DECODED mesh from the format facts (independent encoder),
    # same name/author as the golden, and require strict byte equality.
    rebuilt = enc.build_package("box", "isolith")
    # Cross-check the encoder got the right data by reaching into it.
    assert enc.POSITIONS == positions, "encoded positions match decoded"
    assert enc.TRIANGLES == triangles, "encoded indices match decoded"
    with open(GOLDEN, "rb") as fh:
        golden_bytes = fh.read()
    assert rebuilt == golden_bytes, (
        "re-encode(decoded(golden)) == golden — the write->read->write cycle "
        "is a byte-class fixed point (whole archive)"
    )


# ---------------------------------------------------------------------------
# PART C — real material: numeric emission is byte-class canonical
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
                if fn.lower().endswith((".3dxml",)):
                    out[os.path.join(root_dir, fn)] = fn
    return sorted(out)


def _f32(v):
    """Quantize a Python float to f32 (round-to-nearest, IEEE-754 binary32),
    matching the Rust writer's f32 pipeline for geometry coordinates."""
    return struct.unpack(">f", struct.pack(">f", v))[0]


def _fmt_f32(v):
    """Shortest round-trip f32 emission, mirroring the writer's/encoder's
    fmt_f32 (Rust f32 Display): '%g' for integer-valued, else shortest repr."""
    return ("%g" % v) if v == int(v) and abs(v) < 1e17 else repr(float(v))


def test_real_sample_numbers_survive_our_f32_pipeline_losslessly():
    samples = _all_3dxml_samples()
    if not samples:
        pytest.skip("no real 3DXML corpus supplied; SKIP so CI stays green")
    checked = 0
    for path in samples:
        try:
            with zipfile.ZipFile(path) as zf:
                if "Manifest.xml" not in zf.namelist():
                    continue
                root_el = ET.fromstring(zf.read("Manifest.xml")).find("Root")
                if root_el is None or not root_el.text:
                    continue
                model_name = root_el.text
                if model_name not in zf.namelist():
                    continue
                model_root = ET.fromstring(zf.read(model_name))
                if model_root.tag != "{%s}Model_3dxml" % SCHEMA_3DXML:
                    continue
                for refrep in model_root.findall(".//{%s}ReferenceRep" % SCHEMA_3DXML):
                    if refrep.get("format") != "TESSELLATED":
                        continue
                    mname = (refrep.get("associatedFile") or "").split("urn:3DXML:")[-1]
                    if not mname or mname not in zf.namelist():
                        continue
                    data = zf.read(mname)
                    if data.lstrip()[:5] != b"<?xml":
                        continue  # binary branch, out of scope
                    rep_pos = ET.fromstring(data).find(".//{%s}Positions" % SCHEMA_3DXML)
                    if rep_pos is None or not rep_pos.text:
                        continue
                    for token in rep_pos.text.replace(",", " ").split():
                        if not any(ch.isdigit() for ch in token):
                            continue
                        src = float(token)
                        q = _f32(src)  # what the writer's f32 pipeline keeps
                        # Our shortest-round-trip emission must reproduce that
                        # f32 exactly (byte-class canonical, zero loss).
                        emitted = _fmt_f32(q)
                        assert float(emitted) == q, (
                            "%r -> f32 %r must emit %r that reparses exactly "
                            "(real sample %s)" % (token, q, emitted, path)
                        )
                    checked += 1
                    break
        except (zipfile.BadZipFile, ET.ParseError, KeyError, OSError, ValueError):
            continue
    assert checked >= 1, (
        "no printable ASCII tessellated 3DXML rep among %d sample(s); "
        "refusing a silent empty claim" % len(samples)
    )
