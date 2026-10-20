---
title: Isolith Export-Control Classification (ITAR/EAR release checkbox)
status: Ratified (Wave-3 P11-T05)
version: 1.0.0
date: 2026-10-18
owner: Isolith program governance, P11 security lane
applies_to: every released artifact of the Isolith / Open Engineering Platform (OEP) monorepo -- oep-pdm, oep-geo, oep-viewer, oep-web, the P2P compute lane, and the install/package bundles (P12-T04). Each release TRAIN (release/YYYY.N per P12-T05) and each candidate version (e.g. 2027.1.0-rc1) carries exactly one classification file.
supersedes: nothing (implements P0-M2-T04 recommendation 4 and the P11-T05 release pipeline checkbox)
---

# Isolith Export-Control Classification (ITAR/EAR Release Checkbox)

This record wires the export-control checkbox into the Isolith release
pipeline: **every release emits an ITAR/EAR classification file**, and the
release pipeline **will not proceed** (CI fails, the release-cut step refuses)
unless that file exists, states the release's classification, and asserts
data-locality plus the absence of ITAR/ECCN-restricted content. It builds
directly on the P0-M2-T04 export-control memo (`workstreams/governance/export-and-legal/export-control-itar-eccn.md`,
v1.0, 2026-10-15), which is the binding posture this checklist operationalizes.

The acceptance is exercised by a release drill: put the classification file in
place and the pipeline passes; remove it and the pipeline fails. Both
directions are machine-verified by the gate in `python/oep_tooling/
export_control_gate.py`, exercised by `tests/test_p11t05_export_control.py`,
and recorded in `workstreams/security/proof/`.

## 1. What the checkbox is

Per P0-M2-T04 section 3[G], each release ships an "export control
classification number / no-license-required" notice with the artifact, so
downstream users and CDNs get a durable classification statement instead of
guessing. Concretely:

- Every release version `<V>` (e.g. `2027.1.0-rc1`) must have a committed
  classification file at `workstreams/security/export-control/<V>.classification.toml`.
- The file is a **form, not a mystery** (P0-M2-T04 recommendation 4). The
  template is `workstreams/security/export-control/classification.template.toml`; each release
  copies it to the version-specific name and fills in the slots.
- The release pipeline (P12-T05 `release-cut` / `verify`, and the CI gates
  below) requires the file for the version being cut. **Absence fails the
  release** -- this is the checkbox: unchecked, the release cannot ship.

The Isolith release is an open-source tooling release. Its honest
classification under P0-M2-T04 is:

- **ITAR**: NOT subject. General-purpose CAD/PDM/geometry software is not a
  defense article and not ITAR technical data (22 CFR 120.33(a) only reaches
  software "directly related" to defense articles); as publicly released
  open source it is in the public domain under 22 CFR 120.34.
- **EAR**: NOT subject as published open-source software (15 CFR
  734.3(b)(3) + 734.7(a)); no ECCN is assigned (not on the CCL).
- **Encryption (5D002/5D992)**: open-source release under an open license
  falls under the published-software carve-out (15 CFR 734.7(b)); no closed
  binary, so the 5D992.c mass-market ENC analysis does not apply to this
  release type (P0-M2-T04 section 5.2).
- **734.7(c) firearm-CAM backstop**: no CAM/G-code or AMF artifact content is
  shipped in any current Isolith release, so the published-firearm
  carve-out exception does not trigger (P0-M2-T04 section 2.2).
- **Data locality**: the release bundles tool source code and the binaries
  built from it **only**. It contains no customer CAD models, no PMI/GD&T or
  drawing content, no geometry payloads, no telemetry payloads, and no
  rendered outputs. The cargo rule (P0-M2-T04 section 4, rule 1) means the
  classification of any customer's data rides on the article it describes --
  the release itself carries none.

If any field in the file flags otherwise (restricted content present, data
locality violated, or a non-UNCLASSIFIED status), the gate **fails the
release** and the release engineer must resolve the flag (or trigger the
P0-M2-T04 recommendation 6 escalation to counsel/agency) before shipping. A
release can only proceed with a clean UNCLASSIFIED declaration or an explicit,
reviewed escalation recorded in the file.

## 2. The classification file schema

Each file is TOML (machine-checked with the stdlib `tomllib`, no runtime
dependency) at `workstreams/security/export-control/<V>.classification.toml`:

