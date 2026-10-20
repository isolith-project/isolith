# P13-T03 docs-site proof (measured, 2026-10-18)

Total=6 PASS=6 FAIL=0 SKIP=0 (schema_version 1)

| leg | status | detail |
|-----|--------|--------|
| sections-from-repo | PASS | oep-docs serving docs-site/: /, getting-started, admin-guide, format-matrix, format-matrix.json, assets/style.css all HTTP 200 with section markers; /healthz 200 `{"status":"ok"}`; missing path 404; traversal rejected 404 |
| sections-from-image | PASS | the same three sections + /healthz served from the BUILT delivery image `keelworks/oep-stack:0.1.0-p13t03` (docker build of workstreams/delivery/deploy/deploy/Dockerfile); `oep-docs` at /usr/local/bin/oep-docs, site at /srv/docs - docs from the sum, no separate infra |
| format-matrix-check | PASS | `scripts/format-matrix-check.py` against committed P2(P2-M6-T03)/P3(P3-M5-T02)/P4(P4-M5-T03 matrix + P4-M2-T03 PMI fidelity) ground truth: 8/8 PASS fail=0 exit=0 (full report: format-matrix-check.json) |
| acceptance-tests | PASS | pytest docs-site suite 4/4; full repo pytest 111 passed; `cargo test --workspace` exit 0 (oep-docs 6/6); rustfmt clean; clippy --workspace --all-targets clean |
| compose-valid | PASS | `docker compose config`: four services (docs/pdm/viewer/p2p) on the one stack image; docs OEP_DOCS_PORT=8792 + healthcheck `GET /healthz` |
| p10-ga-surface-unchanged | PASS | the P10-M5 GA-gate three-service health contract is preserved in install.sh; docs is an additive fourth service |

## How to reproduce

    # 1. render + serve from the repo (no docker)
    cargo run -p oep-docs -- --root docs-site --addr 127.0.0.1:8792
    curl -fsS http://127.0.0.1:8792/          # landing
    curl -fsS http://127.0.0.1:8792/getting-started.html
    curl -fsS http://127.0.0.1:8792/admin-guide.html
    curl -fsS http://127.0.0.1:8792/format-matrix.html
    curl -fsS http://127.0.0.1:8792/healthz   # {"status":"ok"}

    # 2. spot-check the matrix against the committed P2/P3/P4 GA matrices
    python3 scripts/format-matrix-check.py --site docs-site \
        --p4-matrix workstreams/docs-site/proof/ground-truth/ga-gate-p4-matrix.json \
        --p4-pmi    workstreams/docs-site/proof/ground-truth/pmi-fidelity-matrix.md \
        --p2-report workstreams/docs-site/proof/ground-truth/ga-gate.md \
        --p3-report workstreams/docs-site/proof/ground-truth/ga-gate-3dxml.md

    # 3. serve from the built delivery image (docs from the sum)
    docker build -f workstreams/delivery/deploy/deploy/Dockerfile -t keelworks/oep-stack:0.1.0 .
    docker run --rm -p 127.0.0.1:8792:8792 keelworks/oep-stack:0.1.0 \
        oep-docs --root /srv/docs --addr 0.0.0.0:8792
