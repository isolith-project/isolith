# Isolith 2027.1.0 — release notes (for the skeptical reader)

Card: t_99963f11. Train: 2027.1. Candidate cut: 2027.1.0-rc3. GA: 2027.1.0.
Frozen commit: `f90d547fcf38990fc573057c8ab3c84f8801dd97` (tag `v2027.1.0-rc3` == GA `v2027.1.0`, same sha).
Host: U1 (promaxgb10-83f1), aarch64.

## What 2027.1 is

The interop and PDM foundation train. It is the first Isolith release on the
public surface (GitHub Releases), carrying real installers for real people. It is
the P12/Q4-train release of the year-2027 line. It is **not** the editor GA: the
interactive editor reaches GA on a later train; 2027.1 ships the foundations the
editor and PDM both stand on:

- CGR / 3DXML / STEP geometry interchange (readers + canonical oep-geo model +
  fidelity-tagged format matrix), with the interchange workbench command surface
  (`oep-workbench`) as the desktop product.
- A self-hosted PDM service (oep-pdm, REST API v1, secure-by-default RBAC), its
  web UI (oep-web), the P2P compute lane (oep-opsdash + workers), and the
  one-command on-prem stack.
- Advisory, signature-verified update checks (Ed25519, decision #11/#20).

## What 2027.1 does NOT do

Read the format matrix before trusting any interop claim:
[docs-site/format-matrix.html](https://isolith-project.github.io/isolith/docs-site/format-matrix.html)
(in-tree: docs-site/format-matrix.json + format-matrix.html). Every fidelity row
is gate-tagged; rows without a green GA report are marked SKIP/not-scoped, never
overstated. In particular:

- STEP is a reader + writer with a documented fidelity budget (see the P4 GA
  gate); lossless round-trip of every possible AP242 file is **not** claimed.
- 3DXML and CGR read paths are proven against the committed corpus; exhaustiveness
  across every vendor dialect is **not** claimed.
- The editor shell is a command surface plus sketch-driven modelling and an
  interactive assembly editing surface, **not** the full CATIA/SolidWorks parity
  editor (that is the editor train; the parity register is
  docs/scope-register.json).
- No macOS artifact ships in 2027.1 (source build documented, untested; no Mac
  was available - stated, not hidden).
- The compose stack is the internal/customer-server distribution; the public
  download surface is the desktop installers on GitHub Releases. The NAS Gitea
  release page stays internal-only.

## Install paths

- Windows x86_64: MSI
  `isolith-oep-workbench-x86_64-pc-windows-gnu-2027.1.0.msi`, silent install:
  `msiexec /i <msi> /qn` (no start-menu shortcut in this train; launch
  `oep-workbench.exe` from `C:\Program Files (x86)\Isolith\bin\`). Downloads are
  NOT code-signed: Windows SmartScreen will warn. The MSI is verified on the
  garage PC (see below).
- Linux aarch64 / arm64: AppImage
  `oep-workbench-isolith-oep-aarch64.AppImage` (chmod +x, run, or
  `--appimage-extract-and-run` on FUSE-less hosts) and `.deb`
  `isolith-oep-2027.1.0-arm64.deb` (`sudo apt install ./isolith-oep-...deb`;
  installs `/usr/bin/oep-workbench` + CLI tools + update checker).
- Linux sandboxed: Flatpak bundle (built + installed in-tree; sandboxed run
  untested on a desktop host, stated).
- macOS: no artifact; see workstreams/release-engineering/packaging/macos-cli.md
  for the source build.

## Checksums and verification

SHA256SUMS plus gpg detached signatures for every asset, a RELEASE-MANIFEST.json,
and an Ed25519-signed update.bundle.json are attached. Verify with
scripts/release/verify-release.sh (needs only the committed public keys). The
gpg signing key is `oep-airgap-signing` (public key committed at
scripts/airgap/keys/oep-airgap-signing.pub.asc); the Ed25519 key is committed at
workstreams/security/keys/oep-update-signing.pub and baked into public builds.

Asset sha256 (rc3 seed; final GA asset set re-hashed at promotion):

```
oep-workbench-isolith-oep-aarch64.AppImage  66a0e75b079ba02af665ef2c4e9977eccb8a4a634416cb2a3dedc2d94eb60b3b
isolith-oep-2027.1.0-arm64.deb             2745e08fd6294ff25727f3b560f3722a4254d4a2c1d5d537f054741f31118ae8
isolith-oep-workbench-...-2027.1.0.msi     45ea439ee01a52a338c990c5fd2df68c6b16ac440d969be308f47f3a58bd5057
(see attached SHA256SUMS for the authoritative set)
```

## Repair / verification results

- **Gate, on the merged head (bd24d82)**: pytest 555 passed / 29 skipped,
  cargo test --workspace 1146 passed / 0 failed, clippy -D warnings RC 0,
  fmt RC 0, format-matrix RC 0, scope-register 23/0, wave checkpoint verify
  RC 0, repository hygiene OK. Recorded verbatim in
  workstreams/release-engineering/proof/gate-2027.1.0-rc3.md.
- **Reproducible build**: two clean-room release builds, 21 artifacts
  byte-identical (workstreams/security/proof/reproducible-build-2027.1.0-rc3.*).
  Linux binaries reproducible; the .deb/MSI/AppImage wrappers are NOT
  byte-identical across builds (embedded timestamps / GUIDs) - recorded honestly.
- **AppImage executed on U1**: full editor workflow PASS (solve dof=0, width
  variable, volume 32000 -> 48000 live recompute, STEP export + reimport,
  before/after renders differ).
- **Windows MSI executed on the garage PC**: see the smoke record
  (workstreams/release-engineering/proof/desktop-2027.1.0/windows-smoke-full.log
  lineage; rc3 rerun recorded in this release's proof).
- **Two-machine clean install rehearsal**: one-command stack install from a
  clean state on U1 (A) and the oep-vm-b VM (B), 3 health checks + worker
  binaries green (workstreams/release-engineering/proof/release-rehearsal-0.1.0.*).
- **Update bundle**: clean bundle reports UpdateAvailable/UpToDate; tampered
  payload and tampered signature are both refused as Untrusted.

## Export control

Isolith is export-controlled-compliant tooling under the documented internal
EAR99 self-classification (2026-10-27 gate answer). It is published open-source
software, not a defense article, not ITAR technical data, not EAR-controlled.
This release does NOT assert "ITAR certified" or "ITAR approved". Classification
of any customer data belongs to the customer's own accredited deployment; Isolith
bundles no customer CAD/geometry/PMI/payload content.

## Canary / GA ceremony

v2027.1.0-rc3 was published as a GitHub pre-release (canary channel receives
candidates before GA). After the 3-hour soak with zero P0/P1 findings attributed
to the candidate, the SAME frozen commit f90d547 is re-tagged v2027.1.0 and the
release is promoted to GA (release-train policy v1.2.0: promotion changes the tag
name, never the tree).
