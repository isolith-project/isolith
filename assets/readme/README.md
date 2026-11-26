# README assets

Files here exist only to make the repository front door readable. They follow the
identity rules in `assets/brand/USAGE.md`; nothing in this folder is a new brand asset.

## Contents

| File | What it is | Source |
| --- | --- | --- |
| `banner.svg` | The README hero: the committed lockup on blueprint paper with the tagline. Editable source. | Composed by `make_banner.py`; the lockup is inlined verbatim from `assets/brand/logo/isolith-lockup.svg` (no geometry rebuilt). |
| `banner.png` | 2x raster of `banner.svg`, 2560x660. This is the file the README loads. | `google-chrome --headless`, `--window-size=2560,660`, device scale 1. |

## Rules applied

- Palette from `assets/brand/design/tokens.json`: paper `#F6F4EF` background, ink `#0F2A43`
  for the tagline, slate `#546981` for the small caps line, one copper `#C8792B` rule as the
  single accent element.
- Type: Space Grotesk for the tagline and wordmark, Inter for the small caps line. Both are
  SIL OFL, so the banner needs no licensed font to rebuild.
- No gradients, no shadows, no glow, no re-colored mark. Clearspace around the lockup runs
  wider than the 0.5 x mark height minimum.
- The blueprint grid is decorative paper texture at 13 percent steel; it is not part of the
  mark.

## Rebuilding

```sh
python3 make_banner.py
```

The script reads the lockup from git (`github/main:assets/brand/logo/isolith-lockup.svg`),
writes both files, and renders the PNG with headless Chrome. Space Grotesk and Inter must be
installed on the machine that renders, otherwise Chrome silently substitutes a fallback and
the wordmark stops matching the brand.

Screenshots and other media in the README are not duplicated here. They live in
`docs-site/assets/images/` so that every capture has one home and one provenance record,
`docs-site/assets/media-manifest.md`.
