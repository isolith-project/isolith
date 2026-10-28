# Competitive parity documentation

This directory holds the repository copies of the competitive research that
underpins the scope register and the release narrative:

- `parity-2026-10.md` - the feature-for-feature matrix (Isolith vs
  SolidCraft, Purrametric, CADCraft, Open CAD Studio, MitCad, Fusion,
  SOLIDWORKS, CATIA), the per-product fact tables, the price anchors, the
  honesty pass against the scope register and format matrix, and the "behind,
  by release" and "ahead" sections.
- `README.md` - this file: how to refresh the matrix, which URLs and API
  endpoints feed it, what changes quarterly, and the source-type labelling
  rule every competitor claim must follow.

## Purpose

Don directive 2026-10-27: the feature-for-feature deep dive must live in the
repository, not only in the vault, so engineers work from it and the release
narrative quotes it. The vault research note
(`Research/CAD Platform Parity Research`) is the research source of record;
this directory is the working copy that stays in sync with the repository
commit it describes.

## Source of truth

- `Research/CAD Platform Parity Research` (vault, v1.2 as of 2026-10-27) -
  the parity matrix, per-product facts, price anchors, and the behind/ahead
  analysis.
- `Research/Vibecoded CAD Clones Video Research` (vault, v1.0 as of
  2026-10-27) - the per-product inventories that feed the fact tables.
- The Isolith repository at the commit the matrix describes (main `45c9be4`
  as of 2026-10-27): `docs/scope-register.json`,
  `docs-site/format-matrix.json`, `corpus/`, `crates/`.

## How to refresh (quarterly cadence)

The reference snapshot is **quarterly**. A refresh means: re-capture every
live figure at a single retrieval date, re-verify the Isolith commits the
matrix describes, and update the "commit this file describes" and
"retrieved" lines so no cell carries an ambiguous date.

### Step 1 - record the retrieval date

Every refresh sets one explicit date at the top of `parity-2026-10.md`, and
every GitHub API figure is tagged with that date. Figures from a different
date are labelled with their own date inline.

### Step 2 - re-poll the live sources (all on the same day)

URLs (fetch and read, do not assume the figure):

- https://solidcraft.io/ and https://docs.solidcraft.io/
- https://raw.githubusercontent.com/tangency-labs/solidcraft/main/docs/features.md
- https://raw.githubusercontent.com/tangency-labs/solidcraft/main/docs/capabilities.md
- https://raw.githubusercontent.com/tangency-labs/solidcraft/main/README.md
- https://raw.githubusercontent.com/tangency-labs/solidcore/main/README.md
- https://purrametric.deepmagic.de/ and /cad/
- https://www.opencadstudio.com/
- https://mitcad.org/
- https://remade.wiki/projects/solidcraft/ and /projects/mitcad/
- Autodesk Fusion pricing (autodesk.com, Vagon and Capterra summaries)
- SOLIDWORKS Design plans and pricing (solidworks.com/how-to-buy)

GitHub API endpoints (re-poll every figure):

- `repos/tangency-labs/solidcraft` (created, stars, forks, open issues,
  latest release tag)
- `repos/tangency-labs/solidcraft/releases` (latest tag/date, asset count)
- `repos/tangency-labs/solidcraft/contributors` (top contributor commit
  counts)
- `repos/tangency-labs/solidcore` (created, stars, open issues)
- `repos/storytold/cadcraft` (stars, forks, open issues, latest release)
- `repos/mitcad/Mitcad` (stars, open issues, latest release)

Note the "warn" cases the current matrix captures and a refresh should check:
SolidCraft's star count moves by hundreds per day and its release cadence is
multiple versions a day; CADCraft's parity percentages in press coverage are
older than its own roadmap (use the roadmap, not the press figure);
Purrametric has no repository so its cells can only ever be `[vendor]`
claims or `[video]` observations.

### Step 3 - re-verify the Isolith side against the committed evidence

The Isolith column is capped by two committed artifacts:

- `docs/scope-register.json` - the status ceiling for every capability row.
  A matrix cell may NOT claim more than the register row for the same
  capability. If the register says `planned`, the matrix says `plan`, never
  `ship`, even when a crate and tests exist on main; the register must be
  advanced to `building`/`shippable` with a cited green test before the
  matrix may follow.
- `docs-site/format-matrix.json` - the status ceiling for every reader/writer
  row. A format not in the committed format matrix is `none` or, when a
  milestone-gated program exists (CATPart today), `plan` - never `ship` or
  `part` as a reader.

Re-check the matrix row count and the honesty-pass table after any Isolith
cell changes. The honesty-pass table must list every divergence from the
vault research note and why.

### Step 4 - what changes quarterly (the checklist)

- Star/fork/issue counts and latest release tags (they drift fastest).
- Whether SolidCore has moved from migration target to a release kernel
  (tracking issue 4012); if it ships, its kernel cell and the SOLIDCORE
  note change.
- Whether CADCraft's roadmap percentages moved toward the 2027 figures.
- Whether MitCad added or dropped import formats.
- Whether the rectangle of "where Isolith is behind, by release" shrank:
  identity conformance, public surface decision, community intake assets,
  docs-site front door (2027.1 gate), editor breadth, surfacing, sheet metal
  / weldments / mold / routing, browser delivery (2027.2).
- Whether any register row advanced (that advances the matrix cell above it).

### Step 5 - bump the version line on `parity-2026-10.md`

Every edit to `parity-2026-10.md` bumps its version with a one-line change
note (same discipline as the vault). A quarterly re-capture with no changes
still bumps the version to record that the refresh happened.

## Source-type labelling rule (mandatory)

Every competitor claim in `parity-2026-10.md` is labelled with a source
type, in the cell or in the fact table it traces to. The type tells the
reader how much an unverified claim is worth:

- `[repo]` - read from the project's own committed repository
  (docs, source tree, manifests). Strong for what exists, weak for quality:
  a project's own README is not independent verification.
- `[api]` - a GitHub/Gitea API capture (stars, issues, releases,
  contributors) at a recorded date. A moving snapshot, re-check on refresh.
- `[vendor]` - the product's own site or marketing page. Vendor claim,
  not independently verified.
- `[press]` - journalistic reporting of a project's own figures (e.g.
  3D Printing Industry reporting CADCraft's roadmap percentages). Secondhand
  by construction; prefer the primary `[repo]` figure.
- `[video]` - directly observed on screen in a video source (what the LTT
  episode actually shows).
- `[transcript]` - a statement reaching us through a transcript quoting
  someone else (e.g. the Purrametric developer via Reddit posts read in the
  LTT transcript). Secondhand.

Rules:

1. A `[vendor]` claim is never written as fact in prose. It is written as
   "claims X" or "X `[vendor]`", and where the vendor also publishes a
   repository the `[repo]` evidence is preferred.
2. Unverifiable-fabrication risk (Purrametric's kernel, any "exact NURBS
   B-rep from scratch" claim) is always marked `[vendor, unverifiable
   without source]` or `[transcript]`, never left bare.
3. Isolith cells are NOT competitor claims and are never source-type
   labelled; they are capped by the scope register and format matrix and
   carry a test/row reference instead.
4. A refresh that cannot re-fetch a source (404, moved, paywalled) labels
   the cell with "not re-verified <date>" rather than recycling the old
   figure silently.

## Relationship to the scope register

`docs/scope-register.md` links here so a reader who asks "why is this row
planned" lands on the competitive evidence. The register's status tokens are
the ceiling for the matrix's Isolith cells; the register is not downstream of
this file. When the register advances a row, update the matrix and the
honesty-pass table in the same commit.
