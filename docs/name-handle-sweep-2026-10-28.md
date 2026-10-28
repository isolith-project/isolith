# Name and handle sweep inventory (2026-10-28)

Machine-complete output for the NAME-AND-ASSET-RESERVATIONS sweep 
(t_a15e108f). Covers identifiers that change once the GitHub handle and 
domain exist. The authoritative mapping is assets/brand/NAME.md v1.2 
(public identity and asset map). Availability evidence: 
assets/brand/evidence/name-handle-availability-2026-10-28.json.

## How to read this

- IMAGE = image reference `isolith/oep-stack[:tag]` -> `ghcr.io/<handle>/oep-stack[:tag]`
- REPO-REF = Gitea repo/clone reference (`192.168.0.101`, `http://gitea`, `isolith/isolith`)
-  -> public GitHub clone URL `https://github.com/<handle>/isolith.git`, except where the
  tool talks to Gitea's API (keep internal). Tests that assert the Gitea repo path stay
  until GITHUB-PRIMARY-MIGRATION flips the source of truth; then they move too.
- SITE-DOMAIN placeholders (CODE_OF_CONDUCT/SECURITY/GOVERNANCE) are owned by card 
  SITE-DOMAIN-ORIGIN (t_ad10a5f5), not this card.

96 files carry a changeable identifier today.

