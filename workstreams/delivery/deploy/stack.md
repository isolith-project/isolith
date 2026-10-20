# P10-M1-T01: One-command Docker-compose stack (PDM + P2P + viewer)

Status: delivered (2026-10-17). Owner: P10 hybrid-cloud delivery lane.
Card: `t_a1035554` (P10-M1-T01 on the Keelworks board).

## What this delivers

A single `install.sh` that, from a clean state, brings the whole Keelworks
integrated stack up on an approved U1/VM-class host with all health checks
green. The stack is three services on one image (one build, three entry
points), exactly the three named deliverables:

| service | binary     | port (container) | health surface              | what it is                                  |
|---------|------------|------------------|-----------------------------|----------------------------------------------|
| `pdm`   | `oep-pdm`  | 8787             | `GET /api/v1/health`        | P8 self-hosted PDM (vault, revisions, RBAC, audit, REST API v1) |
| `viewer`| `oep-web`  | 8790             | `GET /` (explorer, 200)     | P6 preview viewer: item explorer, BOM tree, preview embed, compare |
| `p2p`   | `oep-opsdash` | 8080          | `GET /healthz`              | P9 P2P compute lane: queue depth, node health, job logs, cost |

The `viewer` service (`oep-web`) is the P8-M4-T01 web UI extended with the P6
preview embed. It had been built on an older P8 lineage and was not present on
the P9 GA-gate assembly the P10 wave stacks on; **this card integrates it**
(additive) together with the P8-M2-T03 relationship graph it depends on. See
the integration section below.

## Run target (decision #9)

Approved run targets are U1 (primary) and the garage PC (WoL-required); the
NAS is repo/storage host only and must not host Keelworks services. This card
is verified on U1 (`promaxgb10-83f1`, 192.168.0.67). "Fresh install verified
on two machines" (plan wording) means two U1/VM-class machines — covered by
the P10-M5-T02 GA gate card, which walks a clean operator through the same
install on two machines.

## One-command install

Prereqs on the target: Docker Engine 24+, Docker Compose v2, ~5GB free disk.

```bash
cd workstreams/delivery/deploy/deploy
./install.sh
```

What it does, then verifies:

1. Preflight: docker present+running, compose v2, disk, `.env` from
   `.env.example` (host binds default to `127.0.0.1` — on-prem posture).
2. Builds the single stack image (`keelworks/oep-stack`) from the repo, or
   `docker load`s an offline bundle when `--offline-bundle <tar.gz>` is given.
3. `docker compose up -d` (services `pdm`, `viewer`, `p2p`, volumes
   `pdm_data`, `viewer_data`).
4. Waits until all three compose healthchecks are `healthy` (timeout 180s).
5. Curls each health surface directly and prints the operator summary.

Exit code 0 iff all three surfaces answer green.

```bash
./install.sh --offline-bundle keelworks-oep-stack-0.1.0.tar.gz  # air-gap
./install.sh --no-build                                          # reuse image
./install.sh --down                                              # stop
```

## Offline-image pattern discipline (Bastion S2-equivalent)

The offline/air-gap path follows the same shape as Bastion S2:
build -> `docker save` -> gzip tarball + `sha256sum`, then load on a host
with no network. The P10-M2-T02 downstream card signs and ships this bundle.

```bash
cd workstreams/delivery/deploy/deploy
./release.sh 0.1.0 --tarball --out ../releases
# -> ../releases/keelworks-oep-stack-0.1.0.tar.gz + .sha256
```

`install.sh --offline-bundle <tar.gz>` verifies the `.sha256` (when present)
then `docker load`s the image and starts the stack with `--no-build` — no
network required on the target, same health checks as the online install.
No component phones home (decision #11; the stack ships no reach-back).

## Health-check contract (the GA surface)

The P10-M5-T02 GA gate uses exactly these three green signals on two machines:

```
curl -fsS http://127.0.0.1:8787/api/v1/health   # -> {"status":"ok","service":"oep-pdm",...}
curl -fsS -o /dev/null http://127.0.0.1:8790/    # -> 200 (explorer HTML)
curl -fsS http://127.0.0.1:8080/healthz          # -> {"status":"ok"}
```

## Viewer integration (why crates moved)

`oep-web` (P8-M4-T01) is the browser face of the PDM and renders real
`oep-geo` models through the P6 `oep-viewer` embed. It was authored on an
older P8 relationship-graph lineage and was not on the P9 GA-gate assembly.
This card adds, additively on the P9 base (`task/p9m5t02-ga-gate-1017a`):

- `crates/oep-web` — the viewer service (explorer / item / BOM / preview /
  compare routes merged with the oep-pdm REST v1 router).
- `crates/oep-pdm/src/graph.rs` — P8-M2-T03 relationship graph
  (`RelationshipGraph::from_oepgeo_document`, where-used, impact) restored
  onto the current oep-pdm store API, plus the `oep_pdm` re-exports
  (`ItemKind`, `RelationshipGraph`, `WhereUsedReport`, ...).
- `oep-pdm` gained an `oep-geo` dependency (graph ingest) and the
  `two-part-fixture.oepgeo.json` golden now lives under
  `crates/oep-pdm/tests/golden/` (self-contained; not the oep-assem copy).
- workspace `Cargo.toml` member `crates/oep-web`.

Deliberately NOT pulled in from the web-ui branch: `crates/oep-assem` and the
vendored solvespace solver (LGPL-tier licensing + build surface, out of scope
for the delivery stack — assembly-solver work stays its own lane).

Verified: `cargo test --workspace --offline` all green, including the five
web-UI acceptance tests (explorer, BOM walk, viewer embed renders real
models, REST-alongside-UI, compare) and the full P9/P8 suites.

## Layout

```
workstreams/delivery/deploy/
  stack.md                  this document
  deploy/
    Dockerfile              multi-stage Rust build -> slim runtime (one image)
    docker-compose.yml      services pdm/viewer/p2p + healthchecks + volumes
    install.sh              one-command install (Bastion S2 load-or-build)
    release.sh              build + optional offline tarball + sha256
    .env.example            host binds/ports, image tag
  proof/                    the measured install + health proof (this run)
```
