# Keelworks Canary Repository (P12-T05)

Staged-rollout channel for the Keelworks / Open Engineering Platform (OEP)
release train, per the policy in `keelworks/keelworks` at
`P12/release-train.md`.

## Role

Every release candidate passes through this repo BEFORE general availability.
`keelworks/keelworks` is the product and GA channel; this repo is the canary
channel. Nothing is published to a GA channel without completing the canary
soak first.

## What the canary receives, per candidate

- the `canary/<version>` branch (a copy of the freeze point), and
- the `v<version>` tag (e.g. `v2027.1.0-rc1`), and
- a Gitea pre-release entry naming the candidate and its freeze manifest
  (`P12/freeze-manifests/<version>.json` in `keelworks/keelworks`), with
  binary artifacts attached once P12-T04 packaging exists.

## Promotion gate (P12-T05, section 5)

A candidate is promoted to GA only when ALL of:

1. canary soak of the full window (default 5 business days) with zero P0/P1
   findings attributed to the candidate;
2. CI green on the frozen commit;
3. acceptance suite green against the frozen tree on a run target;
4. release-manager sign-off.

This repo is machine-only for the release engineer's cut tool
(`python/oep_tooling/release_train.py` in `keelworks/keelworks`); no manual
feature branches live here.
