# RELEASE-2027.1-GA -- release checklist record (rc3 -> GA, interop and PDM foundation train)

Card: t_99963f11. Train: 2027.1. GA version: 2027.1.0. Candidate: 2027.1.0-rc3.
Frozen commit: `f90d547fcf38990fc573057c8ab3c84f8801dd97` (tag `v2027.1.0-rc3`).
Policy: workstreams/release-engineering/release-train.md v1.2.0 (canary soak = 3 hours, Don 2026-11-20).

This record is committed with each item marked done and its evidence path. Items are
closed as their evidence is committed and verified.

| # | Item | Status | Evidence |
|---|------|--------|----------|
| 1 | Preconditions landed (GATE-PUBLIC-SURFACE, GITHUB-PRIMARY-MIGRATION, CROSS-PLATFORM-INSTALLERS, IDENTITY-CONFORMANCE, COMMUNITY-INTAKE) | DONE | All five parents complete; GitHub primary live at isolith-project/isolith; Gitea mirror read-only; rc1/rc2/desktop-1 evidence in-tree |
| 2 | rc3 cut from main: freeze manifest, release branch, tag | DONE | workstreams/release-engineering/freeze-manifests/2027.1.0-rc3.json; `release/2027.1` -> f90d547; tag `v2027.1.0-rc3` (f90d547, annotated) pushed to GitHub + Gitea mirror |
| 3 | Full gate green on merged head, recorded verbatim | DONE | workstreams/release-engineering/proof/gate-2027.1.0-rc3.md (pytest 555/29, cargo test --workspace 1146 passed/0 failed, clippy RC0, fmt RC0, format-matrix RC0, scope-register 23/0, wave verify RC0, repo hygiene OK) |
| 4 | Packaging reality check: AppImage built + executed on U1 (editor workflow) | DONE | oep-workbench-isolith-oep-aarch64.AppImage; full editor workflow on U1 rc3 binary: solve dof=0, volume 32000->48000 live recompute, STEP export+reimport, renders differ (proof log /tmp/rc3-editor-proof/trace.txt) |
| 5 | Packaging reality check: Windows MSI built + executed on garage PC | PENDING | isolith-oep-workbench-x86_64-pc-windows-gnu-2027.1.0.msi built (sha256 45ea439e...); garage-PC smoke to run (windows-release-smoke) |
| 6 | Docker stack install on a clean machine (two-machine rehearsal) | IN PROGRESS | scripts/install-rehearsal.sh on U1 (A) + oep-vm-b VM (B, op@127.0.0.1:2229) -> workstreams/release-engineering/proof/release-rehearsal-0.1.0.{json,md} |
| 7 | Two-machine clean install rehearsal from published bundle | PENDING (same run as #6) | rehearsal log committed to workstreams/release-engineering/proof/ |
| 8 | Reproducible build comparison | DONE | workstreams/security/proof/reproducible-build-2027.1.0-rc3.{json,md,SHA256SUMS}; 21 artifacts byte-identical across two clean-room builds |
| 9 | Export-control classification (EAR99 self-classification) | DONE | workstreams/security/export-control/2027.1.0-rc3.classification.toml; export_control_gate verify PASS; release body says export-controlled-compliant tooling, customer data classification belongs to customer's accredited deployment, never "ITAR certified/approved" |
| 10 | gpg-signed assets + SHA256SUMS + RELEASE-MANIFEST + Ed25519 update bundle | DONE | /tmp/rc3-signed (out dir for repeatable re-sign); verify-release.sh PASS (6 detached sigs + checksums + Ed25519 bundle TRUSTED); tamper-refusal proven: payload-hash tamper -> Untrusted, signature tamper -> Untrusted |
| 11 | Canary pre-release published (GitHub Releases, candidates before GA) | PENDING | GitHub pre-release v2027.1.0-rc3 with signed assets + release body; asset count recorded |
| 12 | Canary soak (3 hours on the frozen commit, no feature commits on release branch) | PENDING | start = pre-release publish time; elapsed check at GA |
| 13 | GA promotion of the identical frozen commit (same sha, tag v2027.1.0) | PENDING | GitHub release v2027.1.0 @ f90d547; forward tag to Gitea mirror |
| 14 | Release body for a skeptical reader | PENDING | release body text committed under workstreams/release-engineering/proof/release-body-2027.1.0-rc3.md and used on the canary/GA release |
| 15 | Committed release checklist record | THIS FILE | workstreams/release-engineering/release-checklist-2027.1.0.md |
| 16 | Downloads page + update manifest source point at GitHub Releases | PENDING | docs-site/downloads.json + update bundle URL -> https://github.com/isolith-project/isolith/releases/download/v2027.1.0-rc3/ |

## Version / freeze facts (verbatim)

```
version             2027.1.0-rc3
train               2027.1
GA version          2027.1.0
frozen_commits_full 246
frozen_first_parent 129
cut_tree            272f58787612905612d1ba6a72242110f6d9529d
tag                 v2027.1.0-rc3 (annotated) = f90d547 (pushed, verified)
release branch      release/2027.1 = f90d547 (pushed, verified; Gitea mirror read-only)
```

## Gate per release train policy

- one freeze manifest per train: workstreams/release-engineering/freeze-manifests/2027.1.0-rc3.json (DONE)
- canary receives candidates before GA: GitHub pre-release v2027.1.0-rc3
- GA promotion of the identical frozen commit: v2027.1.0 == f90d547 (same sha as rc3 tag)
- main remains the integration head: main = bd24d82 (untouched by the release branch)
