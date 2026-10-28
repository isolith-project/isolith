# OEP docs site (P13-T03)

The static documentation site for Open Engineering Platform: getting-started,
the admin/install guide, and the honest format capability matrix. It is served
by the P10 delivery stack itself via the `docs` compose service
(`oep-docs`, a zero-dependency std-only static server) on the same image as
the PDM, viewer and P2P services - "docs from the sum, no separate infra".

## Files

    index.html            landing + what the stack is
    getting-started.html  the eight-step verified install checklist + first use
    admin-guide.html      online + air-gap install, TLS, backup, DR, update posture
    install-on-nas.html   customer-NAS quickstart (pull the GHCR image, all health checks green)
    format-matrix.html    the honest format capability table (rendered)
    format-matrix.json    machine-checkable copy of the table + truth anchors
    assets/style.css      self-contained styling (no external assets, air-gap safe)
    assets/images/        real captures used on the landing page (drawing sheet,
                          mechanism animation + GIF + contact sheet, editor pair,
                          NAS compose-equivalent)
    assets/media-manifest.md  provenance and source of truth for every capture
    proof/                committed QA output (format-matrix-check.json)

## How it is served

`workstreams/delivery/deploy/deploy/docker-compose.yml` defines the `docs` service:

    docs  ->  oep-docs --root /srv/docs --addr 0.0.0.0:8792   (OEP_DOCS_PORT=8792)

The site is copied into the stack image under `/srv/docs` (`workstreams/delivery/deploy/deploy/Dockerfile`)
and served with a health surface `GET /healthz`. Host binds default to
127.0.0.1; an operator who wants `docs.openengineeringplatform.example` maps
that vhost to the docs port on their own network (admin/local decision).

## How it is verified

`scripts/format-matrix-check.py` is the QA gate: it validates the
table against a closed fidelity taxonomy and spot-checks the truth anchors
against the committed P2/P3/P4 GA matrices when those artifacts are on the
ref, classifying SKIP (never a fabricated PASS) when they are not. Exit 0
iff no FAIL row. Run it with the ground truth present:

    python3 scripts/format-matrix-check.py --site docs-site \
        --p4-matrix corpus/ga-gate-p4-matrix.json \
        --p4-pmi docs/pmi-fidelity/pmi-fidelity-matrix.md \
        --p2-report docs/re-notes/ga-gate.md \
        --p3-report docs/re-notes/3dxml-ga-gate.md

Serve locally without Docker for a quick status:

    cargo run -p oep-docs -- --root docs-site --addr 127.0.0.1:8792

Then `curl 127.0.0.1:8792/`, `/getting-started.html`, `/admin-guide.html`,
`/format-matrix.html`, and `/healthz`.
