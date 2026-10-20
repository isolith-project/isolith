# E2E scenario suite (reference vertical: import -> model -> release PDM -> render/convert P2P -> share)

Task: P12-T09 | version 0.1.0 | run 2026-10-19T00:32:35Z

**Summary:** total=8 PASS=7 FAIL=1 SKIP=0 exit=1

| id | leg | status | class | detail |
|----|-----|--------|-------|--------|
| stack-boot | BOOT | PASS |  | packaged stack image isolith/oep-stack:0.1.0 (oep-web) on http://127.0.0.1:38095 |
| import-step | IMPORT | PASS |  | STEP item item-0-04f32c384cb5 rev1 blob sha-verified and re-read byte-identical |
| import-cgr | IMPORT | PASS |  | CGR item item-1-fdb4dcf99343 rev1 seeded and blob-sha verified |
| model-geo | MODEL | PASS |  | oep-geo model item item-2-694d50495b0d rev1 format oep-geo, sha-verified |
| model-convert | MODEL | PASS |  | step2mesh convert ok=1 hash_verified=1 (mesh item item-0-04f32c384cb5-mesh) |
| release | RELEASE | FAIL | deliberate-break | deliberate-break: PDM refused the release (corrupt token) |
| render | RENDER | PASS |  | oep-render-worker produced a real PNG (1229438 bytes) |
| share | SHARE | PASS |  | model item-2-694d50495b0d shareable: explorer 200, item page 200, preview PNG drawn=3254 |

