# P12-T06 -- Install rehearsal for release 0.1.0

Run: 2026-10-19T10:16:30Z

**Summary:** total=4 PASS=4 FAIL=0 SKIP=0

## Rows

| leg | id | status | detail |
|-----|----|--------|--------|
| RELEASE | version | PASS | rehearsing release 0.1.0: install.sh + compose + .env.example all pin OEP_VERSION=0.1.0 |
| DOCS | runbook | PASS | runbook present, names 3 health surfaces + 2 approved host classes |
| INSTALL-A | install-A | PASS | clean-state one-command install on A (LOCAL) -> pdm /api/v1/health 200 ok, viewer / 200, p2p /healthz 200 ok, oep-batch-convert + oep-render-worker present in image |
| INSTALL-B | install-B | PASS | clean-state one-command install on B (op@127.0.0.1) -> pdm /api/v1/health 200 ok, viewer / 200, p2p /healthz 200 ok, oep-batch-convert + oep-render-worker present in image |

