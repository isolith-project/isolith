#!/usr/bin/env python3
"""P4-M3-T02: generate the AP214 OCC-conventional conformance reference.

Rewires the committed exporter output (`box-reference.step`, a faceted
SHELL_BASED_SURFACE_MODEL wired via SHAPE_REPRESENTATION_RELATIONSHIP) into the
AP214 representation wiring that Open CASCADE's STEPControl_Reader actually
transfers (verified on real OCCT 7.6 and FreeCAD 1.1):

  SHAPE_REPRESENTATION + SHAPE_REPRESENTATION_RELATIONSHIP
      -> MANIFOLD_SURFACE_SHAPE_REPRESENTATION + SHAPE_DEFINITION_REPRESENTATION

This is a WIRING-ONLY transform: every geometry entity (CARTESIAN_POINT /
DIRECTION / AXIS2_PLACEMENT_3D / PLANE / POLY_LOOP / FACE_OUTER_BOUND /
FACE_SURFACE / CLOSED_SHELL / units / context) is copied byte-for-byte from the
committed export. The reference therefore proves the exporter's GEOMETRY is
sound and isolates the drift to the representation wiring — the target future
exporter work (P4-M3-T01 / P6-M4-T01 follow-up) must converge to.

Deterministic transform on the LAST data line pair; will fail loudly if the
committed export's tail changes shape.

Output: corpus/reference/box-AP214-convention.step
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SRC = REPO / "crates" / "oep-geo" / "tests" / "golden" / "box-reference.step"
OUT = REPO / "corpus" / "reference" / "box-AP214-convention.step"

_RE_SHAPE_REP = re.compile(
    r"#\d+=SHAPE_REPRESENTATION\('([^']*)','([^']*)',\(#(\d+)\),#(\d+)\);\s*$")
_RE_REL = re.compile(
    r"#\d+=SHAPE_REPRESENTATION_RELATIONSHIP\('','',#(\d+),#(\d+)\);\s*$")


def transform(text: str) -> str:
    """Apply the documented AP214 wiring transform."""
    data_at = text.find("DATA;")
    if data_at < 0:
        raise SystemExit("no DATA section in source export")
    blob = text[data_at + len("DATA;"):]
    end_at = blob.find("ENDSEC;")
    if end_at < 0:
        raise SystemExit("no ENDSEC after DATA section")
    blob = blob[:end_at]
    head = text[:data_at + len("DATA;")]
    lines = blob.splitlines(keepends=True)
    # last two non-empty data lines must be the representation + relationship
    ends = [i for i, ln in enumerate(lines) if ln.strip()]
    if len(ends) < 2:
        raise SystemExit("source export has no trailing representation rows")
    rep_i, rel_i = ends[-2], ends[-1]
    rep_line, rel_line = lines[rep_i], lines[rel_i]
    m_rep = _RE_SHAPE_REP.match(rep_line.strip())
    m_rel = _RE_REL.match(rel_line.strip())
    if not m_rep or not m_rel:
        raise SystemExit(
            "source export tail not the expected SHAPE_REPRESENTATION +\n"
            "SHAPE_REPRESENTATION_RELATIONSHIP pair:\n"
            "  %r\n  %r" % (rep_line.strip(), rel_line.strip()))
    name = m_rep.group(1)
    item_id, ctx_id = m_rep.group(3), m_rep.group(4)
    pds_id, rep_ref = m_rel.group(1), m_rel.group(2)
    rel_id = m_rel.string.split("=", 1)[0].lstrip("#")

    # Keep the representation entity id (#135 in our export), rewrite the body
    # to the OCC-transferable AP214 form; the relationship line keeps its own
    # id (#136) and becomes SHAPE_DEFINITION_REPRESENTATION(#pds,#rep).
    rep_id = rep_ref
    new_rep = ("#%s=MANIFOLD_SURFACE_SHAPE_REPRESENTATION('%s',(#%s),#%s);\n"
               % (rep_id, name, item_id, ctx_id))
    new_rel = "#%s=SHAPE_DEFINITION_REPRESENTATION(#%s,#%s);\n" % (
        rel_id, pds_id, rep_id)
    lines[rep_i] = new_rep
    lines[rel_i] = new_rel
    return head + "".join(lines) + "ENDSEC;\nEND-ISO-10303-21;\n"



def main() -> int:
    text = SRC.read_text(encoding="utf-8")
    out_text = transform(text)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(out_text, encoding="utf-8")
    print("wrote %s (%d bytes)" % (OUT, len(out_text)))
    # self-check: geometry entities must be unchanged, wiring must be AP214.
    g_before = re.findall(r"\n(#[0-9]+=(?:CARTESIAN_POINT|DIRECTION|"
                          r"FACE_SURFACE|CLOSED_SHELL|PLANE)[^\n]*)", "\n" + text)
    g_after = re.findall(r"\n(#[0-9]+=(?:CARTESIAN_POINT|DIRECTION|"
                         r"FACE_SURFACE|CLOSED_SHELL|PLANE)[^\n]*)", "\n" + out_text)
    if g_before == g_after:
        print("self-check: geometry entities unchanged (wiring-only transform)")
    else:
        sys.exit("self-check FAILED: geometry changed (before=%d after=%d)"
                 % (len(g_before), len(g_after)))
    if "MANIFOLD_SURFACE_SHAPE_REPRESENTATION" not in out_text \
            or "SHAPE_DEFINITION_REPRESENTATION" not in out_text:
        sys.exit("self-check FAILED: AP214 wiring not present")
    return 0


if __name__ == "__main__":
    sys.exit(main())
