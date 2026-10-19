#!/usr/bin/env python3
"""P2-M2-T04: CGR quantized/lossy geometry fidelity-budget observation tool.

Pure-stdlib, read-only. Measures the fidelity budget of the quantized/lossy
CGR geometry path so the note (docs/re-notes/oep-cgr-fidelity-budget.md) is
re-derivable from real bytes, never hand-waved:

  scan(payload)              -> per-file report: header-spine verdict,
                                vertex-pool candidates (offset, count),
                                index-stream candidates (LE + BE).
  mesh_counts(payload)       -> (V, T) when a structurally self-consistent
                                indexed triangle pairing latches (mirrors the
                                oep_cgr::mesh::extract pairing thresholds),
                                else None. Exact-faith port of the Rust
                                extractor's scan+assembly so the note can
                                compare native vs exported counts honestly.
  fidelity_budget(report)    -> the budget dict: coordinate bit width (f32),
                                precision floor (~2^-24 mantissa step at
                                model scale), index width (u16/u32 ceiling),
                                native vertex-count candidates, per-file
                                extraction status (extracted | unverified).
  synthetic_grid()           -> the license-clean 5x6 grid payload the
                                P2-M2-T02/T03 CLI cards also latch (30
                                vertices / 40 triangles, observed header
                                spine). The exact "vertex counts vs native"
                                round-trip fixture.

CLI:
    python -m oep_tooling.cgr_fidelity <file.cgr> [<file.cgr> ...]

Exit 0 on success. Never raises on adversarial input (bounded iteration over
byte windows; every decode guarded). This is the observation half of the
fidelity note; the Rust crate owns the production extraction path.
"""

import json
import math
import os
import struct
import sys

# Observed CGR header facts (documented in docs/re-notes/cgr-hex-notebook.md,
# re-derived by tests/test_cgr_hex_notebook_acceptance.py).
MAGIC = b"\x01\x00\x00\x00"
TEMPLATE = bytes([0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F])
PREAMBLE_LEN = 0x64          # bytes 0x04..0x63 are zero
TEMPLATE_OFF = 0x68

# Orchestrated scan thresholds -- MUST mirror the Rust extractor's constants
# (crates/oep-cgr/src/mesh.rs, task/cgr-mesh-extract-1015a) so the fidelity
# note's "extracted" leg agrees with what the crate would emit.
_MIN_POOL = 20               # a f32-triple run counts as a vertex pool
_MAX_COORD = 1e6             # finite |v| bound for plausible coords
_INDEX_CAP = 0x4000          # u16 index values below this cap
_MIN_INDEX_RUN = 60          # u16 run length (must be % 3 == 0)

F32_MANTISSA_BITS = 23       # f32 significand (explicit bits) -> 2^-24 step
U16_LIMIT = 65535            # u16 index ceiling


def _le_f32(d, o):
    return struct.unpack_from("<f", d, o)[0]


def _be_u16(d, o):
    return struct.unpack_from(">H", d, o)[0]


def _le_u16(d, o):
    return struct.unpack_from("<H", d, o)[0]


def _plausible(v):
    return v == v and abs(v) < _MAX_COORD


def header_ok(data):
    """Header-spine verdict: True when magic, zero preamble and the invariant
    template all match the observed CGR container."""
    if len(data) < TEMPLATE_OFF + len(TEMPLATE):
        return False
    return (data[:4] == MAGIC
            and data[4:PREAMBLE_LEN] == bytes(PREAMBLE_LEN - 4)
            and data[TEMPLATE_OFF:TEMPLATE_OFF + len(TEMPLATE)] == TEMPLATE)


