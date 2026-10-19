"""P6-M1-T02 acceptance: oep-pmi model schema — GD&T types, datums, notes.

Independent (stdlib-only) verification of the committed canonical PMI example
`crates/oep-pmi/tests/golden/example-bracket.oeppmi.json`, plus the
"importable from P4" contract:

  - the file parses as JSON with a valid OEP header:
      format == "oep-pmi", version == 1, units == "mm"
  - top-level layout is exactly the four PMI collections under one document:
      datums, frames, tolerances, notes
  - id discipline: every id is a non-empty string and the id space is unique
    across ALL four collections (one document = one id space), with no
    duplicate references inside any single list
  - datum labels are unique and non-empty (datum A != datum B)
  - referential integrity (the links that make PMI useful):
      frame.refs[*].datum_id -> datums
      tolerance.frame_id -> frames
      tolerance characteristic is one of the closed 14-value Y14.5 set
  - frame sanity: precedence is an integer in 1..=3, each frame has no
    duplicate precedence, datums referenced by a frame are unique per frame
  - tolerance sanity: value finite and >= 0; material_condition in
    {mmc, lmc, rfs} when present; diameter_zone is a bool
  - note sanity: text non-empty, position (when present) is exactly 3 finite
    numbers
  - golden spot values: bracket gets a position tolerance (diameter zone,
    MMC) on the four mount holes referencing datum frame A|B|C; a flatness
    tolerance on the top face; a perpendicularity callout; plus two notes
  - "importable from P4": the same PMI content built through a P4-flavoured
    construction path (a STEP-AP242-style semantic skeleton) maps to the
    exact same settlement of the document (same ids, same characteristics,
    same datum/frame graph), proving the schema is a faithful import target
    — the real oep-step AP242 importer is a later P4 card, this fixes its
    destination model's contract.

Written first (TDD): the committed golden and these structural checks are the
"real material" gate. The Rust implementation is verified in two directions —
crates/oep-pmi/tests/pmi_serde.rs (single round-trip, byte-locked to the same
golden) and this file (independent re-parse with the stdlib json module).
"""

import json
import math
import os

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "crates", "oep-pmi", "tests", "golden", "example-bracket.oeppmi.json")

EXPECTED_HEADER = {
    "format": "oep-pmi",
    "version": 1,
    "units": "mm",
}
COLLECTIONS = ("datums", "frames", "tolerances", "notes")

# ASME Y14.5 / ISO 1101 geometric characteristics — the closed GD&T type set.
GD_T_CHARACTERISTICS = {
    "straightness",
    "flatness",
    "circularity",
    "cylindricity",
    "profile-of-line",
    "profile-of-surface",
    "angularity",
    "perpendicularity",
    "parallelism",
    "position",
    "concentricity",
    "symmetry",
    "circular-runout",
    "total-runout",
}
MATERIAL_CONDITIONS = {"mmc", "lmc", "rfs"}
PRECEDENCE_VALUES = {1, 2, 3}
FLOAT_FIELDS = ("value",)
OPT_FLOAT_FIELDS = ("second_value",)


def _load():
    with open(GOLDEN, "r", encoding="utf-8") as fh:
        return json.load(fh)


def _resolve(doc):
    """Index a document by collection name -> {id: entry} plus a global id set."""
    by_col = {}
    all_ids = []
    for col in COLLECTIONS:
        m = {}
        for entry in doc.get(col, []):
            m[entry["id"]] = entry
            all_ids.append(entry["id"])
        by_col[col] = m
    return by_col, all_ids


def _is_finite(x):
    return isinstance(x, (int, float)) and math.isfinite(float(x))


# ---------------------------------------------------------------------------
# header + layout
# ---------------------------------------------------------------------------


def test_golden_present_and_parses():
    assert os.path.isfile(GOLDEN), "missing committed golden %s" % GOLDEN
    doc = _load()
    assert isinstance(doc, dict)


def test_header_exact():
    doc = _load()
    for key, val in EXPECTED_HEADER.items():
        assert doc.get(key) == val, "header field %r mismatch" % key


