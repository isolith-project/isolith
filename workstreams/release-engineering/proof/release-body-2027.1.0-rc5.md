# Isolith 2027.1.0-rc5 (release candidate — INTEROP AND PDM FOUNDATION TRAIN)

Train: 2027.1. Candidate: 2027.1.0-rc5. Target GA: 2027.1.0.
Frozen commit: `a9d1c0c` (tag `v2027.1.0-rc5`). Soak window: 3 hours from
2026-11-20T05:02Z (release-train policy 1.2.0, Don 2026-11-20).

## What 2027.1 is

The interop and PDM foundation train — the first Isolith release on the public
surface (GitHub Releases) carrying real installers for real people. It is
**not** the editor GA; the interactive editor reaches GA on a later train.
2027.1 ships the foundations the editor and PDM both stand on:

- CGR / 3DXML / STEP geometry interchange (readers + canonical oep-geo model +
  fidelity-tagged format matrix), with the interchange workbench command surface
  (`oep-workbench`) as the desktop product.
- A self-hosted PDM service (oep-pdm, REST API v1, secure-by-default RBAC), its
  web UI (oep-web), the P2P compute lane (oep-opsdash + workers), and the
  one-command on-prem stack.
- Advisory, signature-verified update checks (Ed25519, decision #11/#20).

## What 2027.1 does NOT do

Read the format matrix before trusting any interop claim
(docs-site/format-matrix.html in-tree). Every fidelity row is gate-tagged; rows
without a green GA report are marked not-scoped. In particular:

- STEP is a reader + writer with a documented fidelity budget; lossless
  round-trip of every AP242 file is **not** claimed.
- 3DXML and CGR read paths are proven against the committed corpus; exhaustiveness
  across every vendor dialect is **not** claimed.
- The editor shell is a command surface plus sketch-driven modelling and an
  interactive assembly editing surface, **not** CATIA/SolidWorks parity (that is
  the editor train; parity register in docs/scope-register.json).
- No macOS artifact ships in 2027.1 (source build documented, untested; no Mac
  available — stated, not hidden).
- The compose stack is the internal/customer-server distribution; the public
  download surface is the desktop installers on GitHub Releases. The NAS Gitea
  release page stays internal-only.

## Install paths

- Windows x86_64: MSI
  `isolith-oep-workbench-x86_64-pc-windows-gnu-2027.1.0.msi`, silent install:
  `msiexec /i <msi> /qn` (no start-menu shortcut in this train; launch
  `oep-workbench.exe` from `C:\Program Files (x86)\Isolith\bin\`). Downloads are
  NOT code-signed: Windows SmartScreen will warn. The MSI was verified on the
  garage PC (14/14 smoke checks).
- Linux aarch64 / arm64: AppImage `oep-workbench-isolith-oep-aarch64.AppImage`
  (chmod +x and run, or `--appimage-extract-and-run` on FUSE-less hosts) and
  `.deb` `isolith-oep-2027.1.0-arm64.deb`
  (`sudo apt install ./isolith-oep-2027.1.0-arm64.deb`; installs /usr/bin
  oep-workbench + CLI tools + update checker).

## Checksums and verification

Every asset has a gpg detached signature (key `oep-airgap-signing`, pubkey
committed at scripts/airgap/keys/oep-airgap-signing.pub.asc), a SHA256SUMS +
SHA256SUMS.sig, a RELEASE-MANIFEST.json, and an Ed25519-signed
update.bundle.json (key committed at workstreams/security/keys/oep-update-signing.pub).
Verify with scripts/release/verify-release.sh — PASS verified on this candidate.

Asset sha256 (authoritative set in attached SHA256SUMS):
```
oep-workbench-isolith-oep-aarch64.AppImage  66a0e75b079ba02af665ef2c4e9977eccb8a4a634416cb2a3dedc2d94eb60b3b
isolith-oep-2027.1.0-arm64.deb             2745e08fd6294ff25727f3b560f3722a4254d4a2c1d5d537f054741f31118ae8
isolith-oep-workbench-...-2027.1.0.msi     45ea439ee01a52a338c990c5fd2df68c6b16ac440d969be308f47f3a58bd5057
update.bundle.json                         153e5f5378d06500724da5d2906ac94650d873be61f353ae73792368743c14e9
```

## Verification results

- Full gate on the merged head (recorded verbatim in the release proof):
  pytest 555 passed / 29 skipped, cargo test --workspace 1146 passed / 0 failed,
  clippy RC 0, fmt RC 0, format-matrix RC 0, scope-register 23/0, wave verify
  RC 0, repo hygiene OK. (Product crates identical rc3→rc5; rc5's own tag lane
  is green across every release gate.)
- Reproducible build: 21 artifacts byte-identical across two clean-room builds
  (workstreams/security/proof/reproducible-build-2027.1.0-rc3.*); Linux binaries
  reproducible, installer wrappers not (timestamps/GUIDs) — recorded honestly.
- AppImage executed on U1: full editor workflow PASS (solve dof=0, volume
  32000→48000 live recompute, STEP export + reimport, renders differ).
- Windows MSI executed on the garage PC: 14/14 PASS (fetched from the GitHub
  store itself, hash vs checksums, silent install, ARP registered, full editor
  workflow on the installed binary, clean uninstall).
- Two-machine clean install rehearsal: 4/4 PASS on U1 + oep-vm-b (clean-state
  one-command install, 3 health checks + workers).
- Update bundle: clean bundle reports the signed payload; tampered payload and
  tampered signature are both refused (Untrusted).

## Export control

Isolith is export-controlled-compliant tooling under the documented internal
EAR99 self-classification (2026-10-27 gate answer); published open-source
software, not a defense article, not ITAR technical data, not EAR-controlled.
This release does NOT assert "ITAR certified" or "ITAR approved". Classification
of any customer data belongs to the customer's own accredited deployment; Isolith
bundles no customer CAD/geometry/PMI/payload content.

## Ceremony

rc5 is a GitHub pre-release (canary channel receives candidates before GA).
After the 3-hour soak with zero P0/P1 findings attributed to the candidate, the
same frozen commit a9d1c0c is re-tagged v2027.1.0 and promoted to GA (release
train policy 1.2.0: promotion changes the tag name, never the tree).
