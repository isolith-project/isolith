#!/usr/bin/env python3
"""Independent Python reference reader for the P2-M7-T01 CGR triangle-stream
record locator (OBSERVED-reference path).

This is the second, fully independent implementation (pure stdlib) of the
polygon-soup record grammar measured on the real corpus (see
crates/oep-cgr/src/locate.rs). Two readers agreeing on the same real bytes is
the program's "measured, not guessed" bar:

    u16 BE  N                       face count        (2..=500)
    N x     u16 BE per-face counts (3..=16)
    S x     u16 BE handles         (S = sum(counts))  h % 3 == 0,
                                                      true vertex index = h / 3

CLI: python -m oep_tooling.cgr_locate_reference <file.cgr>
prints `index_count V extract ...` numeric lines consumed by the committed
numeric-epsilon golden (1e-6 tolerance).
"""
import sys


def be16(b: bytes, o: int) -> int:
    return (b[o] << 8) | b[o + 1]


def spine_ok(b: bytes) -> bool:
    TEMPLATE = bytes([0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00,
                      0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F])
    if len(b) < 0x78:
        return False
    if b[0:4] != b"\x01\x00\x00\x00":
        return False
    if b[4:0x64] != bytes(0x60):
        return False
    if b[0x68:0x78] != TEMPLATE:
        return False
    return True


def locate(b: bytes, min_handles: int = 30):
    """Return list of records dicts (mirrors crates/oep-cgr/src/locate.rs)."""
    if not spine_ok(b):
        return None  # NotCgrPreamble
    n = len(b)
    out = []
    i = 0x78
    while i + 4 < n:
        N = be16(b, i)
        if 2 <= N <= 500 and i + 2 + 2 * N + 4 < n:
            counts = [be16(b, i + 2 + 2 * k) for k in range(N)]
            if all(3 <= c <= 16 for c in counts):
                S = sum(counts)
                ho = i + 2 + 2 * N
                if S >= min_handles and ho + 2 * S <= n:
                    hs = [be16(b, ho + 2 * k) for k in range(S)]
                    if all(h % 3 == 0 and h // 3 < 0x8000 for h in hs):
                        delt = [h // 3 for h in hs]
                        out.append({
                            "offset": i,
                            "face_count": len(counts),
                            "counts": counts,
                            "index_count": S,
                            "vertex_count": len(set(delt)),
                            "triangle_count": sum(c - 2 for c in counts),
                            "max_index": max(delt),
                        })
                        i = ho + 2 * S
                        continue
        i += 2
    return out


def main(argv):
    if len(argv) != 2:
        print("usage: python -m oep_tooling.cgr_locate_reference <file.cgr>", file=sys.stderr)
        return 2
    data = open(argv[1], "rb").read()
    records = locate(data)
    if records is None:
        print("0")
        print("0")
        print("0")
        print("0")
        print("0")
        print("0")
        return 0
    # Numeric-only output (consumed by the committed numeric-epsilon golden,
    # compare=numeric requires every token to be a float):
    #   records_located, anchor_present, index_count, vertex_count,
    #   triangle_count, max_index
    print(len(records))
    anchor = [r for r in records if r["offset"] == 0xA0DC]
    print(len(anchor))
    if anchor:
        r = anchor[0]
        print(r["index_count"])
        print(r["vertex_count"])
        print(r["triangle_count"])
        print(r["max_index"])
    else:
        print(0)
        print(0)
        print(0)
        print(0)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
