#!/usr/bin/env python3
"""P4-M4-T03: synthesize license-clean JT fixtures (dev-only corpus, decision-7).

The JT reader is built clean-room from the published format description
(Siemens JT v10 File Format Reference Rev-B and JTIAP PSI 14-1 V3). This
writer is the OTHER side of the documented byte layout: it emits bytes
following the exact layouts in that reference, so the reader (written
independently from the same prose spec) can be checked against a real
structured input, not against estimates. The fixtures are our own authored
bytes (no third-party JT file is redistributed), catalogued under the corpus
dev-only posture (never shipped).

Layouts implemented here (all cited to Siemens JT v10 Rev-B):
- File header 5.1.1: 80-byte version string, UChar Byte Order, I32 Empty,
  U64 TOC Offset, GUID LSG Segment ID, GUID Empty Field.
- TOC segment 5.1.2: I32 entry count, then per entry GUID Segment ID,
  U64 Segment Offset, U32 Segment Length, U32 Segment Attributes.
- Segment header 5.1.3.1: GUID Segment ID, I32 Segment Type, I32 Segment
  Length. Table 6: type 1 = LSG, 7..16 = Shape LOD0..9.
- Element header 5.1.3.2: I32 Element Length + GUID Object Type ID + U8
  Object Base Type + I32 Object ID. LSG uses the "Compressed" variant
  (Figure 19): U32 Compression Flag, I32 Compressed Data Length, U8
  Compression Algorithm, then the Logical Element Header.
- LSG 6.1.1: Base Node Data (U8 version, U32 flags, I32 attr count + ids);
  Partition = Group Node Data + local version + partition flags + MbString
  file name + bbox + area + count ranges; Part = Meta Data Node Data (group).
  6.2.2 String Property Atom (MbString value); 6.2.7 Late Loaded Property
  Atom (GUID segment id + I32 segment type + I32 payload object id + I32
  reserved).
- Shape LOD 7.2 Primitive Set Shape Element: base shape version (U8), base
  primset version (U8), U64 vertex bindings, U8 version, I32 bits-per-vertex,
  I8 texcoord gen type, then Lossless Compressed Primitive Set Data (I32
  uncompressed size, I32 compressed size < 0 => raw, U8 raw primitive
  records). Primitive record (Table 51): I32 reserved, CoordF32 params1,
  DirF32 params2, Quaternion params3, RGB colour, I32 type (0 box).
  Box interpretation (Table 52): params1 = min corner, params2 = box
  lengths, params3 = orientation quaternion.

End of elements marker GUID (Annex A): ff ff ff ff ... ff.
"""
import os
import struct

HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.join(os.path.dirname(os.path.dirname(HERE)), "corpus", "jt")

HEADER_SIZE = 80 + 1 + 4 + 8 + 16 + 16

# ---------------------------------------------------------------------------
# GUID helpers. JT stores a GUID as U32 + 2x U16 + 8 x U8 (Ref 4 Table 4).
# Files we write are always LSB-first (Byte Order = 0); the little-endian
# word packing below matches that. Guid-open readers read U32/U16 in file
# byte order.
# ---------------------------------------------------------------------------


def guid_le(u32, u16a, u16b, tail):
    return struct.pack("<IHH", u32, u16a, u16b) + bytes(tail)


GID_PARTITION = guid_le(0x10DD103E, 0x2AC8, 0x11D1, [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97])
GID_PART = guid_le(0xCE357244, 0x38FB, 0x11D1, [0xA5, 0x06, 0x00, 0x60, 0x97, 0xBD, 0xC6, 0xE1])
GID_GROUP = guid_le(0x10DD101B, 0x2AC8, 0x11D1, [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97])
GID_INSTANCE = guid_le(0x10DD102A, 0x2AC8, 0x11D1, [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97])
GID_LOD = guid_le(0x10DD102C, 0x2AC8, 0x11D1, [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97])
GID_SWITCH = guid_le(0x10DD10F3, 0x2AC8, 0x11D1, [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97])
GID_STRING_PROP = guid_le(0x10DD106E, 0x2AC8, 0x11D1, [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97])
GID_INT_PROP = guid_le(0x10DD102B, 0x2AC8, 0x11D1, [0x9B, 0x6B, 0x00, 0x80, 0xC7, 0xBB, 0x59, 0x97])
GID_LATE_LOAD = guid_le(0xE0B05BE5, 0xFBBD, 0x11D1, [0xA3, 0xA7, 0x00, 0xAA, 0x00, 0xD1, 0x09, 0x54])
GID_PRIM_SET = guid_le(0xE40373C2, 0x1AD9, 0x11D3, [0x9D, 0xAF, 0x00, 0xA0, 0xC9, 0xC7, 0xDD, 0xC2])
GID_END = b"\xFF" * 16

