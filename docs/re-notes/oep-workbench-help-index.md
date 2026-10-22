# OEP workbench command help index (generated)

This file is GENERATED from the command metadata in `crates/oep-shell/src/commands.rs` (the single source of truth for the workbench help surface). Do not hand-edit. Regenerate and keep the committed copy in sync with:

```
cargo run --example workbench -- --write-help-index:docs/re-notes/oep-workbench-help-index.md
```

help-index-version: 1.1.0

surface: oep-shell workbench command surface

## Index

| Command | Usage | Category | Summary |
| --- | --- | --- | --- |
| load:cgr | `load:cgr:PATH` | interchange | Load a CGR file (classic .cgr or modern V5_CFV2) into the workbench |
| load:3dxml | `load:3dxml:PATH` | interchange | Load a 3DXML file (ZIP or standalone XML) into the workbench |
| load:step | `load:step:PATH` | interchange | Load an ISO 10303-21 STEP file into the workbench |
| save | `save:cgr|3dxml|step` | interchange | Request an interchange save lane (honest pending where no writer exists yet) |
| open-pdm | `open-pdm:ITEM_ID:REVISION` | pdm | Open a revision from the PDM vault and make it the current document |
| assem:open | `assem:open` | assembly | Open the committed 3-part assembly fixture into the interactive surface |
| assem:instances | `assem:instances` | assembly | List the assembly instances and their current placements |
| assem:mates | `assem:mates` | assembly | List the current mate set (indexed) |
| assem:mate-add | `assem:mate-add:KIND:ARGS...` | assembly | Create a mate and re-solve live; instances follow or a typed error reports |
| assem:mate-edit | `assem:mate-edit:INDEX:KIND:ARGS...` | assembly | Replace a mate by index and re-solve live |
| assem:mate-del | `assem:mate-del:INDEX` | assembly | Delete a mate by index and re-solve live |
| assem:solve | `assem:solve` | assembly | Solve the current mate set and write placements back |
| assem:render | `assem:render:PATH` | assembly | Render the current assembly pose to a PPM image (the viewport) |
| assem:animate | `assem:animate:FRAMES:OUT_DIR` | assembly | Play mechanism motion (crank-slider) and record a deterministic trace |
| save-pdm | `save-pdm:ITEM_ID:REVISION[:SUMMARY]` | pdm | Publish the current document as a new PDM vault revision |
| journal | `journal` | inspection | Print the ordered operation journal |
| help | `help [COMMAND]` | help | Print this command index, or the full help for one command |

## Commands

### load:cgr

Usage: `load:cgr:PATH`

Category: interchange

Reads CGR bytes and runs them through the importer hub (oep-import). Both eras are latched: the classic polygon-soup .cgr extractor and the modern V5_CFV2 spatial-mesh decoder. The loaded canonical document becomes the current model.

Arguments:

- `PATH` (required): Path to the CGR file (classic .cgr or modern V5_CFV2)

Examples:

- `oep-workbench load:cgr:nist_stc_06_part.cgr`

### load:3dxml

Usage: `load:3dxml:PATH`

Category: interchange

Reads 3DXML bytes and runs them through the importer hub. Both the ZIP container and the standalone XML rep are accepted; the loaded canonical document becomes the current model.

Arguments:

- `PATH` (required): Path to the 3DXML file (ZIP or standalone .3dxml XML)

Examples:

- `oep-workbench load:3dxml:cube1.3dxml`

### load:step

Usage: `load:step:PATH`

Category: interchange

Reads STEP (ISO 10303-21) bytes through the vendor-neutral oep-step lane and maps the faceted part onto the canonical oep-geo model. The loaded document becomes the current model.

Arguments:

- `PATH` (required): Path to the ISO 10303-21 STEP file

Examples:

- `oep-workbench load:step:box-reference.step`

### save

Usage: `save:cgr|3dxml|step`

Category: interchange

Calls the interchange save lane for the named format. Writers land on their own lanes (3DXML writer P3-M4, STEP/OBJ/glTF exporters P6-M4); until a writer reaches this stack the command answers with a typed InterchangeSavePending that names the owning lane. It never fabricates output.

Arguments:

- `FORMAT` (required): One of cgr, 3dxml or step

Examples:

- `oep-workbench save:step`

### open-pdm

Usage: `open-pdm:ITEM_ID:REVISION`

Category: pdm

Reads a vault revision through the P8 PdmClient contract (today the in-memory local stub), restores the canonical oep-geo JSON and makes it the current document.

Arguments:

- `ITEM_ID` (required): Vault item id
- `REVISION` (required): Vault revision to open

Examples:

- `oep-workbench open-pdm:BOX-01:A`

### assem:open

Usage: `assem:open`

Category: assembly

Loads the committed editor fixture (oep-assem golden: base plate, cap, peg) plus its mate set into the assembly editing surface. The fixture opens deliberately mis-placed and tilted so a live solve has visible work to do. Instance placement is visible with assem:instances and in the viewport with assem:render.

Arguments:

- (none)

Examples:

- `oep-workbench assem:open`