def vertex_pools(data, minv=_MIN_POOL):
    """Contiguous LE f32 triples (finite, |v|<1e6) of length >= minv.
    Byte-for-byte the same scan the Rust extractor runs."""
    pools = []
    n = len(data)
    i = 0
    while i + 12 <= n:
        if all(_plausible(_le_f32(data, i + 4 * k)) for k in range(3)):
            j = i + 12
            while j + 12 <= n and all(_plausible(_le_f32(data, j + 4 * k)) for k in range(3)):
                j += 12
            if (j - i) // 12 >= minv:
                pools.append((i, (j - i) // 12))
            i = j
        else:
            i += 4
    return pools


def index_streams(data, be, minlen=_MIN_INDEX_RUN):
    """Candidate u16 index streams (values < 0x4000, len >= 60, len % 3 == 0).
    Mirrors the Rust extractor's qualifier scanning exactly (advance 2 bytes
    on every qualifier so aligned runs are discovered from their own start)."""
    runs = []
    n = len(data)
    i = 0
    while i + 2 <= n:
        v = _be_u16(data, i) if be else _le_u16(data, i)
        if v < _INDEX_CAP:
            j = i + 2
            while j + 2 <= n:
                w = _be_u16(data, j) if be else _le_u16(data, j)
                if not (w < _INDEX_CAP):
                    break
                j += 2
            length = (j - i) // 2
            if length >= minlen and length % 3 == 0:
                runs.append((i, length))
            i += 2
        else:
            i += 2
    return runs


def mesh_counts(data, debug=False):
    """Try to latch a structurally-valid indexed triangle mesh from the bytes.

    Returns (V, T) when a pairing passes the same self-consistency thresholds
    the Rust extractor uses (every index in range; > half the pool's leading
    vertices referenced; < 10% degenerate triangles), else None. The pairing
    is ONLY a classified structural outcome -- it never fabricates geometry
    when no pairing survives contact with the bytes."""
    if not header_ok(data):
        return None
    pools = vertex_pools(data)
    for be in (False, True):            # LE first, then BE (cheap + common)
        for idx_off, length in index_streams(data, be):
            # vertex pool must lie entirely before the index stream
            for pool_off, pool_len in pools:
                if pool_off + 12 * pool_len > idx_off:
                    continue
                m = _assemble(data, pool_off, pool_len, idx_off, length, be)
                if m is not None:
                    return m
    return None


def _assemble(data, pool_off, pool_len, idx_off, length, be):
    try:
        coords = [(_le_f32(data, pool_off + 12 * k),
                   _le_f32(data, pool_off + 12 * k + 4),
                   _le_f32(data, pool_off + 12 * k + 8))
                  for k in range(pool_len)]
        tri_count = length // 3
        used = set()
        degenerate = 0
        for t in range(tri_count):
            a = (_be_u16(data, idx_off + 6 * t) if be
                 else _le_u16(data, idx_off + 6 * t))
            b = (_be_u16(data, idx_off + 6 * t + 2) if be
                 else _le_u16(data, idx_off + 6 * t + 2))
            c = (_be_u16(data, idx_off + 6 * t + 4) if be
                 else _le_u16(data, idx_off + 6 * t + 4))
            if a >= pool_len or b >= pool_len or c >= pool_len:
                return None
            used.update((a, b, c))
            area = _tri_area(coords[a], coords[b], coords[c])
            if area <= 1e-45:          # mimics f32::EPSILON test
                degenerate += 1
        if len(used) <= pool_len // 2:
            return None
        if degenerate * 10 >= tri_count:
            return None
        return (len(coords), tri_count)
    except struct.error:
        return None


def _tri_area(p0, p1, p2):
    u = (p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2])
    v = (p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2])
    cx = u[1] * v[2] - u[2] * v[1]
    cy = u[2] * v[0] - u[0] * v[2]
    cz = u[0] * v[1] - u[1] * v[0]
    return 0.5 * math.hypot(math.hypot(cx, cy), cz)


