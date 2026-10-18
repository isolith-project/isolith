# P10-M5-T02 -- P10 GA gate: fresh install walkthrough verified on two machines

Run: 2026-10-18T13:49:47Z

**Summary:** total=6 PASS=6 FAIL=0 SKIP=0

## Rows

| leg | id | status | class | detail |
|-----|----|--------|-------|--------|
| DOCS | walkthrough-doc | PASS |  | checklist present and self-consistent (8 checklist rows, air-gap + online paths) |
| UPDATE | update-env | PASS |  | off by default: airgap template OEP_UPDATE_CHECK=off; install .env never enables; acceptance pinned; no phone-home (stack binds 127.0.0.1) |
| INSTALL | install-A | PASS |  | clean-state one-command install on A (localhost) -> pdm /api/v1/health 200 ok, viewer / 200, p2p /healthz 200 ok, oep-batch-convert + oep-render-worker present in image |
| INSTALL | install-B | PASS |  | clean-state one-command install on B (op@127.0.0.1) -> pdm /api/v1/health 200 ok, viewer / 200, p2p /healthz 200 ok, oep-batch-convert + oep-render-worker present in image |
| STACK | full-stack | PASS |  | pdm+viewer+p2p healthy (3), oep-batch-convert + oep-render-worker bins present in image |
| AIRGAP | airgap | PASS |  | signed bundle install on --network none target (routes=0): sig+sha256+manifest verified, docker load only, OEP_UPDATE_CHECK=off, post-install verify rc=0 (health green) |
