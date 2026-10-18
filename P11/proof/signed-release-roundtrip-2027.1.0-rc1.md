# P11-T04 signed-release round-trip proof (2027.1.0-rc1)

Recorded 2026-10-18 on U1 (promaxgb10-83f1, aarch64) as the P11-T04
acceptance evidence: a signed-release round trip on REAL P12-T04 packaging
output using the REAL build-host signing keys, verified with ONLY the
committed public keys.

Machine-readable facts in `signed-release-roundtrip-2027.1.0-rc1.json`.

## What was signed

The four P12-T04 release artifacts (oep-pdm / oep-web AppImages for aarch64,
oep-pdm / oep-update-check MSIs for x86_64-windows), each with:

- a detached armored gpg signature (`<artifact>.sig`) using the
  `oep-airgap-signing` key (secret on the build host, pubkey committed at
  `scripts/airgap/keys/oep-airgap-signing.pub.asc`),
- a sha-256 entry in `SHA256SUMS` (itself detached-signed),
- a listing in `RELEASE-MANIFEST.json` (the update bundle's inline payload),
- a `url` for an on-site mirror.

Plus `update.bundle.json`, the Ed25519-signed update manifest the P10
advisory checker consumes (decision #11): signature over the canonical
manifest, `payloads[0]` = the inline release manifest (verified by the
checker's inline-integrity gate), `payloads[1..]` = each artifact with its
sha256 / size / url. Signed with the `oep-update-signing` key (secret only
under `/home/r1a/keelworks-secrets/`, mode 600; pubkey committed at
`P11/keys/oep-update-signing.pub`).

## What was verified

With a fresh keyring holding ONLY the committed public keys:

- all four artifact signatures verify (PASS),
- all sha256 checksums match (PASS),
- oep-update-check accepts the bundle and reports an advisory update
  available (PASS),
- after flipping one byte in `oep-pdm-keelworks-oep-aarch64.AppImage`,
  verification FAILS (rc 1),
- after flipping one byte in `update.bundle.json`, verification FAILS
  (rc 1; the checker refuses to report it).

Full output of the fresh-keyring reverify:

    [ok] signature: keelworks-oep-pdm-x86_64-pc-windows-gnu-0.1.0.msi
    [ok] signature: keelworks-oep-update-check-x86_64-pc-windows-gnu-0.1.0.msi
    [ok] signature: oep-pdm-keelworks-oep-aarch64.AppImage
    [ok] signature: oep-web-keelworks-oep-aarch64.AppImage
    [ok] signature: SHA256SUMS
    [ok] sha256 checksums
    [ok] update.bundle.json signature verified by oep-update-check (advisory only)
    == VERIFY PASS (exit 0) ==

Re-verify at any time (no secret needed):

    bash scripts/release/verify-release.sh --assets P11/proof/signed-release-2027.1.0-rc1 \
        --version 2027.1.0-rc1