BASE_GRAPH_NODE = 0
BASE_PROPERTY = 5

SEGMENT_LSG = 1
SEGMENT_SHAPE_LOD0 = 7


def mbstring(s):
    """MbString: I32 char count + UTF-16-LE character data (Ref 4 Table 4)."""
    return struct.pack("<i", len(s)) + s.encode("utf-16-le")


def element_body(gid, base_type, oid, payload):
    """Logical Element Header + Object Data (Ref 5.1.3.2)."""
    hdr = gid + struct.pack("<B", base_type) + struct.pack("<i", oid)
    total = len(hdr) + len(payload)
    return struct.pack("<i", total) + hdr + payload


def element_compressed_body(gid, base_type, oid, payload):
    """Logical Element Header Compressed (Ref Figure 19), for all elements
    EXCEPT the first in a segment: I32 Compressed Data Length, U8 Compression
    Algorithm (1 = none), then the Logical Element Header. The U32 Compression
    Flag is written only by the first element of a segment (see
    with_compression_flag) - it is only present on the first element."""
    inner = element_body(gid, base_type, oid, payload)
    # Compressed Data Length includes the algorithm byte.
    return struct.pack("<IB", len(inner) + 1, 1) + inner


def first_element_compressed_body(gid, base_type, oid, payload):
    """First element of an LSG segment: prepend the U32 Compression Flag
    (0 = no compression) before the standard compressed element header."""
    return struct.pack("<I", 0) + element_compressed_body(gid, base_type, oid, payload)


def base_node_data(attrs=()):
    """Ref 6.1.1.1.1 Base Node Data: U8 version, U32 flags, I32 attr count,
    I32 attr object ids."""
    return struct.pack("<BII", 1, 0, len(attrs)) + struct.pack("<%di" % len(attrs), *attrs)


def group_node_data(children=()):
    """Ref 6.1.1.3.1 Group Node Data: Base Node Data + U8 version + I32 child
    count + I32 child ids."""
    b = base_node_data()
    b += struct.pack("<B", 1)
    b += struct.pack("<i", len(children))
    for c in children:
        b += struct.pack("<i", c)
    return b


def partition_payload(children, name, bmin, bmax):
    """Ref 6.1.1.2 Partition Node: Group Node Data + U8 local version + I32
    partition flags (0) + MbString file name + BBoxF32 + F32 area + 3 count
    ranges (I32 min + I32 max each)."""
    b = group_node_data(children)
    b += struct.pack("<B", 1)
    b += struct.pack("<i", 0)
    b += mbstring(name)
    b += struct.pack("<6f", *(bmin + bmax))
    b += struct.pack("<f", 6.0)
    for lo, hi in ((8, 8), (2, 2), (6, 6)):
        b += struct.pack("<ii", lo, hi)
    return b


def part_payload(children=()):
    """Ref 6.1.1.5 Part Node Element: Meta Data Node Data (which is Group Node
    Data + U8 meta-data version) then U8 part version + I32 empty field."""
    return group_node_data(children) + struct.pack("<BBi", 1, 1, 0)


def string_prop_payload(value):
    """Ref 6.2.2 String Property Atom: Base Property Atom Data (U8 version +
    U32 state flags) then U8 version + MbString value."""
    return struct.pack("<BI", 1, 0) + struct.pack("<B", 1) + mbstring(value)


def late_load_payload(seg_gid, seg_type, payload_oid):
    """Ref 6.2.7 Late Loaded Property Atom: U8 version + GUID segment id +
    I32 segment type + I32 payload object id + I32 reserved (>=1)."""
    return struct.pack("<B", 1) + seg_gid + struct.pack("<iii", seg_type, payload_oid, 1)


