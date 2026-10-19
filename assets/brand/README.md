# Isolith identity kit (P13-T08)

The public product brand for Open Engineering Platform (OEP). This folder is the
single source of truth for the brand: name, mark, palette, typography, usage
rules, and the trademark collision evidence.

Chosen on 2026-10-18 because the internal codename Keelworks collides with a
registered KEELWORKS mark (decision record 0001) and no public-facing name could
be left to release pressure. The brand passes the same collision check that
rejected Keelworks.

## Contents

- NAME.md - the brand, the naming map (Isolith / OEP / Keelworks), decision record.
- USAGE.md - binding usage rules (clearspace, color rules, names, do/don't).
- logo/ - the mark (SVG sources): isolith-mark.svg (primary), -dark, -mono,
  isolith-lockup.svg (mark + wordmark), favicon.svg. Plus raster exports in
  logo/png/ for convenience (never edit; regenerate from SVG).
- design/DESIGN.md - DESIGN.md token spec (Google design.md format):
  palette, typography, spacing, components. WCAG AA-checked.
- design/tokens.json - machine tokens for the docs site (colors, fonts, type,
  space, radius, logo paths).
- design/validate_design.py - deterministic DESIGN.md validator (structure +
  WCAG AA). Run: python3 design/validate_design.py design/DESIGN.md
- evidence/trademark-evidence-2026-10-18.json - live registry captures.
- evidence/trademark-collision-check-note.md - the check note.
- The canonical acceptance test lives at tests/test_brand_trademark_acceptance.py
  (program convention; it asserts the kit + evidence consistency).

## Docs site (P13-T03) consumption contract

The docs site must use Isolith, not Keelworks:

1. Site title and header: "Isolith" with the lockup (logo/isolith-lockup.svg),
   and the expanded name "Isolith: Open Engineering Platform (OEP)" on the
   home page. Page background paper #F6F4EF, text ink #0F2A43.
2. Colors and type: import design/tokens.json (or DESIGN.md) - do not hand-pick
   hex values.
3. Favicon: logo/favicon.svg (ink tile, mark on it).
4. Dark surfaces: logo/isolith-mark-dark.svg. Single color: -mono.svg.
5. Never print "Keelworks" on any public page. Never "CATIA" in naming.
6. Honest format matrix stays the GA matrices; the brand does not change claims.

## Verifying the kit

- Trademark acceptance: `pytest tests/test_brand_trademark_acceptance.py`
  (or run it directly) - must pass.
- DESIGN.md: `python3 assets/brand/design/validate_design.py assets/brand/design/DESIGN.md`
  - must exit 0.
- Mark/lockup SVGs are committed sources; PNGs are exports only.

Any change of brand restarts the collision check (evidence file) before use.
