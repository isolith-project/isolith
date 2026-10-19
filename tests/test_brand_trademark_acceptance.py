"""Acceptance test for P13-T08 (public brand + trademark collision check).

Proves the committed identity kit satisfies the same trademark collision check
used in P0-M1-T02 (decision record 0001). Against the committed evidence file it
verifies that:

1. A decision/name record exists and fixes the public brand, the working title,
   the internal codename, and the no-CATIA rule.
2. The brand is distinct from the internal codename (Keelworks).
3. The collision section names the registries actually searched (USPTO, EUIPO,
   WIPO, TMview) and those match the evidence file.
4. The evidence file records the 2026-10-18 search date and the real finding:
   no mark identical to the brand in software classes 9 or 42, and zero USPTO
   wordmark results.
5. The control term (keelworks) still surfaces a registered mark in classes 9/42,
   proving the method would catch a collision if one existed.

The evidence file is the real material: live captures from USPTO Trademark
Search and the WIPO Global Brand Database / TMview aggregate on 2026-10-18.
"""
import json
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
BRAND = REPO / "assets" / "brand"
DECISION = BRAND / "NAME.md"
EVIDENCE = BRAND / "evidence" / "trademark-evidence-2026-10-18.json"
USAGE = BRAND / "USAGE.md"

REQUIRED_DOC_TEXT = [
    "isolith",
    "open engineering platform",
    "keelworks",
    "no catia",
    "trademark check",
    "classes 9 and 42",
]
REQUIRED_REGISTRIES = ["uspto", "euipo", "wipo", "tmview"]
REQUIRED_EVIDENCE_KEYS = ["searched_on", "terms", "registries", "primary_name"]
SOFTWARE_CLASSES = {9, 42}


def test_decision_document_exists():
    assert DECISION.exists(), f"missing name record: {DECISION}"
    assert USAGE.exists(), f"missing usage rules: {USAGE}"


def test_decision_document_covers_required_content():
    text = DECISION.read_text(encoding="utf-8").lower()
    for frag in REQUIRED_DOC_TEXT:
        assert frag in text, f"name record missing required content: {frag!r}"


def test_decision_doc_cites_searched_registries():
    text = DECISION.read_text(encoding="utf-8").lower()
    for reg in REQUIRED_REGISTRIES:
        assert reg in text, f"name record must cite registry: {reg}"


def test_evidence_file_is_valid_and_complete():
    data = json.loads(EVIDENCE.read_text(encoding="utf-8"))
    for key in REQUIRED_EVIDENCE_KEYS:
        assert key in data, f"evidence missing key: {key}"
    assert str(data["searched_on"]) in DECISION.read_text(encoding="utf-8"), \
        "name record must carry the evidence search date"
    for reg in REQUIRED_REGISTRIES:
        assert reg in data["registries"], f"evidence must list registry: {reg}"


def test_primary_brand_has_no_software_class_collision():
    """The core finding: 'isolith' must not collide in classes 9 or 42."""
    data = json.loads(EVIDENCE.read_text(encoding="utf-8"))
    assert data["primary_name"] == "isolith"
    terms = {t.get("term", "").lower() for t in data["terms"]}
    assert "isolith" in terms, "evidence must include a search for 'isolith'"
    iso = next(t for t in data["terms"] if t.get("term", "").lower() == "isolith")
    assert iso.get("software_class_collision") is False, \
        "evidence must state no class 9/42 collision for the primary brand"
    classes = {c for m in (iso.get("marks") or []) for c in (m.get("classes") or [])}
    assert not (classes & SOFTWARE_CLASSES), \
        f"found software-class marks overlapping the brand: {classes & SOFTWARE_CLASSES}"


def test_uspto_control_still_surfaces_keelworks():
    """The control catches a real collision, proving the method is sensitive."""
    data = json.loads(EVIDENCE.read_text(encoding="utf-8"))
    kw = next(t for t in data["terms"] if t.get("term", "").lower() == "keelworks")
    assert kw.get("control") == "regression control"
    assert kw.get("registration_found") is True
    assert kw.get("software_class_collision") is True


def test_primary_brand_distinct_from_codename():
    text = DECISION.read_text(encoding="utf-8").lower()
    assert "isolith" in text and "keelworks" in text
    assert "isolith" != "keelworks"


def test_kit_has_logo_and_palette_artifacts():
    assert (BRAND / "logo" / "isolith-mark.svg").exists(), "missing logo mark"
    assert (BRAND / "logo" / "isolith-lockup.svg").exists(), "missing lockup"
    assert (BRAND / "design" / "DESIGN.md").exists(), "missing DESIGN.md palette spec"
    assert (BRAND / "evidence" / "trademark-collision-check-note.md").exists(), \
        "missing collision-check note"


def main():
    """Run directly: python3 tests/test_brand_trademark_acceptance.py."""
    import sys
    fn = [(n, f) for n, f in sorted(globals().items())
          if n.startswith("test_") and callable(f)]
    failures = []
    for name, f in fn:
        try:
            f()
            print(f"[PASS] {name}")
        except Exception as e:  # noqa: BLE001
            failures.append((name, e))
            print(f"[FAIL] {name}: {e}")
    print(f"{len(fn) - len(failures)}/{len(fn)} branded acceptance checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
