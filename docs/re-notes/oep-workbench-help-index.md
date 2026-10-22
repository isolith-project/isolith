# OEP workbench command help index (generated)

This file is GENERATED from the command metadata in `crates/oep-shell/src/commands.rs` (the single source of truth for the workbench help surface). Do not hand-edit. Regenerate and keep the committed copy in sync with:

```
cargo run --example workbench -- --write-help-index:docs/re-notes/oep-workbench-help-index.md
```

help-index-version: 1.2.0

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
| editor:open | `editor:open` | editor | Open a fresh interactive editor session (blank sketch document) |
| sk:point | `sk:point:X:Y` | editor | Add a point at (X, Y) in the sketch plane; returns its id (p0, p1, ...) |
| sk:line | `sk:line:START:END` | editor | Add a line segment between two existing points; returns its id (l0, l1, ...) |
| sk:constrain | `sk:constrain:KIND:ARGS...` | editor | Add a sketch constraint (fixed, horizontal, vertical, coincident, length, distance) |
| sk:solve | `sk:solve` | editor | Solve the sketch against the variable set and write solved coordinates back |
| sk:render | `sk:render:PATH` | editor | Render the current viewport (solid + sketch overlay) to PPM or PNG |
| sk:props | `sk:props` | editor | Print the sketch summary (points, lines, constraints, solved dof) |
| vars:list | `vars:list` | editor | List the variable-set entries (sorted by name) |
| vars:set | `vars:set:NAME:VALUE` | editor | Set (or add) a variable; a solid feature recomputes live |
| feat:extrude | `feat:extrude:DEPTH` | editor | Create an extrude from the solved sketch profile by DEPTH (number or variable name) |
| feat:revolve | `feat:revolve:ANGLE` | editor | Create a revolve from the solved sketch profile by ANGLE radians about Z |
| feat:recompute | `feat:recompute` | editor | Re-solve the sketch and re-derive the solid feature from the current model |
| view:frame | `view:frame` | editor | Frame the camera to the model bounds |
| view:orbit | `view:orbit:YAW:PITCH` | editor | Orbit the camera by YAW / PITCH (radians) |
| view:zoom | `view:zoom:FACTOR` | editor | Zoom the camera by a multiplicative FACTOR (< 1 moves closer) |
| view:pan | `view:pan:DX:DY` | editor | Pan the camera target by DX / DY world units |
| view:pick | `view:pick:X:Y` | editor | Pick the scene primitive under screen pixel (X, Y) and select it |
| export:step | `export:step:PATH` | editor | Export the derived solid as an AP214 STEP file |
| props | `props` | editor | Print current model measures (volume, bbox, features, sketch stats, variables) |
| undo | `undo` | editor | Undo the last edit (restores the prior canonical document state) |
| redo | `redo` | editor | Redo the last undone edit |
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

### editor:open

Usage: `editor:open`

Category: editor

Starts an EditorSession: a canonical oep-geo document with one part, one body, an empty sketch feature and an empty variable set. All sk:/vars:/feat:/view:/export: commands then operate on this session. The same command set is what a mouse-driven editor calls under the pointer; here it is text-callable (the agent/automation lane).

Arguments:

- (none)

Examples:

- `oep-workbench editor:open`
- `oep-workbench sk:point:0:0`
- `oep-workbench sk:solve`

### sk:point

Usage: `sk:point:X:Y`

Category: editor

Appends a point at the given sketch-plane coordinates. The assigned id is printed (p0, p1, ...) so later sk:line and sk:constrain commands can reference it.

Arguments:

- `X` (required): Sketch-plane x coordinate
- `Y` (required): Sketch-plane y coordinate

Examples:

- `oep-workbench sk:point:0:0`
- `oep-workbench sk:point:3:1`

### sk:line

Usage: `sk:line:START:END`

Category: editor

Appends a line segment from point START to point END. Both points must exist. A closed loop of lines is what extrude/revolve consume.

Arguments:

- `START` (required): Start point id (e.g. p0)
- `END` (required): End point id (e.g. p1)

Examples:

- `oep-workbench sk:line:p0:p1`

### sk:constrain

Usage: `sk:constrain:KIND:ARGS...`

Category: editor

Appends a constraint to the sketch. KIND and ARGS:
  fixed:POINT:X:Y          anchor a point at (X,Y)
  horizontal:LINE          force a line horizontal
  vertical:LINE            force a line vertical
  coincident:A:B           coincide two points
  length:LINE:VAL          driven length (VAL = number, or a variable-set entry name)
  distance:A:B:VAL         driven distance between two points (number or variable name)
When VAL is not a number it references the variable set by name, so an edit to the variable re-solves the sketch (drive lane).

Arguments:

- `KIND:ARGS...` (required): Constraint kind and colon-separated arguments (see long help)

Examples:

- `oep-workbench sk:constrain:fixed:p0:0:0`
- `oep-workbench sk:constrain:length:l0:width`

### sk:solve

Usage: `sk:solve`

Category: editor

Runs the oep-sketch solver over the current sketch + variable set. On convergence the solved point coordinates are written back into the canonical document and the report (dof, residual, driven dimensions) is printed. A non-converged solve is a typed error, never a fabricated solution.

Arguments:

- (none)

Examples:

- `oep-workbench sk:solve`

### sk:render

Usage: `sk:render:PATH`

Category: editor

