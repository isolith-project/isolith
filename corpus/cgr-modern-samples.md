# Modern-era CGR-family verification material — NIST STC/FTC/CTC (V5-6R2019 / V5-6R2023)

P2-M7-T01 modern-era latch source evidence (2026-10-16). These are the
license-clean real files the modern decoder (`crates/oep-cgr/src/modern.rs`
and `python/oep_tooling/cgr_modern_reference.py`) is verified against.

## Licensing (dev-only reference material, never shipped; ITAR/privacy sweep clean)

- Source: NIST MBE PMI / CAx-IF STC-FTC-CTC CAD model archives.
- License: public domain (US Government work product; NIST disclaimer:
  "The test cases developed at the National Institute of Standards and
  Technology (NIST) are not subject to copyright protection and are in the
  public domain.")
- Download pages:
  - STC (CATIA V5-6R2023): https://www.nist.gov/document/nist-stc-pmi-v4
  - FTC/CTC (CATIA V5-6R2019): https://www.nist.gov/document/nist-ftc-ctc-pmi-cad-models
  - Overview: https://www.nist.gov/ctl/smart-connected-systems-division/smart-connected-manufacturing-systems-group/mbe-pmi-0
  - MBx-IF CAx resources (round listings): https://www.mbx-if.org/home/cax/resources/
- Posture: dev / observation only; binaries NOT committed to the shared repo
  (clean-room invariant). The repo records source + identity evidence only.

## Anchor file (the committed golden input)

- `nist_stc_06_asme1_V56R2023.CATPart` — CATIA V5-6R2023 native file with an
  embedded CGR-family visualization stream. golden row
  `cgr-modern-stc06-spatial-mesh-counts` resolves it via
  `OEP_CGR_SAMPLES` / corpus root.
- sha256: recompute at download; do not rely on the streamed copy.

## Measured anchor counts (2 independent readers agree, numeric-epsilon golden)

| field | value |
| --- | --- |
| genuine `0x4x` trim packets | 18 |
| visualization-point table | 1 |
| index_count (sum N) | 2003 |
| vertex_count (distinct) | 1037 |
| triangle_count | 1389 |
| max_index | 1990 |