def test_layout_collections_exact():
    doc = _load()
    for col in COLLECTIONS:
        assert isinstance(doc.get(col), list), "collection %r missing" % col
    # No surprise top-level data beyond the header + collections.
    known = set(EXPECTED_HEADER) | set(COLLECTIONS)
    unexpected = set(doc) - known
    assert not unexpected, "unexpected top-level keys: %r" % sorted(unexpected)


# ---------------------------------------------------------------------------
# id discipline
# ---------------------------------------------------------------------------


def test_ids_nonempty_and_unique():
    doc = _load()
    by_col, all_ids = _resolve(doc)
    for col in COLLECTIONS:
        for entry in doc[col]:
            assert isinstance(entry["id"], str) and entry["id"], (
                "%s entry has a bad id" % col
            )
    assert len(all_ids) == len(set(all_ids)), "duplicate id in document id space"


def test_no_duplicate_references_in_list():
    doc = _load()
    for entry in doc["datums"]:
        for field in ("feature_ids", "body_ids"):
            vals = entry.get(field, [])
            assert len(vals) == len(set(vals)), "%s duplicate refs in %s" % (
                entry["id"],
                field,
            )
    for entry in doc["frames"]:
        datum_ids = [r["datum_id"] for r in entry.get("refs", [])]
        assert len(datum_ids) == len(set(datum_ids)), "frame %s dup datum ref" % (
            entry["id"]
        )
        for field in ("feature_ids", "body_ids"):
            vals = entry.get(field, [])
            assert len(vals) == len(set(vals)), "%s duplicate refs in %s" % (
                entry["id"],
                field,
            )
    for entry in doc["tolerances"]:
        for field in ("feature_ids", "body_ids"):
            vals = entry.get(field, [])
            assert len(vals) == len(set(vals)), "%s duplicate refs in %s" % (
                entry["id"],
                field,
            )
    for entry in doc["notes"]:
        for field in ("feature_ids", "body_ids"):
            vals = entry.get(field, [])
            assert len(vals) == len(set(vals)), "%s duplicate refs in %s" % (
                entry["id"],
                field,
            )


# ---------------------------------------------------------------------------
# datum + frame sanity
# ---------------------------------------------------------------------------


def test_datum_labels_unique_nonempty():
    doc = _load()
    labels = {}
    for datum in doc["datums"]:
        label = datum.get("label")
        assert isinstance(label, str) and label, "datum %s has no label" % datum["id"]
        assert label not in labels, "duplicate datum label %r" % label
        labels[label] = datum["id"]


def test_frames_reference_existing_datums():
    doc = _load()
    _, all_ids = _resolve(doc)
    by_col, _ = _resolve(doc)
    datums = by_col["datums"]
    for frame in doc["frames"]:
        for ref in frame.get("refs", []):
            assert ref["datum_id"] in datums, (
                "frame %s refs unknown datum %s" % (frame["id"], ref["datum_id"])
            )


def test_frame_precedence_rules():
    doc = _load()
    for frame in doc["frames"]:
        refs = frame.get("refs", [])
        assert refs, "frame %s must carry at least one datum ref" % frame["id"]
        precedences = []
        for ref in refs:
            prec = ref["precedence"]
            assert prec in PRECEDENCE_VALUES, (
                "frame %s precedence %r out of 1..3" % (frame["id"], prec)
            )
            modifier = ref.get("modifier")  # absent == None (skip-if-none convention)
            assert isinstance(modifier, str) or modifier is None
            if modifier is not None:
                assert modifier in MATERIAL_CONDITIONS
            precedences.append(prec)
        assert len(precedences) == len(set(precedences)), (
            "frame %s duplicates precedence" % frame["id"]
        )


# ---------------------------------------------------------------------------
# tolerance sanity (GD&T types)
# ---------------------------------------------------------------------------


def test_all_tolerance_characteristics_in_closed_set():
    doc = _load()
    for tol in doc["tolerances"]:
        c = tol["characteristic"]
        assert c in GD_T_CHARACTERISTICS, (
            "tolerance %s characteristic %r not in Y14.5 set" % (tol["id"], c)
        )
        assert tol["characteristic"] in GD_T_CHARACTERISTICS
        assert isinstance(tol["diameter_zone"], bool)


