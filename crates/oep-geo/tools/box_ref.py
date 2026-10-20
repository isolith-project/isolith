#!/usr/bin/env python3
"""Shared reference tessellation + numeric formatters for P6-M4-T01 encoders.

The CC0-1.0 isolith unit box (`corpus/data/box.stl`, P0-M4-T02 fixture):
8 vertices / 12 triangles. Every exporter golden under
`crates/oep-geo/tools/*` imports POSITIONS / TRIANGLES from here so all six
formats are generated from the SAME reference geometry.

`fmt_f32` reproduces Rust's `{}` on a IEEE-754 single: the SHORTEST decimal
literal that round-trips to the same f32 bit pattern, integer-valued floats
rendered without a decimal point (`0`, `1`, ...). For the reference box every
coordinate is exactly integral, so all text formats emit integer literals.
"""

import struct

POSITIONS = [
    (1.0, 0.0, 0.0),  # V0
    (0.0, 1.0, 0.0),  # V1
    (0.0, 0.0, 0.0),  # V2
    (1.0, 1.0, 0.0),  # V3
    (0.0, 0.0, 1.0),  # V4
    (0.0, 1.0, 1.0),  # V5
    (1.0, 0.0, 1.0),  # V6
    (1.0, 1.0, 1.0),  # V7
]

TRIANGLES = [
    (0, 1, 2),
    (0, 3, 1),
    (4, 5, 6),
    (5, 7, 6),
    (2, 6, 4),
    (2, 0, 6),
    (3, 5, 7),
    (3, 1, 5),
    (5, 1, 2),
    (5, 2, 4),
    (0, 7, 3),
    (0, 6, 7),
]


def to_f32(v):
    """Clamp a float to the nearest IEEE-754 single (Python floats are f64)."""
    return struct.unpack("<f", struct.pack("<f", v))[0]


def fmt_f32(v):
    """Shortest-round-trip f32 literal, matching Rust's f32 Display.

    Integer-valued singles render without a decimal point. Falls back to a
    fraction when the shortest integer form does not round-trip.
    """
    f = to_f32(v)
    if f == 0.0:
        neg = struct.pack("<f", f)[3] & 0x80
        return "-0" if neg else "0"
    if f.is_integer() and abs(f) < 1e16:
        return str(int(f))
    # Shortest decimal precision that round-trips to the same f32.
    for prec in range(1, 10):
        cand = "%.*g" % (prec, f)
        # %g may print very large/small values in exponent form; Rust uses a
        # non-exponent literal in the normal range. Expand for our geometry.
        if "e" in cand or "E" in cand:
            cand = "%.*f" % (prec, f)
            cand = cand.rstrip("0").rstrip(".")
        back = to_f32(float(cand))
        if back == f:
            return cand
    return repr(f)


def corner_set():
    return {tuple(POSITIONS[i]) for t in TRIANGLES for i in t}
