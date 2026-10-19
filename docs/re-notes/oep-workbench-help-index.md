# OEP workbench command help index (generated)

This file is GENERATED from the command metadata in `crates/oep-shell/src/commands.rs` (the single source of truth for the workbench help surface). Do not hand-edit. Regenerate and keep the committed copy in sync with:

```
cargo run --example workbench -- --write-help-index docs/re-notes/oep-workbench-help-index.md
```

help-index-version: 1.0.0

surface: oep-shell workbench command surface

## Index

| Command | Usage | Category | Summary |
| --- | --- | --- | --- |
| load:cgr | `load:cgr:PATH` | interchange | Load a CGR file (classic .cgr or modern V5_CFV2) into the workbench |
| load:3dxml | `load:3dxml:PATH` | interchange | Load a 3DXML file (ZIP or standalone XML) into the workbench |
| load:step | `load:step:PATH` | interchange | Load an ISO 10303-21 STEP file into the workbench |
| save | `save:cgr|3dxml|step` | interchange | Request an interchange save lane (honest pending where no writer exists yet) |
| open-pdm | `open-pdm:ITEM_ID:REVISION` | pdm | Open a revision from the PDM vault and make it the current document |
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

