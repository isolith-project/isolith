# Isolith identity usage rules (v1.1, 2026-10-20)

These are the binding usage rules for the Isolith brand. The docs site, release
artifacts, and any other public-facing surface must follow them. When in doubt,
use less decoration, more space, and the primary mark.

## Names and wording

- Say "Isolith" for the product. Write it lowercase as a wordmark ("isolith");
  at the start of a sentence or in headers it may be capitalized ("Isolith").
- First mention on any surface pairs it with the expanded name:
  "Isolith, the open engineering platform" or "Isolith: Open Engineering
  Platform (OEP)".
- Use "Isolith" everywhere, including on every public surface. Never use
  "Keelworks": it is the retired internal codename and collides with a
  registered mark.
- Never use "CATIA" in public naming (program rule, decision record 0001).
- Never claim compatibility with a vendor product you have not tested and
  documented. "Reads files produced by X" only where the GA matrix proves it.

## The mark

- Primary mark: assets/brand/logo/isolith-mark.svg (three-color, on light
  backgrounds). Use it at 24 px minimum width for the mark alone, 120 px
  minimum for the horizontal lockup.
- Dark surfaces: use isolith-mark-dark.svg (paper top face). Mono/ink surfaces
  (stamps, favicons, single-color print): use isolith-mark-mono.svg.
- Clearspace: keep a margin around the mark of at least half the top-face width
  (clearspace X = 0.5 x mark height) on every side. Do not crop, rotate, or
  re-color the mark.
- Do not stretch, add gradients, add drop shadows, or place the mark on a busy
  photograph. The mark sits on paper, ink, or one of the brand surfaces.
- Do not rebuild the geometry. Use the committed SVG files only.

## Color

Tokens and WCAG-checked contrast live in design/DESIGN.md and design/tokens.json.

- Paper #F6F4EF is the default page background (warm blueprint paper).
- Ink #0F2A43 is the default text color and the mono mark color.
- Plate #1D5B9E is the primary interactive and brand accent blue. White text on
  plate meets WCAG AA for normal text.
- Steel #7C94AD is for rules, dividers, and the open face of the mark on light
  surfaces.
- Accent copper #C8792B is a low-volume highlight: one element per view at most.
  Never place text in copper.
- Dark theme: use ink as the page background (#0F2A43), paper text
  (#F6F4EF), plate for interactive elements; the light mark on dark uses
  isolith-mark-dark.svg.

## Typography

- Display and product wordmark: Space Grotesk (SIL Open Font License),
  weights 500/600/700.
- Body: Inter (SIL OFL) or IBM Plex Sans (SIL OFL), weight 400/500.
- Code/data: IBM Plex Mono or JetBrains Mono (SIL OFL).
- Both fonts are freely redistributable and self-hostable, matching the product
  (no external font CDN required for an offline/air-gapped install).

## Do

- Do use the lockup (mark + wordmark) for site headers and product UI.
- Do pair Isolith with the expanded name on the docs site home page.
- Do keep the format capability matrix honest: say exactly what each reader
  opens and at what fidelity, nothing more.
- Do keep one accent element per view.

## Do not

- Do not use another name, another mark, or a close visual variant.
- Do not suggest the product is an official or endorsed replacement for any
  vendor product (no vendor attacks, no "drop-in replacement" claims without
  the evidence to back it).
- Do not put the product name or mark on an export-controlled server
  (ITAR/EAR) claim without the export-control record (P11-T05) alongside it.
- Do not ship the logo in a heavier format than the SVG source. Raster exports
  are generated for convenience only and must not be edited.

## File inventory (assets/brand/)

- NAME.md - brand, naming map, decision record.
- USAGE.md - these rules.
- README.md - kit index and docs-site consumption contract.
- logo/isolith-mark.svg, isolith-mark-dark.svg, isolith-mark-mono.svg,
  isolith-lockup.svg, favicon.svg
- design/DESIGN.md - token spec (palette, typography, components), WCAG-checked.
- design/tokens.json - machine tokens for the docs site.
- evidence/trademark-evidence-2026-10-18.json - registry captures.
- evidence/trademark-collision-check-note.md - the check note.
- evidence/test_brand_trademark_acceptance.py - acceptance test (passes).

## Versioning

This is a living kit. Bump Version on every change in NAME.md, and record the
change in the commit subject (e.g. "docs(brand): ...").
