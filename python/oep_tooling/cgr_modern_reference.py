#!/usr/bin/env python3
"""Independent Python reference reader for the P2-M7-T01 modern-era CGR
visualization stream (OBSERVED-reference path, second implementation).

This is the third, fully independent implementation (pure stdlib) of the
modern CATIA V5_CFV2 + cgr-family visualization decode, cross-checked against
`crates/oep-cgr/src/modern.rs` and the 2026-10-16 real-material latch. Two
independent readers agreeing on the same real bytes is the program's
"measured, not guessed" bar.

It decodes, on real (public-domain NIST V5-6R202x) material:

  * the outer `V5_CFV2` container and its logical stream directory
    (spec 3.1 / 3.2 / 3.4 of the lawful cadmpeg catia.md, CC-BY-4.0
    hypothesis source -- every constant re-derived here, no code copied),
  * the reconstructed `MainDataStream` + `SurfacicReps` (BREP stream),
  * the indexed visualization-point coordinate table (spec 5.4.2,
    marker `ff ff 02 00 01 ff`, mode 0 compressed / mode 1 direct),
  * every genuine `0x4x` trim packet satisfying the documented invariant
    `N == 3*A + sum(K)` (spec 5.3) with its `u16be` handle stream.

CLI: python -m oep_tooling.cgr_modern_reference <file.[CATPart|cgr]>

prints numeric lines consumed by the committed numeric-epsilon golden
(compare=numeric, tolerance 1e-6), in the same shape as the piston locator:

  records_located, anchor_present, index_count, vertex_count,
  triangle_count, max_index

All six values are float tokens so a drifted count fails the golden
(no silent pass).

Licensing: samples are dev-only reference material, never shipped. The NIST
STC/FTC/CTC archives used as the modern-era anchor are public-domain US
Government test data; the repo records source + evidence, not the binaries.
"""
import struct
import sys

BE16 = lambda b, o: struct.unpack(">H", b[o:o + 2])[0]
LE32 = lambda b, o: struct.unpack("<I", b[o:o + 4])[0]
LEF32 = lambda b, o: struct.unpack("<f", b[o:o + 4])[0]

OUTER_MAGIC = b"V5_CFV2\0"
DIR_MAGIC = b"CATIA_V5 CB0001\0"


def be32(b: bytes, o: int) -> int:
    return struct.unpack(">I", b[o:o + 4])[0]


def container_info(b: bytes):
    """Return (inner_magic, inner_dir_offset, inner_dir_length) or (None,...)."""
    if not b.startswith(OUTER_MAGIC):
        return None, None, None
    inner = b.find(OUTER_MAGIC, 8)
    if inner < 0 or inner + 16 > len(b):
        return None, None, None
    delta = be32(b, inner + 8)
    length = be32(b, inner + 12)
    return inner, inner + delta, length


def parse_directory(b: bytes, physical_base: int, dir_offset: int, dir_length: int):
    """Parse the stream directory (spec 3.4) via the self-consistency scan.
    Returns list of (name, [(phys_off, phys_len, log_off, log_len, flags)])."""
    if dir_offset + 16 > len(b) or b[dir_offset:dir_offset + 16] != DIR_MAGIC:
        return []
    dirbuf = b[dir_offset:dir_offset + dir_length]
    descriptors = []
    o = 0
    n = len(dirbuf)
    while o + 4 <= n:
        k = be32(dirbuf, o)
        if k != 0 and o + 4 + k * 20 <= n:
            extents = []
            cum = 0
            ok = True
            for i in range(k):
                base = o + 4 + i * 20
                po, pl, ll, lo, fl = struct.unpack(">IIIII", dirbuf[base:base + 20])
                if pl == 0 or physical_base + po + pl > len(b) or ll != pl:
                    ok = False
                    break
                # store as (phys_off, phys_len, log_len, log_off, flags)
                extents.append((po, pl, ll, lo, fl))
                cum += ll
            if ok and cum > 0 and o >= 0x50:
                ds = o - 0x50
                if be32(dirbuf, ds + 12) == cum:
                    name = descriptor_name(dirbuf, ds)
                    if name:
                        descriptors.append((name, extents))
        o += 1
    return descriptors


def descriptor_name(dirbuf: bytes, ds: int):
    if ds >= 3 and dirbuf[ds - 3:ds] == b"\x00\x00\x00":
        name_start = ds - 3
        while name_start >= 2:
            if 0x20 <= dirbuf[name_start - 2] <= 0x7e and dirbuf[name_start - 1] == 0:
                name_start -= 2
            else:
                break
        nb = dirbuf[name_start:ds - 3]
        if len(nb) >= 6:
            return "".join(chr(nb[i]) for i in range(0, len(nb), 2))
    return None


def unique_largest(directory, name):
    cands = [ext for (nm, ext) in directory if nm == name]
    if not cands:
        return None
    lens = [sum(e[1] for e in ext) for ext in cands]
    m = max(lens)
    return cands[lens.index(m)] if lens.count(m) == 1 else None


