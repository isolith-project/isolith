"""P7-M4-T01 acceptance: sketcher + parametric variable sets, chosen shell.

Independent (stdlib-only) verification of the committed sketcher golden
`crates/oep-sketch/tests/golden/bracket-sketch-drive.oepgeo.json`  -  the
canonical `oep-geo` Document the oep-sketch crate produces when a bracket
profile sketch (constrained 2D rectangle) is driven by variable sets
`width=80` / `height=40` and the solved width drives the model body:

  - the committed bytes parse as JSON with the canonical OEP header
      format == "oep-geo", version == 2, units == "mm"
  - the part's feature history carries the sketch semantics as oep-geo
      FEATURES (the chosen shell contract): one feature kind "Sketch"
      (the profile constraints, including the two Length dimensions that
      REFERENCE variable names "width" / "height", never a literal), and one
      feature kind "VariableSet" (the FreeCAD-v1-variable-set semantics target)
  - the solid body the sketch drives has a width equal to the solved sketch
      dimension (80 mm within solver tolerance)  -  "a sketch parametrically
      drives a model dimension"
  - id discipline: unique document-wide ids, features resolve, bbox well-formed
  - determinism anchor: the golden itself is byte-locked on the Rust side; this
     file re-parses it with the stdlib json module so no single decoder drifts

Written first (TDD): the committed golden and these structural checks are the
"real material" gate. The Rust side byte-locks the same file
(crates/oep-sketch/tests/p7m4_sketch_acceptance.rs) and independently drives
the parametrized solve; this file re-parses from the other side.
"""

import json
import os

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(
    REPO, "crates", "oep-sketch", "tests", "golden", "bracket-sketch-drive.oepgeo.json"
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


def test_golden_present_and_parses():
    assert os.path.isfile(GOLDEN), "missing %s" % GOLDEN
    doc = _load()
    assert isinstance(doc, dict)


def test_header_exact():
    doc = _load()
    for key, val in EXPECTED_HEADER.items():
        assert doc.get(key) == val, "header %r != %r" % (key, val)


def _features_by_kind(doc):
    out = {}
    for f in doc.get("features", []):
        out.setdefault(f.get("kind"), []).append(f)
    return out


def test_sketch_and_varset_are_oep_geo_features():
    """Chosen-shell contract: the sketch + variable sets are stored as oep-geo
    FEATURES (kinds Sketch / VariableSet), matching FreeCAD v1 variable-set
    semantics without importing FreeCAD."""
    doc = _load()
    by = _features_by_kind(doc)
    assert "Sketch" in by, "missing Sketch feature: %r" % list(doc.get("features"))
    assert "VariableSet" in by, "missing VariableSet feature"

    sketch = by["Sketch"][0]
    payload = sketch["parameters"]["sketch"]
    constraints = payload["constraints"]
    # The two width/height Length dimensions reference VARIABLE names by name.
    dims = [
        c for c in constraints if list(c) == ["Length"]
    ]
    assert len(dims) == 2, "expected two Length dimensions"
    refs = sorted(c["Length"]["expr"]["Variable"] for c in dims)
    assert refs == ["height", "width"], "dimensions must reference variables: %r" % refs
    # The Fixed anchor is present: the sketch is anchored.
    assert any(list(c) == ["Fixed"] for c in constraints), "expected a Fixed anchor"

    varset = by["VariableSet"][0]["parameters"]["variables"]
    assert varset.get("width") == 80.0, "varset width"
    assert varset.get("height") == 40.0, "varset height"


def test_sketch_drives_model_body_width():
    """Acceptance: a sketch parametrically drives a model dimension. The solid
    body the sketch drives carries width == solved sketch dimension (80 mm within
    tolerance)."""
    doc = _load()
    # one part, one driven solid body.
    assert len(doc.get("parts")) == 1
    bodies = doc.get("bodies")
    assert len(bodies) == 1
    body = bodies[0]
    assert body["kind"] == "solid"
    bb = body["bbox"]
    assert _finite(bb["min"]) and _finite(bb["max"])
    width = bb["max"][0] - bb["min"][0]
    assert abs(width - 80.0) < 1e-6, "model body width ~80, got %r" % width
    # The part owns the driven body and both features resolve.
    part = doc["parts"][0]
    assert body["id"] in part["body_ids"]
    for fid in part["feature_ids"]:
        assert any(f["id"] == fid for f in doc.get("features")), "dangling feature %s" % fid
    # bbox well-formed: min <= max on every axis.
    assert all(lo <= hi for lo, hi in zip(bb["min"], bb["max"]))


def test_id_discipline_unique():
    doc = _load()
    ids = []
    for col in ("parts", "bodies", "features", "assemblies"):
        for entry in doc.get(col, []):
            ids.append(entry["id"])
    assert len(ids) == len(set(ids)), "duplicate document-wide id"


def test_driven_dimension_name_matches_message():
    """Human-readable trace: the 'width' feature in the doc is what changed when
    the variable edits. This anchors the golden's docstring contract."""
    doc = _load()
    names = [f["name"] for f in doc.get("features", [])]
    assert "bracket-profile" in names
