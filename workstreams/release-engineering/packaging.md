# P12-T04: Packaging Matrix for Shipped Binaries

Status: DONE (2026-10-18)
Branch: task/p12t04-packaging-1018a
Owner: P12 QA/CI/CD release engineering lane

Every binary this program ships gets a buildable package in one or more of the
target formats: Docker image (already exists for the stack), AppImage and/or
Flatpak for Linux desktop, and a WiX MSI for Windows. This file is the
authoritative matrix; the build recipes live in `workstreams/release-engineering/packaging/` and are
parameterized by binary name so any new bin registered in the Cargo workspace
picks up a package by construction.

## Invariants

- Clean-room only: every package contains only binaries compiled from this
  repo's own source plus public-domain/system runtimes. No vendor binaries, no
  copied source (program invariant, board.program.json).
- No corpus/example geometry ships inside any package (decision #7: samples
  are development-only reference material).
- Packaging is reproducible from a clean checkout: recipes take the binary
  name, read the same workspace `target/release`, and produce the same
  artifact shape (verified hashes on the Linux build below).
- On-prem posture (decision #10): default host binds stay 127.0.0.1 everywhere
  they exist; packages do not phone home and run with no network.

## Format decision

- Docker: one multi-service image `keelworks/oep-stack` (P10-M1-T01) is the
  official server/worker distribution; per-binary container images are NOT
  created separately because the stack image already carries the full shipped
  binary set and adds nothing reproducible per binary. Matrix column
  "Docker" = present in `keelworks/oep-stack`.
- Linux desktop: AppImage is the primary desktop format (single-file,
  no root, FUSE or extract-and-run), Flatpak is the alternate path (sandboxed,
  portal-integrated). Both recipes are provided; AppImage is the one built and
  runtime-verified in this card.
- Windows: WiX MSI via `wixl` (msitools) — cross-compiled `.exe` + `.wxs`
  source. Build-only on U1 (no Windows runtime here); install target in the
  field is the garage PC (192.168.0.43, decision #9).

## Matrix (binary x format)

Legend: B = buildable recipe provided and actually built+hashes verified this
card; r = recipe provided (buildable from a clean checkout by the same script);
— = intentionally no package of this format (see note); P = placeholder,
binary does not yet exist in the integration tree (documents the recipe that
will apply when it lands; acceptance holds for every binary that exists).

| Shipped binary            | Description                              | Docker | AppImage | Flatpak | Windows MSI |
|---------------------------|------------------------------------------|:------:|:--------:|:-------:|:-----------:|
| oep-pdm                   | P8 PDM/ENOVIA-lite server                |   B    |    r     |    r    |      B      |
| oep-web                   | P8-M4 web UI / P6 preview viewer server  |   B    |    B     |    r    |      B      |
| oep-opsdash               | P9 ops dashboard                         |   B    |    r     |    r    |      r      |
| oep-render-worker         | P9 headless render worker                |   B    |    —     |    —    |      r      |
| oep-batch-convert         | P9 export/convert batch driver           |   B    |    r     |    r    |      r      |
| oep-ga-gate / oep-ga-node | P9 GA-gate harness driver + agent        |   B    |    —     |    —    |     r       |
| oep-update-check          | P10-M5 advisory update checker           |   B    |    r     |    r    |      r      |
| oep-update-sign           | P10-M5 offline bundle signing tool       |   B    |    r     |    r    |      r      |
| oep-farm-drill            | P9 farm drill/probe                      |   B    |    —     |    —    |     r       |
| sbox-probe                | P9 sandbox probe                         |   B    |    —     |    —    |     —       |
| oep-cgr2gltf              | P2-M5 CLI (delivered PR #50, not merged) |   r    |    r     |    r    |      r      |
| oep-cgr2step              | P2-M5 CLI (delivered PR #48, not merged) |   r    |    r     |    r    |      r      |
| oep-3dxml tooling         | P3 3DXML writer tool (crate only today)  |   r    |    r     |    r    |      r      |
| oep-studio                | P14-M1-T03 integrated shell (target)     |   —    |    P     |    P    |      P      |

Notes on specific cells:

- sbox-probe: sandbox-internal probe, no standalone desktop/Win package (worker
  runtime concern only); present in the Docker image.
- Headless workers (render, gate, drill) have no meaningful AppImage/Flatpak
  GUI surface; a `—` means the recipe is not created for a headless-only
  artifact that already ships in Docker. Any of them can be wrapped by the
  generic `build-appimage.sh <bin>` if a desktop variant is ever wanted — the
  recipe is parameterized, so nothing blocks it.
- oep-cgr2gltf / oep-cgr2step: delivered as PR #50 / #48 on their feed
  branches; they are not yet in the integration tree (checked at e67b8fc). The
  packaging scripts are generic-by-bin so the recipes marked `r` become `B`
  automatically the moment either binary builds from the workspace.
- oep-3dxml: current crate has no `[[bin]]` yet (P3 writer landed as a
  library); recipe provided, becomes `B` when a bin target exists.
- oep-studio (P14): not yet built — placeholder row documents the intended
  AppImage/Flatpak/MSI shape (it will reuse `build-appimage.sh oep-studio`
  etc.).

## Recipes

All recipes live under `workstreams/release-engineering/packaging/` and are parameterized by binary name.
They consume the SAME `target/release` build of the workspaces (or a
cross-target `target/<triple>/release` for Windows).

| Recipe | Purpose | Usage |
|--------|---------|-------|
| `build-appimage.sh <bin>` | Build a Linux desktop AppImage (aarch64/x86_64 auto from host) from a `.desktop` + AppDir + the release binary | `bash workstreams/release-engineering/packaging/build-appimage.sh oep-web` |
| `flatpak/org.keelworks.oep.yml` | Flatpak manifest (Freedesktop 24.08 runtime), finish-args network=local + browser capture for the viewer | `bash workstreams/release-engineering/packaging/build-flatpak.sh oep-web` |
| `build-msi.sh <bin> <arch>` | Cross-compile `<bin>` to the given Windows triple, write WiX `.wxs`, build `.msi` with wixl (msitools, in a hermetic debian container) | `bash workstreams/release-engineering/packaging/build-msi.sh oep-pdm x86_64-pc-windows-gnu` |
| `oep-studio.desktop.in` | Template launcher + icon seeded for the future studio shell | (placeholder) |
| `Dockerfile` | Already shipped at `workstreams/delivery/deploy/deploy/Dockerfile` (stack image); referenced, not duplicated | `docker compose -f workstreams/delivery/deploy/deploy/docker-compose.yml build` |

Linux desktop package host verification (this card): aarch64 U1-class host
(promaxgb10-83f1, Ubuntu 24.04, FUSE-less so AppImage runs with
`--appimage-extract-and-run`), see `workstreams/release-engineering/proof/`.

## Proof recorded in workstreams/release-engineering/proof/

- `packaging-proof.json` — machine-readable run summary (built artifacts,
  sha256, run status).
- `packaging-proof.md` — human narrative of the two acceptance runs.
- `oep-web-appimage-run.log` — oep-web AppImage run: HTTP 200, "Item explorer".
- `oep-pdm-appimage-run.log` / `oep-pdm-appimage-run-serve.log` — oep-pdm
  AppImage run: health `{"status":"ok",...}` HTTP 200.
- `oep-pdm-msi-build.log` / `oep-update-check-msi-build.log` — wixl MSI builds.
- `SHA256SUMS` — the four package binaries attached to the task.

## CI note

The generic packaging scripts are intended to be replayed by P12-T05 (release
train) on every release branch; they must stay executable-free of credentials
and network-indifferent (offline acceptable: they all build from local source +
local cargo cache).
