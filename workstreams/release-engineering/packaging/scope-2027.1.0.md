# 2027.1.0 desktop binary scope decision

Decision record for CROSS-PLATFORM-INSTALLERS card. Decided by the release
engineering lane on 2027-10-27 against the frozen main head `7dffa86`.

## What a real user downloads on 2027.1.0 (Linux + Windows)

One desktop download per platform carries the EDITOR SHELL plus the CLI tools
and the update checker. The PDM server, web UI and workers are the Docker
distribution (DOCKER-NAS-DEPLOYMENT), not part of the desktop download.

| Binary             | What it is                 | In desktop download? | Packaging on Linux        | Windows |
|--------------------|----------------------------|:--------------------:|---------------------------|---------|
| `oep-workbench`    | editor shell (the current  | YES (primary)        | AppImage + .deb + Flatpak | MSI     |
|                    | shell binary name, P7-M5)  |                      |                           |         |
| `oep-viewer`       | viewport library embedded  | n/a (library)        | (ships inside workbench)  |         |
| `oep-web`          | web UI / viewer server     | NO                   | Docker                    | Docker  |
| `oep-batch-convert`| CLI conversion driver     | YES                  | .deb / AppImage           | MSI     |
| `oep-cgr2gltf`     | CLI                        | YES                  | .deb / AppImage           | MSI     |
| `oep-cgr2step`     | CLI                        | YES                  | .deb / AppImage           | MSI     |
| `oep-update-check` | advisory update checker    | YES                  | .deb / AppImage           | MSI     |
| `oep-update-sign`  | offline bundle signer      | admin-only (repo)    | (release tool)            |         |
| `oep-pdm`          | PDM server                 | NO                   | Docker                    | Docker  |
| `oep-render-worker`, `oep-batch-convert`(worker), `oep-opsdash`, `oep-farm-drill`, `oep-ga-*`, sbox-probe | workers / gates | NO | Docker | Docker |

Rationale, per card item 4:

- The **editor shell** is the desktop product: the current shell binary name
  is `oep-workbench` (P7-M5-T02 command surface). Its viewport (oep-viewer)
  is embedded in the shell, so the "viewer" a desktop user sees is inside
  oep-workbench; there is no standalone desktop viewer binary this train.
- **oep-web** is the web UI + viewer server: the card puts web UI in the
  Docker distribution, so it is not a desktop download.
- **CLI tools** (batch-convert, cgr2gltf, cgr2step) ship as utilities in the
  desktop package.
- **oep-update-check** ships with every desktop install (advisory, signed
  manifest, non-blocking).
- **oep-update-sign** is release-engine/admin tooling, kept in the repo
  (published sources) and the Docker image, not sold to desktop users.
- The PDM server, web UI and workers are the Docker distribution.

## Verification state per binary (2027-10-27)

- `oep-workbench`: built Linux aarch64 (release), AppImage (built + editor
  workflow PASS on U1), .deb (dpkg -i PASS on clean Ubuntu 24.04 + workflow),
  Flatpak (hermetic build), Windows MSI (built, **installed + editor workflow
  PASS on the garage PC**, 14/14 smoke checks).
- `oep-batch-convert` / `oep-cgr2gltf` / `oep-cgr2step`: built for Linux
  aarch64 + Windows x86_64; packaged in the .deb/MSI; `--help` exits 0 on each
  (recorded in proof).
- `oep-update-check`: built both platforms; packaged; signature + refusal
  paths proven end-to-end.
- macOS: source-build documented; **no macOS artifact ships** (untested, no
  Mac available; stated on the download page).

## Unsigned-download statement (required by card)

> Downloads on the 2027.1.0 canary channel are NOT code-signed today.
> Windows SmartScreen will show a warning when the MSI is run. This is
> stated plainly on the download page; nothing hides it.

## GitHub Releases pointer

Once GITHUB-PRIMARY-MIGRATION lands, the update manifest source and download
page data switch to GitHub Releases URLs; the signed bundle format and the
checker are unchanged (proven against the canary store end to end on
2027-10-27). Until then all artifacts stay on the canary/prerelease channel
per the card gate.