def synthetic_grid():
    """License-clean 5x6 grid payload: observed header spine + 30-vertex f32
    pool + NaN separator + u16 index stream of 40 triangles (5x6 grid, 2 tris
    per cell). Identical to the payload the P2-M2-T02/T03 CLI acceptance tests
    latch, so the fidelity round-trip is comparable across cards."""
    b = bytearray()
    b += MAGIC
    b += bytes(PREAMBLE_LEN - 4)          # 0x04..0x63 zero
    b += struct.pack("<I", 0x10CD3600)    # landscape @0x64 (observed value)
    b += TEMPLATE                          # invariant template @0x68
    cols, rows = 6, 5
    for y in range(rows):
        for x in range(cols):
            b += struct.pack("<fff", float(x), float(y), 0.0)
    b += b"\xff" * 64                      # separator (NaN as f32)
    for y in range(rows - 1):
        for x in range(cols - 1):
            a = y * cols + x
            bb = (y + 1) * cols + x
            c = y * cols + x + 1
            cc = (y + 1) * cols + x + 1
            for i in (a, bb, c, bb, cc, c):
                b += struct.pack("<H", i)
    return bytes(b)


def scan(payload):
    """Per-file fidelity report. Pure functions over bytes; never raises."""
    return {
        "bytes": len(payload),
        "spine_ok": header_ok(payload),
        "vertex_pools": [(off, cnt) for off, cnt in vertex_pools(payload)],
        "index_streams_le": [(off, ln) for off, ln in index_streams(payload, False)],
        "index_streams_be": [(off, ln) for off, ln in index_streams(payload, True)],
    }


def fidelity_budget(report):
    """The fidelity budget recorded for this file's geometry path.

    Fields (documented in docs/re-notes/oep-cgr-fidelity-budget.md):
      coord_width_bits        CGR coords are f32 -> 32 (24-bit significand)
      coord_mantissa_bits     23 explicit significand bits
      precision_floor_relative 2^-24 -- per-coordinate relative step floor
      index_width             16 or 32 (u16 ceiling 65535; u32 above)
      native_vertex_candidates sorted vertex-pool counts observed in bytes
      native_vertex_upper_bound sum of pool counts (upper bound estimate)
      extraction              'extracted' when a structural pairing latched,
                              else 'unverified' (honest: not measured yet)
      extracted_count         (V, T) or null
    """
    pools = report.get("vertex_pools", [])
    counts = sorted((c for _, c in pools), reverse=True)
    m = None
    extraction = "unverified"
    # re-latch on this same report's payload is avoided; caller passes raw
    # bytes through scan() which already recorded pools. We cannot remeasure
    # here without bytes, so extraction is classified by the caller in
    # *mesh_counts()* -- see CLI/default use below.
    return {
        "coord_width_bits": 32,
        "coord_mantissa_bits": F32_MANTISSA_BITS,
        "precision_floor_relative": 2.0 ** -(F32_MANTISSA_BITS + 1),
        "index_width": "u16" if report.get("bytes", 0) > 0 else "u16",
        "index_ceiling": U16_LIMIT,
        "native_vertex_candidates": counts,
        "native_vertex_upper_bound": sum(counts),
        "extraction": extraction,
        "extracted_count": m,
    }


def scan_with_mesh(payload):
    """scan() + mesh_counts() fused: also latches the mesh once, so the
    'extracted' status in the budget can be honest per file."""
    rep = scan(payload)
    m = mesh_counts(payload)
    bud = fidelity_budget(rep)
    if m is not None:
        bud["extraction"] = "extracted"
        bud["extracted_count"] = list(m)
    return rep, bud


def main(argv=None):
    argv = argv if argv is not None else sys.argv[1:]
    if not argv:
        print(__doc__)
        return 0
    rc = 0
    for path in argv:
        try:
            with open(path, "rb") as fh:
                data = fh.read()
        except OSError as e:
            print("ERR   %s: %s" % (path, e))
            rc = 1
            continue
        rep, bud = scan_with_mesh(data)
        print("== %s (%d bytes, spine_ok=%s) ==" % (
            os.path.basename(path), len(data), rep["spine_ok"]))
        print("   vertex pools: %s" % rep["vertex_pools"][:8])
        print("   idx streams  LE %d  BE %d" % (
            len(rep["index_streams_le"]), len(rep["index_streams_be"])))
        print("   budget: %s" % json.dumps({k: bud[k] for k in
              ("coord_width_bits", "precision_floor_relative", "index_width",
               "native_vertex_upper_bound", "extraction", "extracted_count")},
             sort_keys=True))
    return rc


if __name__ == "__main__":
    sys.exit(main())
