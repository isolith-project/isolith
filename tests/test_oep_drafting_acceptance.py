"""EDITOR-2D-DRAFTING acceptance: drafting workbench on the real kernel.

Independent (stdlib-only) verification of the committed drafting goldens
`crates/oep-drafting/tests/golden/bracket-drawing.{oepgeo.json,svg,pdf}` --
the canonical oep-geo v2 Document (part + solid bbox 80x40x6 + Sketch /
VariableSet / Extrude features + the Drawing record) and the rendered vector
drawing -- so no single decoder can drift:

  - the document parses with the canonical OEP header and the Class 2 drawing
    extension: format == "oep-geo", version == 2, one drawing/one sheet with
    three standard views and three dimensions bound to variable names by id;
  - the VariableSet feature carries the SAME parameters that drive the model
    (width=80, height=40, depth=6) and the dimensions reference those names
    (FreeCAD-v1-variable-set behavior), never literals;
  - the body boundary box agrees with the variables (80 x 40 x 6);
  - the SVG (parsed with the stdlib xml.etree) renders each standard view as
    a group whose projected edges form the correct rectangle for that
    projection (front 80x6, top 80x40, right 40x6), with the title block
    fields and the variable-driven dimension labels (80 / 40 / 6) drawn;
  - the PDF is a structurally valid PDF 1.4 with the same MediaBox.

Written first (TDD): these golden structural + geometric checks are the
"real material" gate; the Rust side byte-locks the same files
(crates/oep-drafting/tests/drawing_acceptance.rs) and independently drives
the parametrized solve.
"""

import json
import os
import re
import xml.etree.ElementTree as ET

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN_DIR = os.path.join(REPO, "crates", "oep-drafting", "tests", "golden")
DOC = os.path.join(GOLDEN_DIR, "bracket-drawing.oepgeo.json")
SVG = os.path.join(GOLDEN_DIR, "bracket-drawing.svg")
PDF = os.path.join(GOLDEN_DIR, "bracket-drawing.pdf")

EXPECTED_HEADER = {"format": "oep-geo", "version": 2, "units": "mm"}

TOL = 1e-3

# (view label, expected extent w, expected extent h) -- model units = paper mm
# here because every view renders at 1:1.
EXPECTED_VIEWS = {
    "FRONT": (80.0, 6.0),
    "TOP": (80.0, 40.0),
    "RIGHT": (40.0, 6.0),
}

EXPECTED_DIMS = {
    "oep-dim-length": ("top", "horizontal", "width", 80.0),
    "oep-dim-width": ("top", "vertical", "height", 40.0),
    "oep-dim-depth": ("front", "vertical", "depth", 6.0),
}

NS = {"svg": "http://www.w3.org/2000/svg"}


def _load():
    with open(DOC, encoding="utf-8") as fh:
        return json.load(fh)


# ---------------------------------------------------------------------------
# document golden: header + Class 2 drawing extension + variable wiring
# ---------------------------------------------------------------------------


def test_document_header_and_drawing_extension():
    doc = _load()
    for k, v in EXPECTED_HEADER.items():
        assert doc.get(k) == v, "header %r mismatch: %r" % (k, doc.get(k))
    assert isinstance(doc.get("drawings"), list) and len(doc["drawings"]) == 1
    drawing = doc["drawings"][0]
    assert drawing["id"] == "oep-drawing-oep-part-bracket"
    assert len(drawing["sheets"]) == 1
    sheet = drawing["sheets"][0]
    assert sheet["size"] == {"width_mm": 297, "height_mm": 210}  # A4 landscape
    assert sheet["projection"] == "third_angle"
    assert len(sheet["views"]) == 3
    assert len(sheet["dimensions"]) == 3
    # title block values
    tb = sheet["title_block"]
    assert tb["title"] == "Bracket"
    assert tb["part_number"] == "ISO-BKT-001"
    assert tb["material"] == "6061-T6"


def test_variable_set_drives_model_and_dimensions():
    doc = _load()
    # VariableSet feature
    varset = {}
    for f in doc["features"]:
        if f["kind"] == "VariableSet":
            varset = f["parameters"]["variables"]
    assert varset == {"width": 80.0, "height": 40.0, "depth": 6.0}, varset
    # Body bbox agrees with the variables (the model dimension)
    body = doc["bodies"][0]
    bmin, bmax = body["bbox"]["min"], body["bbox"]["max"]
    assert bmax == [80.0, 40.0, 6.0] and bmin == [0.0, 0.0, 0.0]
    # Dimensions reference variable names BY NAME (FreeCAD-v1), and the view
    # ids they annotate exist on the sheet.
    sheet = doc["drawings"][0]["sheets"][0]
    view_ids = {v["id"] for v in sheet["views"]}
    kinds = {}
    for v in sheet["views"]:
        kinds[v["id"]] = v["kind"]
    for dim in sheet["dimensions"]:
        assert dim["view_id"] in view_ids, dim
        vid, kind, var, val = EXPECTED_DIMS[dim["id"]]
        assert kinds[dim["view_id"]] == vid, dim
        assert dim["kind"] == kind, dim
        assert dim.get("variable") == var, "dimension must be variable-driven, got %r" % dim