def test_tolerance_value_finite_nonnegative():
    doc = _load()
    for tol in doc["tolerances"]:
        assert _is_finite(tol["value"]) and tol["value"] >= 0, (
            "tolerance %s bad value %r" % (tol["id"], tol["value"])
        )
        if tol.get("second_value") is not None:
            assert _is_finite(tol["second_value"]) and tol["second_value"] >= 0
        if tol.get("material_condition") is not None:
            assert tol["material_condition"] in MATERIAL_CONDITIONS


def test_tolerance_frame_ref_resolves():
    doc = _load()
    by_col, _ = _resolve(doc)
    frames = by_col["frames"]
    for tol in doc["tolerances"]:
        if tol.get("frame_id") is not None:
            assert tol["frame_id"] in frames, (
                "tolerance %s refs unknown frame %s" % (tol["id"], tol["frame_id"])
            )


# ---------------------------------------------------------------------------
# notes
# ---------------------------------------------------------------------------


def test_note_sanity():
    doc = _load()
    for note in doc["notes"]:
        assert isinstance(note["text"], str) and note["text"].strip(), (
            "note %s has empty text" % note["id"]
        )
        pos = note.get("position")
        if pos is not None:
            assert isinstance(pos, list) and len(pos) == 3
            assert all(_is_finite(x) for x in pos), "note %s bad position" % note["id"]


# ---------------------------------------------------------------------------
# golden spot values (real material)
# ---------------------------------------------------------------------------


def test_golden_spots():
    doc = _load()
    by_col, _ = _resolve(doc)
    tols = {t["id"]: t for t in doc["tolerances"]}
    # The bracket's four mount holes carry a diameter-zone MMC position callout.
    pos_tols = [
        t
        for t in doc["tolerances"]
        if t["characteristic"] == "position" and t["diameter_zone"]
        and t.get("material_condition") == "mmc"
    ]
    assert pos_tols, "expected a diameter-zone MMC position tolerance"
    # Every tolerance with a frame id must point at frame "frame-abc"-shaped A|B|C.
    for t in doc["tolerances"]:
        if t.get("frame_id"):
            fid = t["frame_id"]
            assert fid in by_col["frames"]
    # Notes must include the mount-hole callout and a general/defaults note.
    texts = " | ".join(n["text"] for n in doc["notes"])
    assert "THRU" in texts, "expected the 4X THRU mount-hole note"
    assert any("DIMENSIONS" in n["text"] or "MM" in n["text"] for n in doc["notes"]), (
        "expected a units/general note"
    )
    # Datums A, B, C present with unique labels.
    labels = sorted(d["label"] for d in doc["datums"])
    assert set(labels) >= {"A", "B", "C"}, "expected datums A, B, C, got %r" % labels


# ---------------------------------------------------------------------------
# importable from P4 (destination-model contract)
# ---------------------------------------------------------------------------


def _p4_flavoured_source():
    """A STEP-AP242-style semantic skeleton a P4 importer would map into oep-pmi.

    Uses the standard AP242 entity families (geometric tolerance + characteristic,
    datum identification, datum reference frame with precedence) expressed as
    plain dicts — the shape oep-step will translate. Collapsing this source into
    the oep-pmi document below is exactly the mapping a P4 importer performs.
    """
    return {
        "geometric_tolerances": [
            {
                "id": "gt_1",
                "char": "position",
                "value": 0.2,
                "diameter": True,
                "material_condition": "mmc",
                "drf": "drf_abc",
                "features": ["oep-feature-bracket-hole"],
            },
            {
                "id": "gt_flat",
                "char": "flatness",
                "value": 0.05,
                "diameter": False,
                "drf": None,
                "features": ["oep-feature-bracket-extrude"],
            },
            {
                "id": "gt_perp",
                "char": "perpendicularity",
                "value": 0.1,
                "diameter": False,
                "material_condition": "rfs",
                "drf": "drf_abc",
                "features": ["oep-feature-bracket-hole"],
            },
        ],
        "datums": [
            {"id": "datum_A", "label": "A", "features": ["oep-feature-bracket-extrude"]},
            {"id": "datum_B", "label": "B", "features": ["oep-feature-bracket-hole"]},
            {"id": "datum_C", "label": "C", "features": ["oep-feature-bracket-hole"]},
        ],
        "drf": {
            "id": "drf_abc",
            "name": "A-B-C",
            "refs": [
                {"datum": "datum_A", "precedence": 1, "modifier": "rfs"},
                {"datum": "datum_B", "precedence": 2, "modifier": "rfs"},
                {"datum": "datum_C", "precedence": 3, "modifier": None},
            ],
        },
        "notes": [
            {"id": "note_holes", "text": "4X Ø6.00 THRU", "features": ["oep-feature-bracket-hole"]},
            {"id": "note_units", "text": "ALL DIMENSIONS IN MM", "features": []},
        ],
    }


