# Keelworks P1-M1-T06: Privacy / ITAR Sweep of the Sample Corpus

Version: 1.1 (2026-10-16) — full-corpus re-run for the P1-M4-T01 object-store
mirror (edit 2026-10-16: added §Follow-up P1-M4-T01, evidence
`workstreams/format-corpus/samples/privacy-itar-sweep-report-full-2026-10-16.json`).
Owner: P1 corpus pipeline (P1-M1-T06)
Status: GATE PASS on the 2026-10-15 corpus picture. This note records the
sweep's method, the rules, and the per-sample result. It is evidence, not
legal advice; export-control posture is owned by P0-M2-T04 and confirmed by
counsel before the public-release gate (P11-T05).

## What this sweeps

Every file that is destined for the Keelworks shared repo from the public
sample corpus, plus the CGR binaries as a reference-only pass (those are held
outside the repo because their licenses do not grant redistribution; see
P1-M1-T01).

Scope and result (verified on real bytes, sha256-anchored):

| Group | Count | Verdict |
|---|---|---|
| Repo-destined corpus files (CATPart/CATProduct, 3DXML, STL, README) | 18 | CLEAN |
| HIGH findings (ITAR/USML/ECCN, classified/CUI, personal data) | 0 | none |
| MEDIUM/LOW findings | 0 | none |
| CGR binaries (reference-only, never committed) | 11 | CLEAN |
| Sweep gate | - | PASS |

Machine-readable evidence: `workstreams/format-corpus/samples/privacy-itar-sweep-report.json` (this
directory). Each record carries the file's relative path, size, sha256, and any
findings.

## The rule set

A finding is raised by one of three indicator families. Anything of HIGH
severity without a disposition note blocks the gate: a sample may not enter
the shared repo until a worker records a reviewed disposition for it.

1. Export-control markers (ITAR/USML/ECCN):
   `itar`, `usml`, `eccn`, `ear99`, `munitions list`, `defense article`,
   `specially designed`, `export controlled`, `ccats`, `commodity
   jurisdiction`, `ddtc`, `600-series`, `proscribed`, `22 CFR <num>`.

2. Classified / controlled-unclassified markers:
   `TOP SECRET`, `SECRET//`, `CONFIDENTIAL//`, `CONTROLLED UNCLASSIFIED`,
   `FOUO`, `NOFORN`, `NO FOREIGN`, `//REL`, `FOR OFFICIAL USE ONLY`, and the
   short token `CUI`.

3. Personal / privacy data:
   email addresses (TLD of 2+ letters), US Social Security numbers, and US
   phone numbers.

## False-positive discipline (learned on the 2026-10-15 corpus)

Binary CAD files contain tessellation and floating-point data that, read as
bytes, can spell trigram tokens or digit patterns. The detector is calibrated
so that noise is not treated as a finding:

- The short token `CUI` and phone-number patterns are evaluated only in text
  and UTF-16 metadata runs, never in raw ASCII runs pulled from binary
  geometry. Random float renderings spell `cUI` in isolated places; they must
  not gate a sample.
- The email rule requires a letter TLD, so version strings like `1@1.0` or
  `6@s.0` are not flagged.
- Word boundaries are ASCII-only. Python's default `\b` is Unicode-aware, so a
  wide glyph (CJK) rendered from binary noise would otherwise block a
  legitimate match; the rules use `(?<![A-Za-z0-9_])...(?![A-Za-z0-9_])`
  instead, so a token embedded in a longer ASCII word (`GITAR`) is missed but a
  real `ITAR` adjacent to a wide glyph is still caught.
- 3DXML archives are detected by content (they are ZIP containers), not by
  file extension: an uppercase `.3DXML` is scanned member-by-member (XML
  members as text, tessellation members conservatively on their ASCII/UTF-16
  runs). This fixes the naive first pass, which treated `.3DXML` zips as plain
  text and produced a false CUI hit on `FM1_satellite.3DXML`.

## What is NOT re-examined here

- Registry and catalog documents (`workstreams/format-corpus/samples/*.md`, `corpus/*.json`) are authored
  by Keelworks workers and intentionally discuss licenses, including words like
  `ITAR`, `restricted`, and `proprietary` as redistribution commentary. They
  are the corpus bookkeeping, not sample payload; their integrity is guarded by
  the corpus manifest task (P1-M1-T04), not by this content sweep.
- The licensed 3DXML spec (XSD + user guide) obtained under Dassault's free
  internal-use license is held out of repo by license. See
  `workstreams/format-corpus/samples/3dxml-spec-process.md`.

## How to re-run

    PYTHONPATH=python python3 -m oep_tooling.privacy_itar_sweep \
        --corpus corpus/data \
        --reference <scratch-cgr-dir>... \
        --out workstreams/format-corpus/samples/privacy-itar-sweep-report.json

The acceptance test (`tests/test_privacy_itar_sweep.py`) covers: tool present
and API-complete; detector sensitivity on planted ITAR/CUI/email/UTF-16
markers; detector specificity on float noise, version strings, and benign CAD
tokens; the committed report covering every repo-destined sample by matching
sha256 with no unexplained HIGH finding; and a live sweep that activates
automatically once the corpus is present in-tree.

## Follow-up P1-M4-T01 (full-corpus sweep, kept on materialization)

When the object-store mirror materialized the full shared corpus (all 122
backend=object-store blobs: 121 upstream MIT CATPart/CATProduct + the box.stl
fixture mirror), the sweep was re-run over every full-corpus byte — the 121
upstream files plus the repo-destined `corpus/data` set — on 2026-10-16,
sha256-anchored to the manifest so the evidence ties directly to the mirror.
Result (see `workstreams/format-corpus/samples/privacy-itar-sweep-report-full-2026-10-16.json`):

| Group | Count | Verdict |
|---|---|---|
| Full corpus swept (121 upstream CATPart/CATProduct + repo corpus/data) | 139 | CLEAN |
| HIGH findings (ITAR/USML/ECCN, classified/CUI, personal data) | 0 | none |
| MEDIUM/LOW findings | 0 | none |
| Sweep gate | - | PASS |

The licensing posture (Don, 2026-10-16) keeps these as reference-only
development material — never shipped — and the clean redistribution boundary
applies only to the released artifact; the privacy/ITAR sweep stays the only
hard line, and this re-run confirms the materialized mirror clears it.

## Out of scope for this card

Wiring the sweep into CI as a hard gate per corpus change is a release-pipeline
decision (P11-T05 / pre-commit pipeline, P0-M3/P0-M4 series), not a corpus
content task. This card ships the tool, the evidence, and the acceptance test;
the release gate consumes them.