Renders the current scene through the deterministic CPU raster to PATH. .ppm writes a P6 PPM, .png a PNG. Prints the image SHA-256 so two consecutive renders prove whether the model moved.

Arguments:

- `PATH` (required): Output image path (.ppm or .png)

Examples:

- `oep-workbench sk:render:before.png`
- `oep-workbench vars:set:width:120`
- `oep-workbench sk:render:after.png`

### sk:props

Usage: `sk:props`

Category: editor

Prints the current sketch geometry and constraint counts plus the last solve's degrees of freedom and residual, if any.

Arguments:

- (none)

Examples:

- `oep-workbench sk:solve`
- `oep-workbench sk:props`

### vars:list

Usage: `vars:list`

Category: editor

Prints every variable-set entry: name = value. Dimensions that reference a variable by name resolve against this set at solve time.

Arguments:

- (none)

Examples:

- `oep-workbench vars:list`

### vars:set

Usage: `vars:set:NAME:VALUE`

Category: editor

Sets the variable NAME to VALUE. If the session already carries a solid feature (extrude or revolve), the model recomputes immediately: the sketch re-solves and the solid follows the parameter (drive lane). Prints the solid volume before and after so the update is provable.

Arguments:

- `NAME` (required): Variable name
- `VALUE` (required): Numeric value

Examples:

- `oep-workbench vars:set:width:120`

### feat:extrude

Usage: `feat:extrude:DEPTH`

Category: editor

Derives the solid by extruding the solved closed sketch profile by DEPTH (a literal length or a variable-set entry name) along +Z. The solid is tessellated deterministically from the real model, the body bounding box is set from it, and the volume/faces/bbox are printed. Requires the sketch to be solved and to form a single closed loop.

Arguments:

- `DEPTH` (required): Extrude depth (number or variable name)

Examples:

- `oep-workbench feat:extrude:10`
- `oep-workbench feat:extrude:depth`

### feat:revolve

Usage: `feat:revolve:ANGLE`

Category: editor

Derives the solid by revolving the solved closed profile about the Z axis by ANGLE radians (a literal or a variable-set entry name). The profile is a meridian cross section: x is radial distance (>= 0), y axial height, matching the kernel seam's revolve.

Arguments:

- `ANGLE` (required): Revolve angle in radians (number or variable name)

Examples:

- `oep-workbench feat:revolve:3.14159265`

### feat:recompute

Usage: `feat:recompute`

Category: editor

Full recompute: re-solves the sketch against the current variable set, rebuilds the closed profile, re-tessellates the solid feature, and updates the body bounding box and viewport mesh. This is the explicit form of the live recompute vars:set already triggers.

Arguments:

- (none)

Examples:

- `oep-workbench feat:recompute`

### view:frame

Usage: `view:frame`

Category: editor

Fits the orbit camera to the union of the solid and sketch bounds so the next render shows the whole model.

Arguments:

- (none)

Examples:

- `oep-workbench view:frame`

### view:orbit

Usage: `view:orbit:YAW:PITCH`

Category: editor

Rotates the orbit camera around the target (the viewport orbit gesture).

Arguments:

- `YAW` (required): Azimuth delta in radians
- `PITCH` (required): Elevation delta in radians

Examples:

- `oep-workbench view:orbit:0.8:0.5`

### view:zoom

Usage: `view:zoom:FACTOR`

Category: editor

Scales the camera distance (the viewport zoom gesture).

Arguments:

- `FACTOR` (required): Multiplicative zoom factor

Examples:

- `oep-workbench view:zoom:0.8`

### view:pan

Usage: `view:pan:DX:DY`

Category: editor

Moves the orbit target along the camera right/up screen axes (the viewport pan gesture).

Arguments:

- `DX` (required): Pan along the camera right axis
- `DY` (required): Pan along the camera up axis

Examples:

- `oep-workbench view:pan:5:-3`

### view:pick

Usage: `view:pick:X:Y`

Category: editor

Unprojects the pixel through the camera and selects the nearest primitive (the solid, a sketch line or a point handle). Prints the selected id; the next render highlights it in amber.

Arguments:

- `X` (required): Pixel x (0..639)
- `Y` (required): Pixel y (0..479)

Examples:

- `oep-workbench view:pick:320:240`

### export:step

Usage: `export:step:PATH`

Category: editor

Writes the tessellated solid as a STEP (ISO 10303-21) faceted B-Rep via the oep-geo exporter. Requires a solid feature (extrude or revolve). The exported bytes re-import through the importer hub.

Arguments:

- `PATH` (required): Output .step path

Examples:

- `oep-workbench export:step:edpart.step`

### props

Usage: `props`

Category: editor

Prints the derived solid volume and bounding box, the document feature count, the last solve's degrees of freedom, and the variable set.

Arguments:

- (none)

Examples:

- `oep-workbench props`

### undo

Usage: `undo`

Category: editor

Rewinds the session to the previous document snapshot and re-derives the solid. A typed error reports when nothing can be undone.

Arguments:

- (none)

Examples:

- `oep-workbench vars:set:width:120`
- `oep-workbench undo`
- `oep-workbench redo`

### redo

Usage: `redo`

Category: editor

Re-applies the most recently undone document snapshot and re-derives the solid. A typed error reports when nothing can be redone.

Arguments:

- (none)

Examples:

- `oep-workbench vars:set:width:120`
- `oep-workbench undo`
- `oep-workbench redo`

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

