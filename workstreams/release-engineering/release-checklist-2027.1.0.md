# RELEASE-2027.1-GA -- release checklist record (rc3 -> GA, interop and PDM foundation train)

Card: t_99963f11. Train: 2027.1. GA version: 2027.1.0. Candidate: 2027.1.0-rc5 (the candidate whose lane is green and that is soaking since 2026-11-20T05:02Z on `a9d1c0c`; rc3 `f90d547` was superseded after its lane returned reds, rc4 `d690cfc` after one red).
Candidate frozen commit: `a9d1c0c` (tag `v2027.1.0-rc5`). rc5 freeze manifest committed.
Policy: workstreams/release-engineering/release-train.md v1.2.0 (canary soak = 3 hours, Don 2026-11-20); soak ends 2026-11-20T08:02Z.
Note: product crates are byte-identical across rc3/rc4/rc5 (only CI/harness fixes landed between cuts), so the verified rc3-built binaries were re-signed with the rc5 identity.

This record is committed with each item marked done and its evidence path. Items are
closed as their evidence is committed and verified.

| # | Item | Status | Evidence |
|---|------|--------|----------|
| 1 | Preconditions landed (GATE-PUBLIC-SURFACE, GITHUB-PRIMARY-MIGRATION, CROSS-PLATFORM-INSTALLERS, IDENTITY-CONFORMANCE, COMMUNITY-INTAKE) | DONE | All five parents complete; GitHub primary live at isolith-project/isolith; Gitea mirror read-only; rc1/rc2/desktop-1 evidence in-tree |
| 2 | rc3 cut from main: freeze manifest, release branch, tag | DONE | workstreams/release-engineering/freeze-manifests/2027.1.0-rc3.json; `release/2027.1` -> f90d547; tag `v2027.1.0-rc3` (f90d547, annotated) pushed to GitHub + Gitea mirror |
| 3 | Full gate green on merged head, recorded verbatim | DONE | workstreams/release-engineering/proof/gate-2027.1.0-rc3.md (pytest 555/29, cargo test --workspace 1146 passed/0 failed, clippy RC0, fmt RC0, format-matrix RC0, scope-register 23/0, wave verify RC0, repo hygiene OK) |
| 4 | Packaging reality check: AppImage built + executed on U1 (editor workflow) | DONE | oep-workbench-isolith-oep-aarch64.AppImage; full editor workflow on U1 rc3 binary: solve dof=0, volume 32000->48000 live recompute, STEP export+reimport, renders differ (proof log workstreams/release-engineering/proof/appimage-editor-rc3.log) |
| 5 | Packaging reality check: Windows MSI built + executed on garage PC | DONE | isolith-oep-workbench-x86_64-pc-windows-gnu-2027.1.0.msi (sha256 45ea439e...); 14/14 smoke checks PASS on garage PC (192.168.0.43): fetched MSI from GitHub store itself, box hash == release checksum, silent install exit 0, ARP registered, installed binary full editor workflow, clean uninstall + ARP clean (workstreams/release-engineering/proof/windows-smoke-rc3.log) |
| 6 | Docker stack install on a clean machine (two-machine rehearsal) | DONE | scripts/install-rehearsal.sh on U1 (A) + oep-vm-b VM (B, op@127.0.0.1:2229) -> workstreams/release-engineering/proof/release-rehearsal-0.1.0.{json,md}: total=4 PASS=4 FAIL=0 SKIP=0, exit 0 |
| 7 | Two-machine clean install rehearsal from published bundle | DONE | 4/4 PASS above (both hosts clean-state one-command install, 3 health checks + workers) |
| 8 | Reproducible build comparison | DONE | workstreams/security/proof/reproducible-build-2027.1.0-rc3.{json,md,SHA256SUMS}; 21 artifacts byte-identical across two clean-room builds |
| 9 | Export-control classification (EAR99 self-classification) | DONE | workstreams/security/export-control/2027.1.0-rc5.classification.toml; export_control_gate verify PASS; release body says export-controlled-compliant tooling, customer data classification belongs to customer's accredited deployment, never "ITAR certified/approved" |
| 10 | gpg-signed assets + SHA256SUMS + RELEASE-MANIFEST + Ed25519 update bundle | DONE | proof/signed-release-2027.1.0-rc5/ (re-signed with rc5 identity); verify-release.sh PASS; tamper-refusal both paths -> Untrusted |
| 11 | Canary pre-release published (GitHub Releases, candidates before GA) | DONE | GitHub pre-release v2027.1.0-rc5 @ a9d1c0c; 15 signed assets attached (5 artifacts + sigs + SHA256SUMS + RELEASE-MANIFEST + update.bundle.json); readback verified via API. rc3/rc4 pre-releases not created (lane superseded) |
| 12 | Canary soak (3 hours on the frozen commit, no feature commits on release branch) | DONE | soak 2026-11-20T05:02Z -> 08:02Z on a9d1c0c; zero P0/P1 findings; release branch had 0 commits during the window |
| 13 | GA promotion of the identical frozen commit (same sha, tag v2027.1.0) | DONE | tag v2027.1.0 @ a9d1c0c (same sha as rc5) created + pushed to GitHub and Gitea mirror; GA release created with 15 signed assets (byte-identical binaries to the canary — hashes match); readback verified |
| 14 | Release body for a skeptical reader | DONE | proof/release-body-2027.1.0-rc5.md; GA body attached to the v2027.1.0 release (what 2027.1 is/is not, install paths, checksums, rehearsal result, export-control wording) |
| 15 | Committed release checklist record | THIS FILE | workstreams/release-engineering/release-checklist-2027.1.0.md |
| 16 | Downloads page + update manifest source point at GitHub Releases | DONE (rc5 identity) | docs-site/downloads.json base_url -> github.com/isolith-project/isolith/releases/download/...; update bundle URL flips to v2027.1.0 at GA |

## Version / freeze facts (verbatim)

```
version             2027.1.0-rc5
train               2027.1
GA version          2027.1.0
frozen_commits_full 253
frozen_first_parent 133
cut_tree            cd979d9b3f1d80ec741579bc59adc9bb41123406
tag                 v2027.1.0-rc5 (annotated) = a9d1c0c (pushed to GitHub, verified)
release branch      release/2027.1 (protected; GA tag v2027.1.0 goes on the same sha a9d1c0c)
rc3 (superseded)    f90d547 (freeze manifest 2027.1.0-rc3.json; lane returned reds)
rc4 (superseded)    d690cfc (one red: P10 demo unauth vault boot; fixed in rc5)
```

## Gate per release train policy

- one freeze manifest per train: workstreams/release-engineering/freeze-manifests/2027.1.0-rc5.json (DONE, committed)
- canary receives candidates before GA: GitHub pre-release v2027.1.0-rc5, soak 2026-11-20T05:02Z -> 08:02Z (3h, policy 1.2.0)
- GA promotion of the identical frozen commit: GA tag v2027.1.0 == a9d1c0c (same sha as rc5 tag), after the soak with zero P0/P1
- main remains the integration head: main = a9d1c0c (release branch/tag point at the frozen commit; no feature commits during soak)