### assem:instances

Usage: `assem:instances`

Category: assembly

Prints one line per assembly instance: id, source part id and the world placement of the part origin. After a live solve the lines reflect the solved placements (the instances follow the mate edit).

Arguments:

- (none)

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:solve`
- `oep-workbench assem:instances`

### assem:mates

Usage: `assem:mates`

Category: assembly

Prints one line per mate in the constraint set with its index and kind (fixed, coincident, on-plane, plane-distance, parallel). Indices are what assem:mate-edit and assem:mate-del consume.

Arguments:

- (none)

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:mates`

### assem:mate-add

Usage: `assem:mate-add:KIND:ARGS...`

Category: assembly

Appends a mate to the constraint set and immediately re-solves the assembly through the vendor-neutral seam. On a convergent solve the report prints and the instances follow. On a diverging solve the mate is kept, a typed error is reported (solver-failed and friends), and the previous converged placement is left untouched. KIND plus ARGS: fixed INSTANCE; coincident INSTA ANCHA INSTB ANCHB; on-plane INST ANCH P_INST P_ANCH; plane-distance INST ANCH P_INST P_ANCH DIST; parallel INSTA ANCHA INSTB ANCHB. Args are colon-separated.

Arguments:

- `KIND:ARGS...` (required): Mate kind and its colon-separated arguments (see long help)

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:mate-add:plane-distance:cap:origin:base:inX:30`

### assem:mate-edit

Usage: `assem:mate-edit:INDEX:KIND:ARGS...`

Category: assembly

Replaces the mate at INDEX in the constraint set and re-solves live. Same outcome rules as assem:mate-add. An out-of-range index is a typed rejection.

Arguments:

- `INDEX:KIND:ARGS...` (required): Mate index, then the replacement mate (same grammar as mate-add)

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:mate-edit:2:plane-distance:cap:origin:base:inX:30`

### assem:mate-del

Usage: `assem:mate-del:INDEX`

Category: assembly

Removes the mate at INDEX and re-solves live. Deleting a constraint relaxes the assembly, so this normally converges and the instances settle into the now-looser placement. Out-of-range indices are typed rejections.

Arguments:

- `INDEX` (required): Index into the mate set (see assem:mates)

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:mate-del:4`
- `oep-workbench assem:solve`

### assem:solve

Usage: `assem:solve`

Category: assembly

Runs the assembly solver over the current mate set through the vendor-neutral seam and, on success, writes the solved placements into the document. Prints the report (dof, moved instances, residuals). On divergence it prints a typed error and leaves the document poses untouched.

Arguments:

- (none)

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:solve`

### assem:render

Usage: `assem:render:PATH`

Category: assembly

Renders the current instance placements as bounding-box proxies to a P6 PPM file at PATH using the deterministic CPU raster. Prints the image SHA-256 so two consecutive renders prove whether the instances followed a solve.

Arguments:

- `PATH` (required): Output PPM path

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:render:before.ppm`
- `oep-workbench assem:solve`
- `oep-workbench assem:render:after.ppm`

### assem:animate

Usage: `assem:animate:FRAMES:OUT_DIR`

Category: assembly

Drives the kinematics lane (P7-M3-T02 damped Gauss-Newton) through a full crank turn of the committed crank-slider mechanism built from its joint set, recording FRAMES deterministic per-frame poses to OUT_DIR/trace.json plus rendered PPM frames and a manifest. Prints the trace SHA-256: identical on every run, which is the determinism proof.

Arguments:

- `FRAMES` (required): Number of animation frames (one crank turn)
- `OUT_DIR` (required): Output directory for trace.json, manifest.json and frames

Examples:

- `oep-workbench assem:open`
- `oep-workbench assem:animate:36:demos/editor-assem-motion`

### save-pdm

Usage: `save-pdm:ITEM_ID:REVISION[:SUMMARY]`

Category: pdm

Serializes the current canonical document to oep-geo JSON and publishes it as a new revision through the P8 PdmClient contract. The summary is optional free-form change text; a save with no document loaded fails with a clear message.

Arguments:

- `ITEM_ID` (required): Vault item id
- `REVISION` (required): Vault revision to write
- `SUMMARY` (optional): Optional free-form change summary

Examples:

- `oep-workbench save-pdm:BOX-01:A:initial`

### journal

Usage: `journal`

Category: inspection

Prints every action outcome recorded on the workbench journal for this process, oldest first. The journal is the observable, machine-callable record of the session.

Arguments:

- (none)

Examples:

- `oep-workbench journal`

### help

Usage: `help [COMMAND]`

Category: help

With no argument, prints the full command index grouped by category. With a command name (for example help:load:cgr) prints that command's full documentation: usage, arguments, description and examples. Prefix --json for the machine-readable index, --markdown for the generated markdown document, or --write-help-index PATH to regenerate the committed help index.

Arguments:

- `COMMAND` (optional): Command name to document, e.g. load:cgr

Examples:

- `oep-workbench help`
- `oep-workbench help:load:cgr`
- `oep-workbench help:--json`

