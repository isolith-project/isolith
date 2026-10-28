# Isolith

Isolith is an open-source engineering design program. It does the everyday job
of professional CAD: you create and edit 3D parts, put them into assemblies,
and turn them into drawings, without the subscriptions, per-seat fees, or
forced cloud accounts that ship with commercial CAD. Your software and your
data stay on machines you control: it runs entirely inside your own network and
works fully air-gapped, which is why defense teams and suppliers that handle
ITAR data can consider it at all.

It opens the files engineers actually exchange: STEP, CATIA CGR and 3DXML, and
it can pull geometry out of native CATPart files. It gives you a real geometry
kernel (OCCT behind the vendor-neutral `oep-geo` seam), a self-hosted PDM, and
peer-to-peer compute. The docs-site front door carries this same description
and the honest media package (see
`docs-site/assets/media-manifest.md` for the provenance of every capture).

"Isolith" is the name of the program and the product. Its former working
codename was retired in October 2026 because it collided with a registered
trademark; see `assets/brand/NAME.md` for the name record and retirement
statement.

## What it is for

Commercial CAD and PLM tie your data to the vendor's formats, pricing, and in
some cases their cloud. Interop readers are expensive, native files are
undocumented, and for defense and aerospace teams the models can carry data
the customer will not let sit in a foreign data center. Isolith targets that
gap: you keep the data, the formats stay readable, and the whole thing
installs inside your own network. The documented exchange formats (STEP
AP242, QIF, JT) are covered, and the proprietary exchange formats are the
first things the readers handle.

## Status

Pre-release. 2027.1 is the interop/PDM foundation release; the editor is the
product spine. The code builds with all tests passing, and a signed install
candidate for that foundation line (interop readers, geometry core, PDM,
compute, delivery) is published on the Releases page for review. This line is
not the product GA and does not claim to be the editor: the interactive editor
ships along the defined kernel-first sequence and is the product bar. Do not
plan around stable behavior yet; things will change before general
availability.

## Quick start

Two ways to run the whole stack (PDM, viewer, compute dashboard, docs site):
as containers, or from source.

```sh
# Containers
git clone <your-repo-url> isolith
cd isolith/workstreams/delivery/deploy/deploy
./install.sh

# From source
cargo build --release
./target/release/oep-pdm    # REST API on http://127.0.0.1:8787
./target/release/oep-docs   # docs site on http://127.0.0.1:8792
```

Stop a container stack with `./install.sh --down`. The getting-started guide,
admin guide, and the format capability matrix are served by the docs site
itself.

## What's in it

- Readers for the exchange formats teams rely on: CGR (triangle
  tessellation), 3DXML, and STEP AP242 with semantic PMI. QIF and JT are next.
- A geometry core verified against real files, not just synthetic fixtures.
- A self-hosted PDM: vault storage, revisions, roles and audit on a REST API,
  secure by default.
- A compute lane: job scheduling plus sandboxed render and convert workers
  with checked outputs.
- A preview viewer and this documentation site, both part of the same stack.

Limits are stated rather than hidden: the format matrix tells you exactly what
each reader opens and does not open today, and the native CATPart reader is
gated by a milestone decision, not a date.

## Build and test

```sh
cargo build --workspace && cargo test
uv run pytest tests/
```

## License and contributing

Apache-2.0 for applications and libraries; AGPL-3.0 for the server, PDM, and
P2P components. No GPL-3.0 ships in the delivered software. Exports are
classified per release.

- Charter: `charter.md`
- Contributions: `CONTRIBUTING.md`
- Identity kit: `assets/brand/`

## Documentation

- Docs site (getting started, admin guide, format matrix):
  `docs-site/index.html`
- Repo layout: `docs/layout.md`
