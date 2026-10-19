#!/usr/bin/env python3
"""P2-M3-T01: CGR vertex-attribute / normal record observation tool.

Pure-stdlib, read-only is NOT enough — this tool actively classifies whether
real CGR material carries recoverable per-vertex attribute records. It is the
"observation" half of docs/re-notes/oep-cgr-vertex-attributes.md and exists
to re-derive the note's facts from real bytes (acceptance-testable, per the
P1/P2 observation-tool pattern).

CLI:
    python -m oep_tooling.cgr_attributes <file.cgr> [<file.cgr> ...]

Exit 0 = scanned. Prints per file:
  * the observed header spine verdict (magic/preamble/template)
  * how many mesh-like vertex pools are found (structural, P2-M2-T01 style)
  * whether the piston 20 41 01 ff ff ef d1 ca [50|51] record marker occurs
  * whether raw unit-length f32 normal runs occur (>=8)
  * whether any quantized-3-byte normal-ish runs exist (>=8)
so the absence/presence classification in the acceptance test is anchored to
real bytes, never invented.
"""
import math
import os
import struct
import sys

MAGIC = b"\x01\x00\x00\x00"
TEMPLATE = bytes([0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F])
MARKER = b"\x20\x41\x01\xff\xff\xef\xd1\xca"


def f32(d, o):
    return struct.unpack_from("<f", d, o)[0]


def vertex_pools(d, minv=20):
    """Contiguous LE f32 triples (finite, |v|<1e5) of length >= minv."""
    pools = []
    n = len(d)
    i = 0
    while i + 12 <= n:
        ok = all((lambda v: (v == v) and abs(v) < 1e5)(f32(d, i + 4 * k)) for k in range(3))
        if ok:
            j = i + 12
            while j + 12 <= n and all((lambda v: (v == v) and abs(v) < 1e5)(f32(d, j + 4 * k)) for k in range(3)):
                j += 12
            if (j - i) // 12 >= minv:
                pools.append((i, (j - i) // 12))
            i = j
        else:
            i += 4
    return pools


def unit_normal_runs(d):
    """Maximal runs where every aligned f32 triple is unit-length."""
    runs = []
    n = len(d)
    i = 0x78
    while i + 12 <= n:
        x, y, z = f32(d, i), f32(d, i + 4), f32(d, i + 8)
        m = math.hypot(math.hypot(x, y), z)
        if abs(m - 1.0) < 2e-3 and m > 1e-6:
            j = i + 12
            while j + 12 <= n:
                x, y, z = f32(d, j), f32(d, j + 4), f32(d, j + 8)
                m2 = math.hypot(math.hypot(x, y), z)
                if abs(m2 - 1.0) >= 2e-3 or m2 <= 1e-6:
                    break
                j += 12
            if (j - i) // 12 >= 8:
                runs.append((i, (j - i) // 12))
            i = j
        else:
            i += 4
    return runs


def quant3_normal_runs(d, minv=8):
    """3-byte quantized normal-ish runs (dequant [0,255]->[-1,1], |n| in 0.05..2).
    NOTE: this is a WEAK positive filter and known to over-fire on dense
    payload noise; the notebook treats it as suspect. Reported for honesty."""
    runs = []
    n = len(d)
    i = 0x78
    while i + 3 <= n:
        def qnorm(o):
            if o + 3 > n:
                return None
            nx = (d[o] - 127.5) / 127.5
            ny = (d[o + 1] - 127.5) / 127.5
            nz = (d[o + 2] - 127.5) / 127.5
            m = math.hypot(math.hypot(nx, ny), nz)
            return m
        m = qnorm(i)
        if m is not None and 0.05 <= m <= 2.0:
            j = i + 3
            while j + 3 <= n:
                m2 = qnorm(j)
                if m2 is None or not (0.05 <= m2 <= 2.0):
                    break
                j += 3
            if (j - i) // 3 >= minv:
                runs.append((i, (j - i) // 3))
            i = j
        else:
            i += 1
    return runs


def scan(path):
    with open(path, "rb") as fh:
        d = fh.read()
    n = len(d)
    print("== %s  (%d bytes, 0x%x) ==" % (os.path.basename(path), n, n))
    header_ok = (d[:4] == MAGIC and d[4:0x64] == bytes(0x60)
                 and n >= 0x78 and d[0x68:0x78] == TEMPLATE)
    print("  header spine ok        : %s" % header_ok)
    pools = vertex_pools(d)
    print("  vertex pools (>=20)    : %d  %s" % (len(pools),
          " ".join("0x%x:%d" % p for p in pools[:6])))
    nmark = d.count(MARKER)
    print("  piston record marker   : %d hits" % nmark)
    ur = unit_normal_runs(d)
    print("  unit-f32 normal runs   : %d  %s" % (len(ur),
          " ".join("0x%x:%d" % r for r in ur[:6])))
    q3 = quant3_normal_runs(d)
    print("  quant-3b normal-ish runs (weak filter): %d  %s" % (len(q3),
          " ".join("0x%x:%d" % r for r in q3[:6])))
    return header_ok


def main(argv):
    rc = 0
    if not argv:
        print(__doc__)
        return 2
    for p in argv:
        if not os.path.isfile(p):
            print("missing file: %s" % p, file=sys.stderr)
            rc = 1
            continue
        try:
            scan(p)
        except Exception as e:
            print("  ERROR scanning %s: %r" % (p, e))
            rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
