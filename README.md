# Isolith

Isolith is an open-source engineering platform you run on your own machines,
built for teams that need to own their CAD data. It reads the proprietary
geometry files suppliers actually exchange (CGR, 3DXML, CATPart), gives you a
geometry core, a self-hosted PDM, and peer-to-peer compute, and it replaces
per-seat license fees with software you install yourself.

"Isolith" is the product name. The project's working codename, Keelworks, is
kept internal because it collides with a registered trademark; see
`assets/brand/NAME.md`.

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

Pre-release. The code builds with all tests passing, and a signed install
candidate is published on the Releases page for review. Do not plan around
stable behavior yet; things will change before general availability.

## Quick start

Two ways to run the whole stack (PDM, viewer, compute dashboard, docs site):
as containers, or from source.

```sh
# Containers
git clone <your-repo-url> keelworks
cd keelworks/workstreams/P10-M1/deploy
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
