# Keelworks P12-T06 - Release Install Rehearsal (fresh clean machine per release)

Version: 1.0.0 (2026-10-18)

Status: active release-engineering routine (P12). Every time a release is about
to ship, run `scripts/install-rehearsal.sh` on the two approved hosts and only
release when the recorded report is green. This is the Keelworks form of the
r1a-release-workflow install rehearsal.

Owner: P12 QA / CI-CD / release engineering lane.
Card: P12-T06 on the Keelworks board.
Ties to: decision #9 (run targets: U1 + a second U1/VM-class host; NAS stays
repo/storage only), P10-M1-T01 (the one-command install being rehearsed),
P10-M5-T02 (the two-machine fresh-install walkthrough + GA gate proof this
routine reuses), P12-T05 (release train: each train ships with a rehearsal
record at P12/proof/release-rehearsal-<ver>.{json,md}).

## 1. What this rehearses

Before a release ships you must prove the one-command install works on a fresh
clean machine - catching packaging bugs while the release is still a branch,
not after it went live. The install being rehearsed is the P10 one-command
stack: `P10-M1/deploy/install.sh` from a clean state brings up three services
with three health checks green:

| service | binary       | container        | host port | health surface               |
|---------|--------------|------------------|-----------|------------------------------|
| PDM     | oep-pdm      | keelworks-pdm    | 8787      | GET /api/v1/health           |
| Viewer  | oep-web      | keelworks-viewer | 8790      | GET /  (explorer HTML, 200)  |
| P2P     | oep-opsdash  | keelworks-p2p   | 8080      | GET /healthz                 |

The worker binaries (oep-batch-convert, oep-render-worker) must be present in
the installed image, and the stack binds to 127.0.0.1 (nothing exposed to the
LAN, decision #10). "Two approved hosts" is decision #9: machine A = U1, machine
B = a U1/VM-class host (the keelwb2 QEMU/KVM VM on U1, or the garage PC over
WoL). The NAS is repo/storage only and is never a run target.

## 2. Run the rehearsal

On U1, from the repo checkout at the release candidate head:

```bash
./scripts/install-rehearsal.sh --version 0.1.0
```

Defaults (safe to rely on):

- version: read from `P10-M1/deploy/.env.example` `OEP_VERSION` when `--version`
  is omitted;
- machine B: `op@127.0.0.1:2229` (the keelwb2 VM on U1). Override with
  `--machine-b-host` / `--machine-b-port` for any other approved second host.

What the script does, top to bottom:

1. Preflights docker, asserts the RC head has no unmerged conflict markers.
2. Records the RELEASE leg: the requested version matches
   `P10-M1/deploy/.env.example` and the compose stack image tag (`oep-stack`
   default `OEP_VERSION`), so the rehearsal exercises the exact release being
   shipped.
3. Ships the release tree to machine B verbatim (rsync, the documented
   physical-transfer path).
4. Machine A = U1: tears down all `keelworks-` containers, `pdm_data` /
   `viewer_data` volumes, the `keelworks/oep-stack:<ver>` image and the
   generated `P10-M1/deploy/.env` (the clean state), then runs the one-command
   `./install.sh`.
5. Asserts on A: PDM `/api/v1/health` returns 200 + `{"status":"ok"}`, Viewer
   `/` returns 200, P2P `/healthz` returns 200 + `{"status":"ok"}`,
   `oep-batch-convert --help` and `oep-render-worker --describe` exit 0.
6. Machine B (same clean state, same install, same assertions over SSH).
7. Writes `P12/proof/release-rehearsal-<ver>.json` + `.md` with the measured
   rows and the summary.

Exit-code contract (same as the P8/P9/P10 gates): exit 0 iff the recorded
report has ZERO FAIL rows. A non-zero exit means do not ship.

## 3. The checklist (what an operator would run by hand)

For a truly manual rehearsal, on each clean machine, in order:

| # | Step | Command | Expected |
|---|------|---------|----------|
| 1 | Clean state confirmed | `docker ps --filter name=keelworks- --format '{{.Names}}'` | (empty - no keelworks containers) |
| 2 | Install (from the deploy dir) | `cd P10-M1/deploy && ./install.sh` | `All 3 services healthy.` and exit 0 |
| 3 | PDM health | `curl -fsS http://127.0.0.1:8787/api/v1/health` | `{"status":"ok",...}` |
| 4 | Viewer health | `curl -fsS -o /dev/null -w '%{http_code}' http://127.0.0.1:8790/` | `200` |
| 5 | P2P health | `curl -fsS http://127.0.0.1:8080/healthz` | `{"status":"ok"}` |
| 6 | Convert worker shipped | `docker run --rm --entrypoint oep-batch-convert keelworks/oep-stack:0.1.0 --help` | usage text, exit 0 |
| 7 | Render worker shipped | `docker run --rm --entrypoint oep-render-worker keelworks/oep-stack:0.1.0 --describe` | JSON descriptor, exit 0 |

Run rows 1-7 on machine A and again on machine B. Both machines must come up
green; one recorded verification is not enough for a release rehearsal.

## 4. The record (per release)

`scripts/install-rehearsal.sh` writes the measured record to:

```
P12/proof/release-rehearsal-<ver>.json   # machine-parseable, exit_code field
P12/proof/release-rehearsal-<ver>.md     # human-readable gate report
```

`<ver>` is the release's calendar-train version (P0-M5-T03 / release-train
decision; e.g. `0.1.0` for the current stack, `2027.1` once trains start). The
recorded rows:

| leg | id | what is measured |
|-----|----|------------------|
| RELEASE | version | requested version == `.env.example` OEP_VERSION; compose pins it |
| DOCS | runbook | `P12/install-rehearsal.md` exists, names the 3 health surfaces + 2 host classes |
| INSTALL-A | install-A | clean-state one-command install on U1 -> 3 health checks green + workers |
| INSTALL-B | install-B | clean-state one-command install on machine B -> 3 health checks green + workers |

The exit-code contract means downstream automation (a release pipeline or a
reviewer) can gate on `exit_code == 0` / `summary.FAIL == 0` in the JSON.

## 5. When to run it

- Before every GA cut (online path, two machines).
- Before a patch train (P12-T05 patch policy) ships, on the patch head.
- After any change to `P10-M1/deploy/` (install.sh, docker-compose.yml,
  Dockerfile, .env.example) - install-affecting changes invalidate the last
  rehearsal.

CI-safe mode: `./scripts/install-rehearsal.sh --selfcheck` runs the RELEASE and
DOCS legs only (no docker, no second host) and classifies the machine legs SKIP
under the closed taxonomy - used by the Gitea ga-gate-p12 job and by the
acceptance test `tests/test_p12t06_install_rehearsal.py`, exactly how the
P8/P9/P10 gates degrade in CI.

## 6. Version history

- 1.0.0 (2026-10-18): initial P12 release-rehearsal runbook; two-machine
  one-command install rehearsal, version-parameterized, exit-code contract,
  proof under P12/proof/. First rehearsal record: release-rehearsal-0.1.0.
