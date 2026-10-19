#!/usr/bin/env python3
"""P5-M3-T02 — generate an external-converter REFERENCE mesh from a native file.

This is the committed PRODUCER of the test-only ground-truth reference files in
`corpus/reference/`. It drives FreeCAD (a fully independent, OCCT-kernel
converter) to read the SAME native sample our clean-room reader parses, and
writes the resulting mesh in the mesh-reference JSON schema the harness
(`oep_tooling.native_accuracy.read_reference`) consumes.

Clean-room line (P5-M1-T03 §6): the converter output is used STRICTLY as
test-only ground truth for our own testing. It is never a source of format
internals, never shipped, never claimed as the product. Running this tool is
NOT required for CI (the committed reference files carry their provenance and
the harness reads them offline); it documents how the reference was produced so
it is reproducible and auditable.

Usage:
  freecadcmd -c < corpus/tools/gen_freecad_reference.py <native.stl> <out.json>

Requires a FreeCAD console interpreter with the Mesh module (OCCT-backed).
The script reads *exactly one* mesh from the input and dumps positions +
triangles (deduplicated to 9 dp), tagged with the FreeCAD version + provenance.
"""
import json
import pathlib
import sys

import FreeCAD
import Mesh

if len(sys.argv) < 3:
    sys.exit("usage: gen_freecad_reference.py <native.stl> <out.json>")

src, dst = sys.argv[1], sys.argv[2]

mesh = Mesh.Mesh(src)
index = {}
positions = []
triangles = []
for face in mesh.Facets:
    ids = []
    for p in face.Points:
        key = (round(float(p[0]), 9), round(float(p[1]), 9),
               round(float(p[2]), 9))
        if key not in index:
            index[key] = len(positions)
            positions.append(list(key))
        ids.append(index[key])
    triangles.append(ids)

out = {
    "schema_version": 1,
    "format": "mesh-reference",
    "provenance": {
        "converter": "FreeCAD",
        "freecad_version": ".".join(map(str, FreeCAD.Version()[:2])),
        "freecad_executable": "freecadcmd (micromamba envs/freecad)",
        "input": pathlib.Path(src).name,   # basename; the repo-relative path is stable
        "method": "Mesh.read() independent OCCT-backed importer; "
                  "vertices normalized to 9 dp",
        "role": "external-converter reference, test-only ground truth "
                "(P5-M3-T02); never shipped, never claimed as product",
        "clean_room": "converter output used strictly as test reference; "
                      "not a source of format internals",
    },
    "mesh": {"positions": positions, "triangles": triangles},
}
pathlib.Path(dst).parent.mkdir(parents=True, exist_ok=True)
pathlib.Path(dst).write_text(json.dumps(out, indent=2), encoding="utf-8")
print("WROTE %s (points=%d triangles=%d)" % (dst, len(positions), len(triangles)))