def test_document_ids_and_references_resolve():
    doc = _load()
    all_ids = []
    for col in ("parts", "bodies", "features", "drawings"):
        for entry in doc.get(col, []):
            all_ids.append(entry["id"])
    sheet = doc["drawings"][0]["sheets"][0]
    for v in sheet["views"]:
        all_ids.append(v["id"])
        if v.get("part_id"):
            assert v["part_id"] in [p["id"] for p in doc["parts"]], v["part_id"]
    for d in sheet["dimensions"]:
        all_ids.append(d["id"])
    assert len(all_ids) == len(set(all_ids)), "id space must be unique"


# ---------------------------------------------------------------------------
# SVG golden: projected edges per view + title block + dimension labels
# ---------------------------------------------------------------------------


def _svg():
    return ET.parse(SVG).getroot()


def _view_edges(group):
    """Return the set of distinct 2D endpoint pairs in a view's <g>."""
    pts = set()
    for path in group.findall("svg:path", NS):
        d = path.get("d")
        m = re.match(r"M (-?[\d.]+) (-?[\d.]+) L (-?[\d.]+) (-?[\d.]+)", d)
        assert m, "unexpected path: %r" % d
        p1 = (float(m.group(1)), float(m.group(2)))
        p2 = (float(m.group(3)), float(m.group(4)))
        # canonicalize endpoints
        pts.add(tuple(sorted((p1, p2))))
    return pts


def _extent(pts):
    xs = [p[0] for e in pts for p in e]
    ys = [p[1] for e in pts for p in e]
    return (max(xs) - min(xs), max(ys) - min(ys))


def test_svg_renders_correct_projected_edges_per_view():
    root = _svg()
    assert root.get("width") == "297mm" and root.get("height") == "210mm"
    groups = {}
    for g in root.iter("{%s}g" % NS["svg"]):
        name = g.get("data-view")
        if name:
            groups[name] = g
    assert set(groups) == set(EXPECTED_VIEWS), groups.keys()
    for name, (ew, eh) in EXPECTED_VIEWS.items():
        edges = _view_edges(groups[name])
        assert len(edges) == 4, "%s must be a 4-edge rectangle, got %r" % (name, edges)
        w, h = _extent(edges)
        assert abs(w - ew) < TOL and abs(h - eh) < TOL, (
            "%s extent %sx%s != expected %sx%s" % (name, w, h, ew, eh)
        )


def test_svg_draws_title_block_and_variable_driven_dimensions():
    root = _svg()
    texts = [t.text for t in root.iter("{%s}text" % NS["svg"])]
    joined = "\n".join(x for x in texts if x)
    assert "TITLE: Bracket" in joined
    assert "PART NO: ISO-BKT-001" in joined
    assert "MATERIAL: 6061-T6" in joined
    # The three dimensions are labeled by the variable-driven values.
    for label in ("80", "40", "6"):
        assert label in joined, "dimension label %r missing" % label
    # View labels present.
    for name in ("FRONT", "TOP", "RIGHT"):
        assert name in joined


def test_svg_third_angle_layout_orders_views():
    root = _svg()
    centers = {}
    for g in root.iter("{%s}g" % NS["svg"]):
        name = g.get("data-view")
        if not name:
            continue
        pts = _view_edges(g)
        xs = [p[0] for e in pts for p in e]
        ys = [p[1] for e in pts for p in e]
        # SVG coordinates are the paper model's Y-up space (shared with PDF).
        centers[name] = ((max(xs) + min(xs)) / 2.0, (max(ys) + min(ys)) / 2.0)
    # Third-angle: TOP above FRONT (larger paper Y), RIGHT to the right (larger X).
    assert centers["TOP"][1] > centers["FRONT"][1], centers
    assert centers["RIGHT"][0] > centers["FRONT"][0], centers


# ---------------------------------------------------------------------------
# PDF golden: structurally valid vector document
# ---------------------------------------------------------------------------


def test_pdf_is_structural_valid_vector_document():
    data = open(PDF, "rb").read()
    assert data.startswith(b"%PDF-1.4\n")
    assert data.rstrip().endswith(b"%%EOF")
    assert b"MediaBox [0 0 297 210]" in data
    assert b"/Subtype /Type1 /BaseFont /Helvetica" in data
    assert b"BT /F1" in data and b"Tj ET" in data  # text operators
    # xref startxref integrity: the number after startxref points at the xref
    # keyword line.
    m = re.search(rb"startxref\n(\d+)\n%%EOF", data)
    assert m, "startxref/trailer missing"
    xref_at = int(m.group(1))
    assert data[xref_at : xref_at + 5] == b"xref\n"