def primset_box_payload(bmin, lengths, rgb):
    """Ref 7.2 Primitive Set Shape Element with one raw Box primitive."""
    record = struct.pack("<i", 0)                    # reserved
    record += struct.pack("<3f", *bmin)              # params1 min corner
    record += struct.pack("<3f", *lengths)           # params2 box lengths
    record += struct.pack("<4f", 1.0, 0.0, 0.0, 0.0)  # params3 quaternion (id)
    record += struct.pack("<3f", *rgb)               # colour
    record += struct.pack("<i", 0)                   # type = box
    payload = struct.pack("<BB", 1, 1)               # base shape + primset ver
    payload += struct.pack("<Q", 0)                  # vertex bindings
    payload += struct.pack("<B", 1)                  # primset version (=1)
    payload += struct.pack("<i", 0)                  # bits per vertex = 0
    payload += struct.pack("<b", 0)                  # texcoord gen type
    payload += struct.pack("<i", len(record))        # uncompressed data size
    payload += struct.pack("<i", -len(record))       # comp size < 0 => raw
    payload += record
    return payload


def version_bytes():
    """80-char version with the ASCII/binary translation bytes at the end."""
    v = b"Version 10.0 JT DM 8.0.5.0"
    return v[:75].ljust(75, b" ") + b"\x20\x0a\x0d\x0a\x20"


def integer_prop_payload(value):
    """Ref 6.2.3 Integer Property Atom Element: Base Property Atom Data (U8
    version + U32 state flags) then U8 version + I32 value. Used as an
    anonymous key in the property table pairing."""
    return struct.pack("<BI", 1, 0) + struct.pack("<Bi", 1, value)


def property_table(element_id, pairs):
    """Ref 6.3 Property Table: I16 version + I32 element property table count
    then, per element, the I32 element object id and a list of key/value
    property atom object id pairs terminated by a key of 0.

    `pairs` is a list of (key_atom_id, value_atom_id). The reader classifies
    each value atom by its own object type (a String Property Atom provides the
    node name; a Late Loaded Property Atom provides the shape reference), so
    the key atoms here are anonymous positional keys required by the list
    structure."""
    b = struct.pack("<hi", 1, 1)
    b += struct.pack("<i", element_id)
    for key_id, value_id in pairs:
        b += struct.pack("<ii", key_id, value_id)
    b += struct.pack("<i", 0)  # list terminator (key id 0, no value)
    return b


def build_golden_box():
    """A JT file: header, one LSG segment, one Shape LOD0 segment, carrying a
    single box primitive at (0,0,0)..(1,2,3) named 'golden-box-a', linked from
    the LSG partition's children and a matching String Property Atom."""
    lsg_gid = guid_le(0x00000001, 0x0000, 0x1111, list(range(8)))
    shape_gid = guid_le(0x00000002, 0x0000, 0x2222, list(range(8, 16)))

    lsg_segment = lsg_gid + struct.pack("<ii", SEGMENT_LSG, HEADER_SIZE + 4)
    # (segment length is patched after assembling the body; the +4 is the
    # GUID independent, we re-assemble below anyway)

    els = []
    els.append(first_element_compressed_body(GID_PARTITION, BASE_GRAPH_NODE, 1,
                                             partition_payload((2,), "golden-box-a", (0, 0, 0), (1, 2, 3))))
    els.append(element_compressed_body(GID_PART, BASE_GRAPH_NODE, 2, part_payload()))
    els.append(element_compressed_body(GID_STRING_PROP, BASE_PROPERTY, 3,
                                       string_prop_payload("golden-box-a")))
    els.append(element_compressed_body(GID_LATE_LOAD, BASE_PROPERTY, 4,
                                       late_load_payload(shape_gid, SEGMENT_SHAPE_LOD0, 100)))
    # Two anonymous integer-prop keys pair the part node with its name value
    # atom (3) and its shape reference value atom (4).
    els.append(element_compressed_body(GID_INT_PROP, BASE_PROPERTY, 10, integer_prop_payload(1)))
    els.append(element_compressed_body(GID_INT_PROP, BASE_PROPERTY, 11, integer_prop_payload(2)))
    lsg_data = b"".join(els) + GID_END + property_table(2, [(10, 3), (11, 4)])
    lsg_seg = lsg_gid + struct.pack("<ii", SEGMENT_LSG, 24 + len(lsg_data)) + lsg_data

    prim_data = element_body(GID_PRIM_SET, 4, 100,
                             primset_box_payload((0, 0, 0), (1, 2, 3), (0.8, 0.5, 0.2))) + GID_END
    shape_seg = shape_gid + struct.pack("<ii", SEGMENT_SHAPE_LOD0, 24 + len(prim_data)) + prim_data

    return assemble([(lsg_gid, SEGMENT_LSG, lsg_seg), (shape_gid, SEGMENT_SHAPE_LOD0, shape_seg)], lsg_gid)


