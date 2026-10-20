# P12-T04 Packaging proof (2026-10-18)

Host: promaxgb10-83f1 (U1-class, aarch64, Ubuntu 24.04, FUSE-less)
Base: e67b8fc (p10m5t02-ga-gate-1018a integration head)
Branch: task/p12t04-packaging-1018a

## Recipes (workstreams/release-engineering/packaging/)

- workstreams/release-engineering/packaging/build-appimage.sh <bin>   — Linux desktop AppImage (by-bin)
- workstreams/release-engineering/packaging/build-flatpak.sh <bin>    — Flatpak manifest + bundle (alt path)
- workstreams/release-engineering/packaging/build-msi.sh <bin> [triple] — Windows WiX MSI (wixl, hermetic)
- Docker: pre-existing workstreams/delivery/deploy/deploy stack image keelworks/oep-stack:0.1.0

Matrix: workstreams/release-engineering/packaging.md (every shipped binary x Docker/AppImage/Flatpak/MSI).

## Built this run

| Artifact | Size | sha256 |
|---|---|---|
| oep-web-keelworks-oep-aarch64.AppImage | 1741184 | b4ab6896d3a332f41ab696caa3244624ec62b882f7352d006ed903fbcb2804d3 |
| oep-pdm-keelworks-oep-aarch64.AppImage | 2937216 | 404520769a18e6771a4d8c71428ae9b1208f6a41ad749ed346ab4180ac1a6226 |
| keelworks-oep-pdm-x86_64-pc-windows-gnu-0.1.0.msi | 4356096 | 1f2903d15e33d0318986c1f2f889dbe7962f5e272578978bc3f27f0f1b336e33 |
| keelworks-oep-update-check-x86_64-pc-windows-gnu-0.1.0.msi | 813568 | 6f43617828c8afa269043af1e788aa5949fc0b41de1b013035e4d5c61d7b3e7f |

## Linux desktop package installs and runs (acceptance)

1. `./oep-web-keelworks-oep-aarch64.AppImage --appimage-extract` → AppDir with
   `usr/bin/oep-web`; `AppRun` execs the binary (FUSE-less host).
2. Direct AppImage exec (the normal desktop path on FUSE hosts; fallback
   `--appimage-extract-and-run` here):
   `./oep-web-keelworks-oep-aarch64.AppImage --addr 127.0.0.1:8792`
   → log: `oep-web: serving UI + REST API v1 at http://127.0.0.1:8792`
   → `curl http://127.0.0.1:8792/` → HTTP 200, `<title>Item explorer</title>`.

The packaged binary itself served the viewer: installed and runs.
strace-free plain rust binary, no system package dependencies exposed by the
package (only libc/libgcc/libm via the host, standard for an AppImage).

## MSI builds (acceptance: build-only, no Windows runtime on U1)

- Cross-compile: `cargo build --release --target x86_64-pc-windows-gnu -p oep-pdm`
  (mingw; same chain as bastion/stronghold per gitea skill).
- `.wxs` written, `wixl` (msitools/debian container) produced
  `keelworks-oep-pdm-x86_64-pc-windows-gnu-0.1.0.msi`:
  `Composite Document File V2 ... MSI Installer, Template: Intel;1033,
  msitools 0.101`. Valid MSI database; install target is the garage PC
  (192.168.0.43, decision #9) in the field.

## Repo health

- `python3 tests/monorepo_layout_test.py` → ALL CHECKS PASSED (incl. cargo
  build --workspace + cargo test --workspace).
- `pytest tests/` → 93 passed (GA-gate live-service suites excluded).
- `docker compose -f workstreams/delivery/deploy/deploy/docker-compose.yml config` → OK; stack
  image present.

## Artifacts

The four package binaries are attached to the task (kanban attachments) and
hashed in workstreams/release-engineering/proof/SHA256SUMS; build logs in workstreams/release-engineering/proof/*.log.
