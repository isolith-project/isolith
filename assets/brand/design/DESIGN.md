---
version: alpha
name: Isolith
description: Sovereign engineering platform. Blueprint ink, plate blue, drafting steel, copper accent on warm blueprint paper. Space Grotesk display, Inter body.
colors:
  primary: "{colors.plate}"
  ink: "#0F2A43"
  plate: "#1D5B9E"
  steel: "#7C94AD"
  slate: "#546981"
  accent: "#C8792B"
  paper: "#F6F4EF"
  white: "#FFFFFF"
typography:
  display-xl:
    fontFamily: "Space Grotesk"
    fontSize: 3rem
    fontWeight: 700
    lineHeight: 1.05
    letterSpacing: "-0.01em"
  display-lg:
    fontFamily: "Space Grotesk"
    fontSize: 2.5rem
    fontWeight: 600
    lineHeight: 1.1
    letterSpacing: "-0.01em"
  h1:
    fontFamily: "Space Grotesk"
    fontSize: 2rem
    fontWeight: 600
    lineHeight: 1.15
  h2:
    fontFamily: "Space Grotesk"
    fontSize: 1.5rem
    fontWeight: 600
    lineHeight: 1.2
  h3:
    fontFamily: "Space Grotesk"
    fontSize: 1.125rem
    fontWeight: 600
    lineHeight: 1.3
  body-md:
    fontFamily: "Inter"
    fontSize: 1rem
    fontWeight: 400
    lineHeight: 1.6
  body-sm:
    fontFamily: "Inter"
    fontSize: 0.875rem
    fontWeight: 400
    lineHeight: 1.55
  label-caps:
    fontFamily: "Inter"
    fontSize: 0.75rem
    fontWeight: 600
    lineHeight: 1.2
    letterSpacing: "0.08em"
    textTransform: uppercase
  code:
    fontFamily: "IBM Plex Mono"
    fontSize: 0.875rem
    fontWeight: 400
    lineHeight: 1.5
spacing:
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 32px
  xxl: 48px
rounded:
  sm: 4px
  md: 8px
  lg: 12px
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "#FFFFFF"
    typography: "{typography.body-md}"
    rounded: "{rounded.sm}"
    padding: 12px
  button-primary-hover:
    backgroundColor: "{colors.ink}"
    textColor: "#FFFFFF"
    rounded: "{rounded.sm}"
    padding: 12px
  divider:
    backgroundColor: "{colors.steel}"
    height: 1px
    width: 100%
  link:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.plate}"
    typography: "{typography.body-md}"
  text-body:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    typography: "{typography.body-md}"
  text-muted:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.slate}"
    typography: "{typography.body-sm}"
  surface-tile:
    backgroundColor: "{colors.white}"
    textColor: "{colors.ink}"
    rounded: "{rounded.md}"
    padding: 24px
  surface-tile-dark:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    rounded: "{rounded.md}"
    padding: 24px
  chip-accent:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    typography: "{typography.label-caps}"
    rounded: "{rounded.md}"
    padding: 6px
  accent-stripe:
    backgroundColor: "{colors.accent}"
    width: 48px
    height: 4px
    rounded: "{rounded.sm}"
  code-block:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    typography: "{typography.code}"
    rounded: "{rounded.md}"
    padding: 16px
---

## Overview

Isolith is the public brand for the Open Engineering Platform (OEP), a
sovereign, self-hosted, open-source engineering suite. The identity is drawn
from the drafting room: warm blueprint paper, ink notes, plate-blue rev clouds,
drafting steel, and a copper highlight like a copper printing plate. The mark
is an isometric block with nested section plates - a solid, engineered object,
not an overclaim. The tone is factual, precise, and calm; nothing shouts.

## Colors

- Ink (#0F2A43): primary text, the mono mark, and hover states. Deep navy-ink,
  heavier than pure black, softer on the eye.
- Plate (#1D5B9E): the brand blue. Top face of the mark, primary actions, and
  links. White text on Plate passes WCAG AA.
- Steel (#7C94AD): drafting rules, dividers, and the open face of the mark on
  light surfaces. Never used for body text.
- Slate (#546981): muted secondary text on paper. Passes WCAG AA for normal
  text.
- Accent (#C8792B): copper, one element per view at most. Decorations and data
  highlights; never body text.
- Paper (#F6F4EF): default page background. Warm, low-glare blueprint paper.
- The dark theme inverts: Ink page, Paper text, Plate actions, Steel rules.

## Typography

Space Grotesk for display and the product wordmark - geometric, engineered,
with an open feel. Inter (or IBM Plex Sans) for body. IBM Plex Mono for code
and lattice data. All are SIL Open Font License and self-hostable, so an
air-gapped install needs no font CDN.

## Layout

A 4px spacing base; surfaces at 8, 16, 24, 32, 48. Content max-width 1120px on
the docs site, 80ch for prose. Mark clearspace equals half the mark height on
every side.

## Elevation & Depth

Flat by default. One soft shadow token for floating controls only:
0 1px 2px rgba(15, 42, 67, 0.18). No gradients on the mark, no heavy 3D.

## Shapes

Small radii throughout: 4-8px for controls, 12px for tiles. The only hard 3D
shape is the isometric mark itself; everything else is flat and calm.

## Components

- button-primary/button-primary-hover: the only high-emphasis action.
- link, text-body, text-muted, surface-tile, surface-tile-dark, chip-accent,
  code-block: the docs-site building blocks. Contrast pairs are WCAG AA-checked
  (see validate_design.py).

## Do's and Don'ts

- Do keep one accent element per view. Do use Paper as the quiet default. Do
  stay factual in copy; this brand does not shout.
- Do not place text in Accent or Steel. Do not gradient the mark. Do not use
  external fonts on the shipped product. Do not use another name, another mark,
  or a visual near-copy of the mark.