```toml
release = "<V>"                 # exact version, e.g. 2027.1.0-rc1
train = "YYYY.N"                # release train this version belongs to
classification_date = "YYYY-MM-DD"

[classification]
status = "UNCLASSIFIED"         # the only value that passes the gate
summary = "General-engineering open-source software release: not a defense article, not ITAR technical data, not EAR-controlled."

[classification.itar]
subject = false                 # false = NOT a USML defense article / NOT ITAR technical data

[classification.ear]
subject = false                 # false = NOT subject to the EAR (published OSS, 15 CFR 734.3(b)(3)+734.7)
eccn = "NONE"                   # not on the CCL; NONE | EAR99 | <actual ECCN if ever flagged>
enc_note = "published-software carve-out (15 CFR 734.7(b)); no closed binary (5D992.c N/A)"
firearm_cam_backstop = "no CAM/G-code/AMF content in this release (15 CFR 734.7(c) N/A)"

[restricted_content]
contains_itar_technical_data = false
contains_eccn_controlled_content = false
contains_customer_data = false          # data-locality assertion
data_locality = "Release bundles tool source + built binaries only; no customer CAD/geometry/PMI/payload content."

[governance]
basis = "P0-M2-T04 export-control memo v1.0 (2026-10-15)"
prepared_by = "Isolith P11 security lane"
```

Forbidden to pass: `status != UNCLASSIFIED`, any `*.subject = true`, any
`restricted_content.* = true`, an empty `data_locality`, or a `release` field
that does not equal the version being released. These are the gate's hard
rules; the memo's nuance (customer-governed content, ITAR/EAR for defense
articles) is documented in P0-M2-T04 and carried by cargo-metadata in P8/P6,
not by the release file.

## 3. The gate and the release-pipeline rule

`python/oep_tooling/export_control_gate.py` is the enforcement point
(pure stdlib, hermetic). Subcommands:

    python3 python/oep_tooling/export_control_gate.py \
        verify --release <V> [--repo DIR]
        # exit 0 iff workstreams/security/export-control/<V>.classification.toml exists
        # AND validates (schema + UNCLASSIFIED + data-locality + no
        # restricted content). exit 1 otherwise -- MISSING FILE fails,
        # exactly the checkbox semantics.

    python3 python/oep_tooling/export_control_gate.py selfcheck [--repo DIR]
        # CI-safe: validates the template and every committed
        # *.<V>.classification.toml in workstreams/security/export-control/ without
        # requiring any particular release. Used on every push/PR.

`scripts/export-control/verify-classification.sh` is the bash wrapper the CI
and the release-cut step call (it also exposes `--selfcheck` for the
hermetic, no-specific-release mode). The release pipeline rule, inherited by
P12-T05's release-cut:

    A release TRAIN or candidate may not be cut or published while
    scripts/export-control/verify-classification.sh --release <V> exits
    non-zero for that version. The classification file for every shipped
    version MUST be committed before the freeze tag is pushed.

## 4. CI enforcement

`.gitea/workflows/ga-gate-p11-export-control.yaml` runs, on every push and
pull request:

1. **export-control-selfcheck** (all branches): parses the template and every
   committed classification file; any malformed or non-UNCLASSIFIED/
   restricted file fails CI. Combined with the P11-T02 SBOM/waivers gate this
   is the P11 lane's static release scan.
2. **export-control-release-required** (release/* branch line +
   `workflow_dispatch`): runs `verify --release <the train's candidate>` in
   the mode a release must pass; a push to `release/*` without the matching
   classification file is a hard CI failure, so **absence fails the release**
   before anything can be tagged or published from the release branch.

## 5. Release drill (acceptance)

Verified by `tests/test_p11t05_export_control.py`, which drives the actual
gate both ways on the real tree:

- **PASS direction (file present)**: with `workstreams/security/export-control/<V>.
  classification.toml` in place for the drill version, `verify --release <V>`
  exits 0 -- the release is cleared to proceed.
- **FAIL direction (file removed)**: the same directory with the
  classification file removed makes `verify --release <V>` exit non-zero with
  a "classification file missing -- release blocked" message.

The recorded live drill is in `workstreams/security/proof/export-control-drill-*.{json,md}`.

## 6. Versioning and change log

Bump the version in the front matter and append a line here on any material
change (program convention, mirrors vault-conventions).

- 1.0.0 (2026-10-18): first ratification. Classification file form, pure-stdlib
  gate, release-pipeline absence-fails rule, CI enforcement, and the two-way
  release drill, all building on P0-M2-T04 v1.0.