| File | Identifier(s) | Class |
|---|---|---|
| `.gitea/workflows/changelog.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ci.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/crypto-integrity.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/dco.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-3dxml.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p10.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p11-export-control.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p11.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p12.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p12e2e.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p4.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p8.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/ga-gate-p9.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/kernel-seam.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/licensing-boundary.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/precommit-gate.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/release-changelog.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/supply-chain.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `.gitea/workflows/tls-and-backup.yaml` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `Cargo.toml` | `192.168.0.101:3200/don/isolith` | REPO-REF |
| `crates/oep-docs/src/lib.rs` | `isolith/oep-stack` | IMAGE |
| `crates/oep-job/src/lib.rs` | `192.168.0.101:3200/isolith/isolith` | REPO-REF |
| `crates/oep-sandbox/src/lib.rs` | `192.168.0.101:3200/isolith/isolith` | REPO-REF |
| `crates/oep-state/src/record.rs` | `isolith/isolith` | REPO-REF |
| `docs-site/admin-guide.html` | `http://gitea/isolith/isolith.git` | REPO-REF |
| `docs-site/getting-started.html` | `http://gitea/isolith/isolith.git/isolith/oep-stack:0.1.0` | IMAGE/REPO-REF |
| `docs/oep-meta-archive-note.md` | `isolith/isolith` | REPO-REF |
| `docs/re-notes/3dxml-ga-gate.md` | `isolith/isolith` | REPO-REF |
| `docs/re-notes/compute-ga-gate.md` | `isolith/isolith` | REPO-REF |
| `docs/re-notes/ga-gate.md` | `isolith/isolith` | REPO-REF |
| `docs/re-notes/release-train.md` | `isolith/isolith` | REPO-REF |
| `docs/re-notes/step-mbd-ga-gate.md` | `isolith/isolith` | REPO-REF |
| `docs/status/report.json` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `docs/status/status.html` | `192.168.0.101:3200/isolith/isolith.git` | REPO-REF |
| `python/oep_tooling/corpus/store.py` | `192.168.0.101:3200` | REPO-REF |
| `python/oep_tooling/e2e_vertical.py` | `isolith/oep-stack/isolith/oep-stack:/isolith/oep-stack:0.1.0` | IMAGE |
| `python/oep_tooling/ga_gate_p10.py` | `isolith/oep-stack:0.1.0` | IMAGE |
| `python/oep_tooling/ga_gate_p8.py` | `192.168.0.101:3200/isolith/isolith` | REPO-REF |
| `python/oep_tooling/objectstore.py` | `192.168.0.101:3200` | REPO-REF |
| `python/oep_tooling/release_train.py` | `192.168.0.101%3a3200),` | REPO-REF |
| `python/oep_tooling/vault_blobs.py` | `192.168.0.101:3200` | REPO-REF |
| `python/oep_tooling/wave_checkpoint.py` | `192.168.0.101:3200/api/v1/isolith/isolith` | REPO-REF |
| `scripts/airgap-proof.sh` | `192.168.0.101:3200/isolith/isolith` | REPO-REF |
| `scripts/backup/install-vault-backup-cron.sh` | `192.168.0.101*)` | REPO-REF |
| `scripts/backup/vault-backup.sh` | `192.168.0.101*)` | REPO-REF |
| `scripts/install-rehearsal.sh` | `isolith/oep-stack:$VERSION` | IMAGE |
| `scripts/release/attach-release-notes.py` | `isolith/isolith` | REPO-REF |
| `tests/ci_pipeline_test.py` | `192.168.0.101:3200/api/v1/isolith/isolith` | REPO-REF |
| `tests/gitea_layout_test.py` | `192.168.0.101/192.168.0.101:3200)/192.168.0.101:3200/api/v1` | REPO-REF |
| `tests/test_p12release_train.py` | `isolith/isolith` | REPO-REF |
| `tests/test_p8m5t03_hybrid_cloud_role.py` | `192.168.0.101:3200/api/v1/repos/isolith/isolith` | REPO-REF |
| `tests/test_wave_integration_policy.py` | `isolith/isolith` | REPO-REF |
| `workstreams/delivery/airgap/proof/airgap-proof.json` | `192.168.0.101:3200` | REPO-REF |
| `workstreams/delivery/deploy/deploy/Dockerfile` | `isolith/oep-stack` | IMAGE |
| `workstreams/delivery/deploy/deploy/docker-compose.yml` | `isolith/oep-stack:${OEP_VERSION:-0.1.0}` | IMAGE |
| `workstreams/delivery/deploy/deploy/install.sh` | `isolith/oep-stack:${TAG}/isolith/oep-stack:${TAG}.` | IMAGE |
| `workstreams/delivery/deploy/deploy/release.sh` | `isolith/oep-stack:${VERSION}/isolith/oep-stack:latest/isolith/oep-stack:latest.` | IMAGE |
| `workstreams/delivery/deploy/proof/compose-stack-proof.json` | `192.168.0.101:3200/isolith/isolith/pulls/151/isolith/oep-stack:0.1.0` | IMAGE/REPO-REF |
| `workstreams/delivery/deploy/proof/offline-install.log` | `isolith/oep-stack:0.1.0/isolith/oep-stack:0.1.0.` | IMAGE |
| `workstreams/delivery/deploy/proof/online-install.log` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/delivery/deploy/stack.md` | `isolith/oep-stack` | IMAGE |
| `workstreams/delivery/update-checker/install-walkthrough.md` | `isolith/isolith/isolith/oep-stack:0.1.0/isolith/oep-stack:0.1.0`` | IMAGE/REPO-REF |
| `workstreams/delivery/update-checker/proof/video/machine-a-online.typescript` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/delivery/update-checker/proof/video/machine-b-online-sliced.typescript` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/delivery/update-checker/proof/video/machine-b-online.typescript` | `isolith/oep-stack/isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/docs-site/docs-site.md` | `isolith/oep-stack` | IMAGE |
| `workstreams/docs-site/proof/docs-site-proof.json` | `isolith/oep-stack/isolith/oep-stack:0.1.0-p13t03` | IMAGE |
| `workstreams/docs-site/proof/docs-site-proof.md` | `isolith/oep-stack:0.1.0/isolith/oep-stack:0.1.0-p13t03`` | IMAGE |
| `workstreams/docs-site/proof/ground-truth/README.md` | `isolith/isolith` | REPO-REF |
| `workstreams/docs-site/proof/ground-truth/ga-gate-3dxml.md` | `isolith/isolith` | REPO-REF |
| `workstreams/docs-site/proof/ground-truth/ga-gate.md` | `isolith/isolith` | REPO-REF |
| `workstreams/modeler/shell/open-modeler-shell-sources.md` | `isolith/isolith` | REPO-REF |
| `workstreams/pdm/api/proof/ga-gate-p8-multiuser.json` | `192.168.0.101:3200` | REPO-REF |
| `workstreams/pdm/service/working-set-protocol.md` | `isolith/isolith` | REPO-REF |
| `workstreams/release-engineering/install-rehearsal.md` | `isolith/oep-stack:/isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/release-engineering/packaging.md` | `isolith/oep-stack` | IMAGE |
| `workstreams/release-engineering/proof/dry-run-cut-2027.1.0-rc1.json` | `isolith/isolith` | REPO-REF |
| `workstreams/release-engineering/proof/dry-run-cut-2027.1.0-rc1.md` | `isolith/isolith` | REPO-REF |
| `workstreams/release-engineering/proof/e2e-vertical-0.1.0.json` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/release-engineering/proof/e2e-vertical-0.1.0.md` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/release-engineering/proof/e2e-vertical-break-0.1.0.json` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/release-engineering/proof/e2e-vertical-break-0.1.0.md` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/release-engineering/proof/packaging-proof.json` | `isolith/oep-stack` | IMAGE |
| `workstreams/release-engineering/proof/packaging-proof.md` | `isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/release-engineering/release-train.md` | `isolith/isolith` | REPO-REF |
| `workstreams/release-engineering/wave-integration-and-checkpoint-policy.md` | `isolith/isolith` | REPO-REF |
| `workstreams/security/artifacts/0.1.0/scan-report.json` | `isolith/oep-stack/isolith/oep-stack:apt/isolith/oep-stack:bash/isolith/oep-stack:bsdutils/isolith/oep-stack:coreutils/isolith/oep-stack:curl/isolith/oep-stack:dash/isolith/oep-stack:diffutils/isolith/oep-stack:gcc-12-base/isolith/oep-stack:gpgv/isolith/oep-stack:gzip/isolith/oep-stack:libacl1/isolith/oep-stack:libapt-pkg6.0/isolith/oep-stack:libattr1/isolith/oep-stack:libblkid1/isolith/oep-stack:libbz2-1.0/isolith/oep-stack:libc-bin/isolith/oep-stack:libc6/isolith/oep-stack:libcurl4/isolith/oep-stack:libgcc-s1/isolith/oep-stack:libgcrypt20/isolith/oep-stack:libgnutls30/isolith/oep-stack:libgssapi-krb5-2/isolith/oep-stack:libk5crypto3/isolith/oep-stack:libkrb5-3/isolith/oep-stack:libkrb5support0/isolith/oep-stack:libldap-2.5-0/isolith/oep-stack:libmount1/isolith/oep-stack:libnghttp2-14/isolith/oep-stack:libp11-kit0/isolith/oep-stack:libpam-modules/isolith/oep-stack:libpam-modules-bin/isolith/oep-stack:libpam-runtime/isolith/oep-stack:libpam0g/isolith/oep-stack:libsasl2-2/isolith/oep-stack:libsasl2-modules-db/isolith/oep-stack:libsmartcols1/isolith/oep-stack:libssl3/isolith/oep-stack:libstdc++6/isolith/oep-stack:libsystemd0/isolith/oep-stack:libtasn1-6/isolith/oep-stack:libtinfo6/isolith/oep-stack:libudev1/isolith/oep-stack:libuuid1/isolith/oep-stack:login/isolith/oep-stack:mount/isolith/oep-stack:ncurses-base/isolith/oep-stack:ncurses-bin/isolith/oep-stack:openssl/isolith/oep-stack:passwd/isolith/oep-stack:perl-base/isolith/oep-stack:sysvinit-utils/isolith/oep-stack:tar/isolith/oep-stack:util-linux/isolith/oep-stack:util-linux-extra/isolith/oep-stack:zlib1g` | IMAGE |
| `workstreams/security/artifacts/0.1.0/scan-report.md` | `isolith/oep-stack:apt/isolith/oep-stack:bash/isolith/oep-stack:bsdutils/isolith/oep-stack:coreutils/isolith/oep-stack:curl/isolith/oep-stack:dash/isolith/oep-stack:diffutils/isolith/oep-stack:gcc-12-base/isolith/oep-stack:gpgv/isolith/oep-stack:gzip/isolith/oep-stack:libacl1/isolith/oep-stack:libapt-pkg6.0/isolith/oep-stack:libattr1/isolith/oep-stack:libblkid1/isolith/oep-stack:libbz2-1.0/isolith/oep-stack:libc-bin/isolith/oep-stack:libc6/isolith/oep-stack:libcurl4/isolith/oep-stack:libgcc-s1/isolith/oep-stack:libgcrypt20/isolith/oep-stack:libgnutls30/isolith/oep-stack:libgssapi-krb5-2/isolith/oep-stack:libk5crypto3/isolith/oep-stack:libkrb5-3/isolith/oep-stack:libkrb5support0/isolith/oep-stack:libldap-2.5-0/isolith/oep-stack:libmount1/isolith/oep-stack:libnghttp2-14/isolith/oep-stack:libp11-kit0/isolith/oep-stack:libpam-modules/isolith/oep-stack:libpam-modules-bin/isolith/oep-stack:libpam-runtime/isolith/oep-stack:libpam0g/isolith/oep-stack:libsasl2-2/isolith/oep-stack:libsasl2-modules-db/isolith/oep-stack:libsmartcols1/isolith/oep-stack:libssl3/isolith/oep-stack:libstdc++6/isolith/oep-stack:libsystemd0/isolith/oep-stack:libtasn1-6/isolith/oep-stack:libtinfo6/isolith/oep-stack:libudev1/isolith/oep-stack:libuuid1/isolith/oep-stack:login/isolith/oep-stack:mount/isolith/oep-stack:ncurses-base/isolith/oep-stack:ncurses-bin/isolith/oep-stack:openssl/isolith/oep-stack:passwd/isolith/oep-stack:perl-base/isolith/oep-stack:sysvinit-utils/isolith/oep-stack:tar/isolith/oep-stack:util-linux/isolith/oep-stack:util-linux-extra/isolith/oep-stack:zlib1g` | IMAGE |
| `workstreams/security/pentest-2026-10-18.md` | `isolith/oep-stack:0.1.0`` | IMAGE |
| `workstreams/security/proof/sbom-scan-0.1.0.json` | `isolith/oep-stack/isolith/oep-stack:apt/isolith/oep-stack:bash/isolith/oep-stack:bsdutils/isolith/oep-stack:coreutils/isolith/oep-stack:curl/isolith/oep-stack:dash/isolith/oep-stack:diffutils/isolith/oep-stack:gcc-12-base/isolith/oep-stack:gpgv/isolith/oep-stack:gzip/isolith/oep-stack:libacl1/isolith/oep-stack:libapt-pkg6.0/isolith/oep-stack:libattr1/isolith/oep-stack:libblkid1/isolith/oep-stack:libbz2-1.0/isolith/oep-stack:libc-bin/isolith/oep-stack:libc6/isolith/oep-stack:libcurl4/isolith/oep-stack:libgcc-s1/isolith/oep-stack:libgcrypt20/isolith/oep-stack:libgnutls30/isolith/oep-stack:libgssapi-krb5-2/isolith/oep-stack:libk5crypto3/isolith/oep-stack:libkrb5-3/isolith/oep-stack:libkrb5support0/isolith/oep-stack:libldap-2.5-0/isolith/oep-stack:libmount1/isolith/oep-stack:libnghttp2-14/isolith/oep-stack:libp11-kit0/isolith/oep-stack:libpam-modules/isolith/oep-stack:libpam-modules-bin/isolith/oep-stack:libpam-runtime/isolith/oep-stack:libpam0g/isolith/oep-stack:libsasl2-2/isolith/oep-stack:libsasl2-modules-db/isolith/oep-stack:libsmartcols1/isolith/oep-stack:libssl3/isolith/oep-stack:libstdc++6/isolith/oep-stack:libsystemd0/isolith/oep-stack:libtasn1-6/isolith/oep-stack:libtinfo6/isolith/oep-stack:libudev1/isolith/oep-stack:libuuid1/isolith/oep-stack:login/isolith/oep-stack:mount/isolith/oep-stack:ncurses-base/isolith/oep-stack:ncurses-bin/isolith/oep-stack:openssl/isolith/oep-stack:passwd/isolith/oep-stack:perl-base/isolith/oep-stack:sysvinit-utils/isolith/oep-stack:tar/isolith/oep-stack:util-linux/isolith/oep-stack:util-linux-extra/isolith/oep-stack:zlib1g` | IMAGE |
| `workstreams/security/proof/sbom-scan-0.1.0.md` | `isolith/oep-stack:apt/isolith/oep-stack:bash/isolith/oep-stack:bsdutils/isolith/oep-stack:coreutils/isolith/oep-stack:curl/isolith/oep-stack:dash/isolith/oep-stack:diffutils/isolith/oep-stack:gcc-12-base/isolith/oep-stack:gpgv/isolith/oep-stack:gzip/isolith/oep-stack:libacl1/isolith/oep-stack:libapt-pkg6.0/isolith/oep-stack:libattr1/isolith/oep-stack:libblkid1/isolith/oep-stack:libbz2-1.0/isolith/oep-stack:libc-bin/isolith/oep-stack:libc6/isolith/oep-stack:libcurl4/isolith/oep-stack:libgcc-s1/isolith/oep-stack:libgcrypt20/isolith/oep-stack:libgnutls30/isolith/oep-stack:libgssapi-krb5-2/isolith/oep-stack:libk5crypto3/isolith/oep-stack:libkrb5-3/isolith/oep-stack:libkrb5support0/isolith/oep-stack:libldap-2.5-0/isolith/oep-stack:libmount1/isolith/oep-stack:libnghttp2-14/isolith/oep-stack:libp11-kit0/isolith/oep-stack:libpam-modules/isolith/oep-stack:libpam-modules-bin/isolith/oep-stack:libpam-runtime/isolith/oep-stack:libpam0g/isolith/oep-stack:libsasl2-2/isolith/oep-stack:libsasl2-modules-db/isolith/oep-stack:libsmartcols1/isolith/oep-stack:libssl3/isolith/oep-stack:libstdc++6/isolith/oep-stack:libsystemd0/isolith/oep-stack:libtasn1-6/isolith/oep-stack:libtinfo6/isolith/oep-stack:libudev1/isolith/oep-stack:libuuid1/isolith/oep-stack:login/isolith/oep-stack:mount/isolith/oep-stack:ncurses-base/isolith/oep-stack:ncurses-bin/isolith/oep-stack:openssl/isolith/oep-stack:passwd/isolith/oep-stack:perl-base/isolith/oep-stack:sysvinit-utils/isolith/oep-stack:tar/isolith/oep-stack:util-linux/isolith/oep-stack:util-linux-extra/isolith/oep-stack:zlib1g` | IMAGE |
| `workstreams/security/proof/trivy-image-findings.json` | `isolith/oep-stack/isolith/oep-stack:apt/isolith/oep-stack:bash/isolith/oep-stack:bsdutils/isolith/oep-stack:coreutils/isolith/oep-stack:curl/isolith/oep-stack:dash/isolith/oep-stack:diffutils/isolith/oep-stack:gcc-12-base/isolith/oep-stack:gpgv/isolith/oep-stack:gzip/isolith/oep-stack:libacl1/isolith/oep-stack:libapt-pkg6.0/isolith/oep-stack:libattr1/isolith/oep-stack:libblkid1/isolith/oep-stack:libbz2-1.0/isolith/oep-stack:libc-bin/isolith/oep-stack:libc6/isolith/oep-stack:libcurl4/isolith/oep-stack:libgcc-s1/isolith/oep-stack:libgcrypt20/isolith/oep-stack:libgnutls30/isolith/oep-stack:libgssapi-krb5-2/isolith/oep-stack:libk5crypto3/isolith/oep-stack:libkrb5-3/isolith/oep-stack:libkrb5support0/isolith/oep-stack:libldap-2.5-0/isolith/oep-stack:libmount1/isolith/oep-stack:libnghttp2-14/isolith/oep-stack:libp11-kit0/isolith/oep-stack:libpam-modules/isolith/oep-stack:libpam-modules-bin/isolith/oep-stack:libpam-runtime/isolith/oep-stack:libpam0g/isolith/oep-stack:libsasl2-2/isolith/oep-stack:libsasl2-modules-db/isolith/oep-stack:libsmartcols1/isolith/oep-stack:libssl3/isolith/oep-stack:libstdc++6/isolith/oep-stack:libsystemd0/isolith/oep-stack:libtasn1-6/isolith/oep-stack:libtinfo6/isolith/oep-stack:libudev1/isolith/oep-stack:libuuid1/isolith/oep-stack:login/isolith/oep-stack:mount/isolith/oep-stack:ncurses-base/isolith/oep-stack:ncurses-bin/isolith/oep-stack:openssl/isolith/oep-stack:passwd/isolith/oep-stack:perl-base/isolith/oep-stack:sysvinit-utils/isolith/oep-stack:tar/isolith/oep-stack:util-linux/isolith/oep-stack:util-linux-extra/isolith/oep-stack:zlib1g` | IMAGE |
| `workstreams/security/proof/trivy-image.cdx.json` | `isolith/oep-stack/isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/security/proof/trivy-image.json` | `isolith/oep-stack/isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/security/sbom-cve.md` | `isolith/oep-stack:/isolith/oep-stack:0.1.0` | IMAGE |
| `workstreams/security/waivers.toml` | `isolith/oep-stack/isolith/oep-stack:0.1.0/isolith/oep-stack:bsdutils/isolith/oep-stack:curl/isolith/oep-stack:gzip/isolith/oep-stack:libacl1/isolith/oep-stack:libblkid1/isolith/oep-stack:libcurl4/isolith/oep-stack:libldap-2.5-0/isolith/oep-stack:libmount1/isolith/oep-stack:libsmartcols1/isolith/oep-stack:libssl3/isolith/oep-stack:libsystemd0/isolith/oep-stack:libtinfo6/isolith/oep-stack:libudev1/isolith/oep-stack:libuuid1/isolith/oep-stack:mount/isolith/oep-stack:ncurses-base/isolith/oep-stack:ncurses-bin/isolith/oep-stack:openssl/isolith/oep-stack:perl-base/isolith/oep-stack:util-linux/isolith/oep-stack:util-linux-extra/isolith/oep-stack:zlib1g` | IMAGE |

## Policy notes (recorded, pending Don)

1. crates.io 0.0.0 placeholder crate: only if Don approves squatting a throwaway
   package; requires a crates.io token (none on this host). Otherwise no reservation,
   crates keep the `oep-` prefix.
2. npm `isolith` claim with placeholder README: needs an npm account/token; gated on
   the handle + Don's account.
3. PyPI `isolith`: held only if a Python package will ship; otherwise not squatted.
4. GitHub account/org: Don creates it (gate); this card only checks availability.