def reconstruct(b: bytes, inner: int, ext):
    buf = bytearray()
    # extents stored as (phys_off, phys_len, log_len, log_off, flags); sort by log_off
    for (po, pl, ll, lo, fl) in sorted(ext, key=lambda e: e[3]):
        buf += b[inner + po: inner + po + pl]
    return bytes(buf)


def scan_count(b: bytes, i: int):
    if i < len(b) and b[i] == 0xFF:
        return LE32(b, i + 1), i + 5
    return b[i], i + 1


def parse_trim(s: bytes, pos: int):
    """A genuine 0x4x trim packet satisfying N == 3A + sum(K)."""
    if pos + 2 > len(s) or s[pos] != 0x01 or not (0x40 <= s[pos + 1] <= 0x4F):
        return None
    kind = s[pos + 1]
    mask = kind & 0x0F
    kc = pos + 2
    A, kc = scan_count(s, kc)
    B, kc = scan_count(s, kc)
    C, kc = scan_count(s, kc)
    if None in (A, B, C) or kc + 1 > len(s) or s[kc] != 0xFF:
        return None
    N = LE32(s, kc + 1)
    kc += 5
    if mask & 0x08:
        kc += 12
    K = []
    for _ in range(B + C):
        v, kc = scan_count(s, kc)
        if v is None:
            return None
        K.append(v)
    if N != 3 * A + sum(K):
        return None
    tri = A + sum(max(0, x - 2) for x in K[:B]) + sum(max(0, f - 2) for f in K[B:B + C])
    return {"pos": pos, "kind": kind, "A": A, "B": B, "C": C, "K": K, "N": N,
            "hstart": kc, "tri": tri}


def decode_viz_table(brep: bytes):
    """Coordinate pool from the indexed visualization-point table (5.4.2)."""
    marker = b"\xff\xff\x02\x00\x01\xff"
    oi = brep.find(marker)
    if oi < 0:
        return None
    entry_count = LE32(brep, oi + 6)
    mode = brep[oi + 18]
    if mode == 1:
        coords = []
        for e in range(entry_count):
            off = oi + 19 + 12 * e
            coords.append([LEF32(brep, off + 4 * k) for k in range(3)])
        return coords
    if mode == 0:
        ctl_len = (entry_count + 3) // 4
        ctl = brep[oi + 19:oi + 19 + ctl_len]
        j = oi + 19 + ctl_len
        if j >= len(brep) or brep[j] != 0xFF:
            return None
        sc = LE32(brep, j + 1)
        j += 5
        scalars = [LEF32(brep, j + 4 * k) for k in range(sc)]
        rows = []
        prev = [0.0, 0.0, 0.0]
        si = 0
        for i in range(entry_count):
            code = (ctl[i // 4] >> (2 * (i % 4))) & 0x3
            if i == 0:
                code = 0
            x, y, z = prev
            if code == 0:
                x, y, z = scalars[si], scalars[si + 1], scalars[si + 2]
                si += 3
            elif code == 1:
                pass
            elif code == 2:
                z = scalars[si]
                si += 1
            elif code == 3:
                y, z = scalars[si], scalars[si + 1]
                si += 2
            rows.append([x, y, z])
            prev = [x, y, z]
        if si != sc:
            return None
        return rows
    return None


def extract(data: bytes):
    """Return (records, anchor_present, index_count, vertex_count,
    triangle_count, max_index) or None. Mirrors crates/oep-cgr/src/modern.rs."""
    inner, dir_off, dir_len = container_info(data)
    if inner is None or dir_off is None or dir_len is None:
        return None
    directory = parse_directory(data, inner, dir_off, dir_len)
    main = unique_largest(directory, "MainDataStream")
    surf = unique_largest(directory, "SurfacicReps")
    if not main or not surf:
        return None
    main_stream = reconstruct(data, inner, main)
    surf_stream = reconstruct(data, inner, surf)
    brep = main_stream + surf_stream
    coords = decode_viz_table(brep)
    anchor_present = 1 if coords else 0

    packets = []
    for i in range(len(main_stream) - 2):
        r = parse_trim(main_stream, i)
        if r:
            packets.append(r)
    records = len(packets)
    index_count = sum(p["N"] for p in packets)
    triangle_count = sum(p["tri"] for p in packets)
    handles = []
    for p in packets:
        i = p["hstart"]
        for _ in range(p["N"]):
            if i + 2 > len(main_stream):
                break
            handles.append(BE16(main_stream, i))
            i += 2
    vertex_count = len(set(handles)) if handles else 0
    max_index = max(handles) if handles else 0
    return records, anchor_present, index_count, vertex_count, triangle_count, max_index


def main(argv):
    if len(argv) != 2:
        print("usage: python -m oep_tooling.cgr_modern_reference <file>", file=sys.stderr)
        return 2
    data = open(argv[1], "rb").read()
    r = extract(data)
    if r is None:
        # NotCgrContainer / no BREP stream -> honest 0,0,0,0,0,0
        for _ in range(6):
            print("0")
        return 0
    for v in r:
        print(v)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
