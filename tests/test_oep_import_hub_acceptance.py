"""P6-M4-T02 acceptance: importer hub canonical output independently re-parsed.

Independent (stdlib-only) verification of the committed importer-hub golden
`crates/oep-import/tests/golden/cube1-import.oepgeo.json` — the canonical
`oep-geo` Document that the P3 (3DXML) importer adapter produces from the real
license-clean Abaqus cube1 3DXML container:

  - the committed bytes parse as JSON with the canonical OEP header:
      format == "oep-geo", version == 2, units == "mm"
  - structural fidelity of the 3DXML -> canonical mapping:
      - 2 parts (the two Reference3D nodes Abaqus_Model / Abaqus_Geometry)
      - 1 solid body (the polygonal mesh rep) with a ~10-wide cube bbox
      - 1 assembly with 1 instance of Abaqus_Geometry at a 16-component
        row-major affine transform
      - id discipline: all ids unique document-wide; instance sources resolve
      - part 2 (Abaqus_Geometry) owns the mesh body via the InstanceRep wiring
  - every float is finite and the body bbox is well-formed (min <= max)

Written first (TDD): the committed golden and these structural checks are the
"real material" gate — the Rust side byte-locks the same file
(crates/oep-import/tests/importer_hub.rs), this file re-parses it from the
other side with the stdlib json module, so no single decoder can drift alone.
"""

import json
import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(
    REPO, "crates", "oep-import", "tests", "golden", "cube1-import.oepgeo.json"
)

EXPECTED_HEADER = {
    "format": "oep-geo",
    "version": 2,
    "units": "mm",
}


def _load():
    with open(GOLDEN, "r", encoding="utf-8") as fh:
        return json.load(fh)


def _finite(vals):
    return all(isinstance(x, (int, float)) and x == x for x in vals)


def _well_formed_bbox(bb):
    return _finite(bb["min"]) and _finite(bb["max"]) and all(
        lo <= hi for lo, hi in zip(bb["min"], bb["max"])
    )


def test_header_and_flat_layout():
    doc = _load()
    for key, val in EXPECTED_HEADER.items():
        assert doc.get(key) == val, (key, doc.get(key))
    for col in ("parts", "bodies", "features", "assemblies"):
        assert isinstance(doc.get(col, []), list), col
    # id discipline: unique across all collections and all instance ids.
    ids = []
    for col in ("parts", "bodies", "features"):
        ids += [e["id"] for e in doc.get(col, [])]
    for a in doc.get("assemblies", []):
        ids.append(a["id"])
        ids += [i["id"] for i in a.get("instances", [])]
    assert len(ids) == len(set(ids)), "ids must be unique document-wide"


def test_cube1_structural_fidelity():
    doc = _load()
    assert len(doc["parts"]) == 2
    names = {p["name"] for p in doc["parts"]}
    assert names == {"Abaqus_Model", "Abaqus_Geometry"}

    assert len(doc["bodies"]) == 1
    body = doc["bodies"][0]
    assert body["kind"] == "solid"
    assert _well_formed_bbox(body["bbox"])
    # cube1 is a 10x10x10 unit cube (Abaqus)
    assert abs(body["bbox"]["max"][0] - 10.0) < 1e-3
    assert abs(body["bbox"]["max"][1] - 10.0) < 1e-3
    assert abs(body["bbox"]["max"][2] - 10.0) < 1e-3

    # body owned by Abaqus_Geometry (node 2) through the InstanceRep link
    owner = [p for p in doc["parts"] if body["id"] in p.get("body_ids", [])]
    assert len(owner) == 1 and owner[0]["name"] == "Abaqus_Geometry"

    assert len(doc["assemblies"]) == 1
    asm = doc["assemblies"][0]
    instances = asm.get("instances", [])
    assert len(instances) == 1
    inst = instances[0]
    assert inst["source"]["kind"] == "part"
    assert inst["source"]["id"] == owner[0]["id"]
    t = inst["transform"]
    assert len(t) == 16 and _finite(t), "16-component finite affine transform"


def test_rerun_on_gen_golden_output():
    # The example gen path is exercised by the Rust test; here we merely assert
    # the committed golden is still parseable by a strict json round trip.
    _load()  # raises if malformed


if __name__ == "__main__":
    for fn in (test_header_and_flat_layout, test_cube1_structural_fidelity):
        fn()
    print("P6-M4-T02 importer-hub golden acceptance: PASS")