def assemble(segments, lsg_gid):
    """segments: list of (guid, type, full_segment_bytes)."""
    toc_size = 4 + 32 * len(segments)
    cursor = HEADER_SIZE + toc_size
    placed = []
    for gid, seg_type, body in segments:
        placed.append((gid, seg_type, cursor, body))
        cursor += len(body)

    header = version_bytes()
    header += struct.pack("<B", 0)       # byte order = LSB first
    header += struct.pack("<i", 0)       # empty field
    header += struct.pack("<Q", HEADER_SIZE)  # TOC offset
    header += lsg_gid
    header += b"\x00" * 16               # empty GUID

    toc = struct.pack("<i", len(segments))
    for gid, seg_type, off, body in placed:
        toc += gid + struct.pack("<Q", off) + struct.pack("<I", len(body)) + struct.pack("<I", seg_type << 24)

    return header + toc + b"".join(b for _, _, _, b in placed)


def corrupt_variants(golden):
    """Corrupt fixtures: each must hit a typed error, never a panic."""
    out = {}

    # Not a JT file (no version magic).
    out["junk-not-jt.bin"] = b"this is not a jt file at all\x00\x00"

    # Empty input.
    out["empty.jt"] = b""

    # Truncated header.
    out["truncated-header.jt"] = golden[:40]

    # TOC offset out of range.
    hdr = version_bytes() + struct.pack("<B", 0) + struct.pack("<i", 0) + struct.pack("<Q", 1 << 30)
    hdr += guid_le(0x00000001, 0x0000, 0x1111, list(range(8))) + b"\x00" * 16
    out["toc-offset-oob.jt"] = hdr

    # Invalid byte order value (only 0/1 are documented).
    hdr2 = version_bytes() + struct.pack("<B", 3) + struct.pack("<i", 0) + struct.pack("<Q", HEADER_SIZE)
    hdr2 += guid_le(0x00000001, 0x0000, 0x1111, list(range(8))) + b"\x00" * 16
    out["bad-byte-order.jt"] = hdr2

    # Segment length that overruns the file.
    lsg_gid = guid_le(0x00000001, 0x0000, 0x1111, list(range(8)))
    shape_gid = guid_le(0x00000002, 0x0000, 0x2222, list(range(8, 16)))
    lsg_seg = lsg_gid + struct.pack("<ii", SEGMENT_LSG, 1 << 20) + b"\x00" * 64
    out["segment-overrun.jt"] = assemble(
        [(lsg_gid, SEGMENT_LSG, lsg_seg), (shape_gid, SEGMENT_SHAPE_LOD0, b"\x00" * 24)], lsg_gid)

    # Root block must be kind 1 (LSG); a kind-99 root is a typed error.
    root_not_lsg = lsg_gid + struct.pack("<ii", 99, 24 + 64) + b"\x00" * 64
    out["root-not-lsg.jt"] = assemble(
        [(lsg_gid, 99, root_not_lsg), (shape_gid, SEGMENT_SHAPE_LOD0, b"\x00" * 24)], lsg_gid)

    return out


def write_fixtures(outdir=CORPUS):
    os.makedirs(outdir, exist_ok=True)
    golden = build_golden_box()
    with open(os.path.join(outdir, "golden-box-a.jt"), "wb") as fh:
        fh.write(golden)
    for name, data in corrupt_variants(golden).items():
        with open(os.path.join(outdir, name), "wb") as fh:
            fh.write(data)
    print("wrote %d fixtures to %s" % (1 + len(corrupt_variants(golden)), outdir))
    return golden


if __name__ == "__main__":
    write_fixtures()