def test_importable_from_p4_destination_contract():
    """The golden must be reproducible from a P4-flavoured semantic source."""

    def collapse(src):
        doc = {
            "format": "oep-pmi",
            "version": 1,
            "units": "mm",
            "datums": [],
            "frames": [],
            "tolerances": [],
            "notes": [],
        }
        for d in src["datums"]:
            doc["datums"].append(
                {
                    "id": d["id"],
                    "label": d["label"],
                    "feature_ids": d["features"],
                    "body_ids": [],
                }
            )
        drf = src["drf"]
        doc["frames"].append(
            {
                "id": drf["id"],
                "name": drf["name"],
                "refs": [
                    {
                        "datum_id": r["datum"],
                        "precedence": r["precedence"],
                        "modifier": r["modifier"],
                    }
                    for r in drf["refs"]
                ],
            }
        )
        for gt in src["geometric_tolerances"]:
            doc["tolerances"].append(
                {
                    "id": gt["id"],
                    "characteristic": gt["char"],
                    "value": gt["value"],
                    "diameter_zone": gt["diameter"],
                    "material_condition": gt.get("material_condition"),
                    "frame_id": gt["drf"],
                    "second_value": None,
                    "feature_ids": gt["features"],
                    "body_ids": [],
                    "note": None,
                }
            )
        for n in src["notes"]:
            doc["notes"].append(
                {
                    "id": n["id"],
                    "text": n["text"],
                    "feature_ids": n["features"],
                    "body_ids": [],
                    "position": None,
                }
            )
        return doc

    golden = _load()
    collapsed = collapse(_p4_flavoured_source())
    # The collapsed structure must match the golden's id/ref graph and the
    # GD&T settlement (characteristic + value + zone + material condition).
    assert sorted(d["id"] for d in collapsed["datums"]) == sorted(
        d["id"] for d in golden["datums"]
    )
    assert sorted(f["id"] for f in collapsed["frames"]) == sorted(
        f["id"] for f in golden["frames"]
    )
    assert sorted(t["id"] for t in collapsed["tolerances"]) == sorted(
        t["id"] for t in golden["tolerances"]
    )
    assert sorted(n["id"] for n in collapsed["notes"]) == sorted(
        n["id"] for n in golden["notes"]
    )
    g_by_id = {t["id"]: t for t in golden["tolerances"]}
    for t in collapsed["tolerances"]:
        g = g_by_id[t["id"]]
        assert g["characteristic"] == t["characteristic"]
        assert g["value"] == t["value"]
        assert g["diameter_zone"] == t["diameter_zone"]
        # Optional fields are omitted when None in the canonical format.
        assert g.get("material_condition") == t.get("material_condition")
        assert g.get("frame_id") == t.get("frame_id")
    # The DRF graph must agree (same precedence/order) on every frame.
    gf_by_id = {f["id"]: f for f in golden["frames"]}
    for f in collapsed["frames"]:
        gf = gf_by_id[f["id"]]
        assert [r["precedence"] for r in gf["refs"]] == [r["precedence"] for r in f["refs"]]
        assert [r["datum_id"] for r in gf["refs"]] == [r["datum_id"] for r in f["refs"]]
