# ASME Y14.41 — Keelworks Use

## Why Keelworks cares

ASME Y14.41 is the *business practice* definition of Model-Based Definition: what
makes a digital model legally and practically "product definition" instead of just
geometry. Keelworks' strongest wedge (per the master plan) is the P4-M5 MBD
validator plus honest capability reporting. That validator is only meaningful if its
rule set encodes Y14.41 expectations. Y14.41 also anchors the PMI model in P6 and the
MBD fidelity claims in P4.

## Workstream mapping

- **P4-M5-T01 — MBD validator service**: check a supplied model/drawing against a
  rule set "(Y14.41, prime requirements)": missing PMI, dangling datums, units. This
  card literally names Y14.41.
- **P4-M5-T02 — MBD validator CLI + report**: the wedge product surface; its rule
  engine is Y14.41-informed.
- **P4-M2-T03 — PMI fidelity audit runner**: matrix of GD&T types x apps reported
  honestly; Y14.41 defines what "good PMI presentation" is.
- **P6-M1-T02 — PMI model schema (GD&T types, datums, notes)**: importable from P4;
  Y14.41 semantics shape the schema fields.
- **P2/P3 (CGR/3DXML readers)** — where the source models live before MBD
  validation; Y14.41 conformance checks operate on the extracted model+PMI.

## What we take from the standard (observational, license-safe)

- **Data set taxonomies** (model-only vs model-plus-drawing), **annotation
  presentation requirements**, **model coordinate/datum framing**: these become
  validation rules, expressed in our own words — standard text is not copied.
- **Exception/additional-requirements pattern** ("where no exception... existing
  ASME standards shall apply"): Keelworks' validator outputs can mirror this
  structure (base rules + exceptions) as a familiar audit format.
- Secondary public sources (Action Engineering MBE Dictionary term definitions
  citing Y14.41) give snippet-level definitions we can cite under fair use.

## Risks / notes

- **Paywalled + ASME copyright**: do not copy the standard body into tests,
  docs, or validator rule files. Encode *rules* in our own words; if a test needs a
  normative reference, cite section numbers/links rather than quoting text into code.
- Y14.41 works hand-in-hand with Y14.5 (GD&T) and Y14.47 (3D data interchange, used
  by MIL-STD-31000); the rule set should reference the whole family, each paywalled,
  each handled as a catalog pointer.
- Never present validator output as "certified Y14.41 compliance" — that is a
  certification claim; we report conformance findings honestly and let the customer's
  quality authority decide certification.
