#!/usr/bin/env python3
"""P3-M4-T01 golden encoder: internal mesh -> 3DXML ASCII ZIP package.

INDEPENDENT encoder for the committed golden file
`crates/oep-3dxml/tests/golden/box-reference.3dxml`. Written directly from the
observed 3DXML format facts (docs/re-notes/3dxml-hex-notebook.md section 4)
and the public PKWARE ZIP APPNOTE — NOT from the Rust writer, so the Rust test
that byte-locks its output against this golden proves conformance against an
encoding derived independently from the format facts.

The unit box (8 corners / 12 triangles) is the isolith CC0-1.0 reference
tessellation (`corpus/data/box.stl`, P0-M4-T02 fixture).

ZIP byte contract (matched bv the Rust writer for determinism):
  - 3 STORED members in order: Manifest.xml, box.3dxml, box.3DRep
  - fixed DOS timestamp 1980-01-01 00:00:00 (date word 0x0021, time 0x0000)
  - version_needed/version_made_by 20, flags 0, method 0, extra/comment 0
  - CRC-32 (PKWARE, reflected, poly 0xEDB88320) via zlib.crc32
Usage: python3 crates/oep-3dxml/tools/gen_golden_box_3dxml.py
       [--out <path>]   (default: crates/oep-3dxml/tests/golden/box-reference.3dxml)
"""
import argparse
import struct
import zlib
from pathlib import Path

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


def fmt_f32(v: float) -> str:
    # Shortest round-trip representation — must match Rust's f32 Display
    # exactly for every f32 bit pattern. Box corners are 0/1 (exact).
    return ("%g" % v) if v == int(v) and abs(v) < 1e17 else repr(float(v))


def positions_text() -> str:
    return ",".join(" ".join(fmt_f32(c) for c in p) for p in POSITIONS)


def strips_text() -> str:
    return ",".join("%d %d %d" % t for t in TRIANGLES)


def manifest(name: str) -> str:
    return (
        '<?xml version="1.0" encoding="utf-8" ?>\n'
        '<Manifest xmlns:xsi="%s" xsi:noNamespaceSchemaLocation="Manifest.xsd">'
        "<Root><![CDATA[%s.3dxml]]></Root></Manifest>\n" % (XSI, name)
    )


def model_doc(name: str, author: str) -> str:
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<Model_3dxml xmlns="%s" xmlns:xsi="%s">\n'
        "  <Header>\n"
        "    <SchemaVersion>4.0</SchemaVersion>\n"
        "    <Title>%s</Title>\n"
        "    <Author>%s</Author>\n"
        "    <Generator>Isolith oep-3dxml</Generator>\n"
        "  </Header>\n"
        '  <ProductStructure root="1">\n'
        '    <Reference3D id="1" name="%s"/>\n'
        '    <Instance3D id="11" name="Instance1">\n'
        "      <IsAggregatedBy/>\n"
        '      <IsInstanceOf xsi:type="InstanceOfReferenceType" aggregatedById="1"/>\n'
        "      <RelativeMatrix>1 0 0 0 0 1 0 0 0 0 1 0</RelativeMatrix>\n"
        "    </Instance3D>\n"
        '    <ReferenceRep id="21" name="%s" format="TESSELLATED" version="1.0" '
        'associatedFile="urn:3DXML:%s.3DRep"/>\n'
        "  </ProductStructure>\n"
        "</Model_3dxml>\n" % (SCHEMA_3DXML, XSI, name, author, name, name, name)
    )


def rep_markup(name: str) -> str:
    return (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<XMLRepresentation version="1.0" xmlns="%s" xmlns:xsi="%s">\n'
        '  <Root xsi:type="BagRepType">\n'
        '    <Rep xsi:type="BagRepType">\n'
        '      <Rep xsi:type="PolygonalRepType">\n'
        '        <Faces><Face strips="%s"/></Faces>\n'
        "        <VertexBuffer><Positions>%s</Positions></VertexBuffer>\n"
        "      </Rep>\n"
        "    </Rep>\n"
        "  </Root>\n"
        "</XMLRepresentation>\n" % (SCHEMA_3DXML, XSI, strips_text(), positions_text())
    )


def _local_header(name: bytes, data: bytes, crc: int) -> bytes:
    return struct.pack(
        "<4s5H3I2H",
        b"PK\x03\x04",
        20,          # version needed
        0,           # flags
        0,           # method: STORED
        0,           # mod time (1980-01-01 00:00:00)
        0x0021,      # mod date (1980-01-01)
        crc,
        len(data),   # compressed size == uncompressed (STORED)
        len(data),
        len(name),
        0,           # extra len
    ) + name + data


def _central(name: bytes, data: bytes, crc: int, offset: int) -> bytes:
    return struct.pack(
        "<4s6H3I5H2I",
        b"PK\x01\x02",
        20,  # version made by
        20,  # version needed
        0,   # flags
        0,   # method STORED
        0,   # mod time
        0x0021,  # mod date
        crc,
        len(data),
        len(data),
        len(name),
        0,      # extra len
        0,      # comment len
        0,      # disk start
        0,      # internal attrs
        0,      # external attrs
        offset,
    ) + name


def build_package(name: str, author: str) -> bytes:
    members = [
        (("Manifest.xml").encode("utf-8"), manifest(name).encode("utf-8")),
        (("%s.3dxml" % name).encode("utf-8"), model_doc(name, author).encode("utf-8")),
        (("%s.3DRep" % name).encode("utf-8"), rep_markup(name).encode("utf-8")),
    ]
    local = b""
    central = b""
    offset = 0
    for mname, data in members:
        crc = zlib.crc32(data) & 0xFFFFFFFF
        lh = _local_header(mname, data, crc)
        local += lh
        central += _central(mname, data, crc, offset)
        offset += len(lh)
    eocd = struct.pack(
        "<4s4H2LH",
        b"PK\x05\x06",
        0, 0,               # disk number / cd start disk
        len(members),       # entries on this disk
        len(members),       # total entries
        len(central),       # central directory size
        len(local),         # central directory offset
        0,                  # comment len
    )
    return local + central + eocd


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    out = (
        Path(args.out)
        if args.out
        else Path(__file__).resolve().parents[1]
        / "tests"
        / "golden"
        / "box-reference.3dxml"
    )
    pkg = build_package("box", "isolith")
    out.write_bytes(pkg)
    print("wrote %s (%d bytes)" % (out, len(pkg)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
