# CGR Version Drift: R18-Era vs R2026-Era Comparison

P1-M2-T02 · Keelworks / OEP P1 Format Reconnaissance
Status: verified on real material 2026-10-15 (old-era leg); modern-era leg
recorded as blocked-on-sample.
Version: 1.0

## 1. Question

Does the CGR (CATIA Graphical Representation) on-disk structure differ between
samples from the V5 R18 era (2007) and the current R2026 era (V5-6R2026 /
3DEXPERIENCE)? The answer shapes the P2 `oep-cgr` reader: a format that drifted
between eras needs a version-catalog + per-era parse paths; a stable one can
ride a single decoder with tolerant unknown-block handling (P2-M1-T04).

This notebook is the COMPARISON. It consumes the byte-level facts established
by P1-M2-T01 (`docs/re-notes/cgr-hex-notebook.md`, `python/oep_tooling/cgr_observe.py`)
and the sample provenance/registry owned by P1-M1-T01
(`workstreams/P1-M1/cgr-sample-registry.json`). The drift tool is
`python/oep_tooling/cgr_drift.py`; its acceptance test is
`tests/test_cgr_version_drift_acceptance.py` (TDD: written first, RED, then
GREEN on real material). This notebook is stored as a fellow RE note under
`docs/re-notes/` alongside `cgr-hex-notebook.md`.

## 2. Method

1. Fingerprint every real `.cgr` sample with `cgr_drift.fingerprint(path)`:
   magic, preamble zero pad, the 16-byte invariant template at 0x68..0x77,
   0x1000 alignment, header-region class-name strings, f32 at 0x7c.
2. Classify each sample into an era/family by OBSERVED structure, not by
   marketing release labels.
3. Run `cgr_drift.compare(probes)` to group by family and emit a drift verdict.
4. Honesty rule (program doctrine, decision 5 clean-room): any era with zero
   real samples is reported `status=blocked_on_sample`, never "no drift".

## 3. Real material analysed (2026-10-15)

Eleven real CGR files, all verified locally (sha256 in the P1-M1-T01 registry;
the proprietary binaries are NOT committed - clean-room):

- cfysa/ATOMIZER.cgr ... cfysa/VALVE.cgr (10 files) - CATIA V5 documentation
  DMU Space Analysis tutorial sample set (R19-era doc tree, ~2008-2009).
- piston/piston.cgr - independent author piston sample, 2007 (V5 R18-era).

## 4. Observed invariants (old-era leg, all 11 samples)

| Probe | Value | Samples |
|---|---|---|
| magic (u32 LE @0x00) | `01 00 00 00` | 11/11 |
| preamble 0x04..0x63 | 96 zero bytes | 11/11 |
| invariant template 0x68..0x77 | `00 07 00 00 00 07 00 00 00 b1 01 0b 01 08 02 4f` | 11/11 |
| file size | multiple of 0x1000 | 11/11 |
| scalar encoding | little-endian | 11/11 |

Conclusion: within the old V5 family band, the CGR header SPINE is stable
from ~2007 (R18-era) through the R19-era doc set. Strong positive signal for
P2: a single header decoder can serve this band.

## 5. Observed drift: the two old-era families

| Family | Samples | Marker |
|---|---|---|
| class-string-tail | piston.cgr | ASCII `CAT3DBagRep` / `CATSurfacicRep` class names in header |
| float-tail | ATOMIZER ... VALVE (10) | f32 at 0x7c clusters ~1.158; no header class strings |

The two families share the invariant header spine but differ in the tail field
at 0x78/0x7c (class-name strings vs a float). That tail difference is the
observable "drift" between the two old-era writer families and is exactly what
P2 needs two parse paths for (P1-M2-T01 section 8 confirms and feeds this).

Drift verdict on real material: `stable-header-within-family,tail-drift-detected`.

## 6. The R2026-era leg: recorded as blocked_on_sample [uncertain]

No license-clean, independently-downloadable V5-6R2015+/R2026-era CGR sample
was obtainable as of 2026-10-15. The P1-M1-T01 registry documents this: modern
CGRs are vendor-locked (ship inside CATIA/DM entries or commercial converter
installs), or the public samples carry no redistributable license. Per program
doctrine this leg is reported `status=blocked_on_sample` and is NOT treated as
"no drift".

What "R2026 era" would test once a clean sample exists:
- A V5-6R2026 (R36) or 3DEXPERIENCE R2026x exported `.cgr`.
- Compare magic, preamble, invariant template, tail style against the old-era
  baseline above.
- Per commercial-reader version tables (P1-M3-T02), native readers support CGR
  R10..R25+ with a different wrapper family (`V5_CFV2` container is the
  CATPart/CATProduct container magic; `__3_NEWPNFCT` is a newer native
  signature reported by file-format aggregators). Neither appears in our
  old-era direct-format samples, so the modern leg likely differs at the
  wrapper level — to be verified on a real modern sample, [uncertain].
- Lock in the diff harness P1-M2-T06 to bisect which bytes change across
  re-export before assigning meaning to any modern-era field.

## 7. Findings for the P1 report / P2 reader

1. CGR header structure is stable across the observed old V5 band (2007 .. R19
   doc set): same magic, preamble, template, alignment. Leg 1 of the drift
   question is answered on real material: NO header drift within this band.
2. The tail is where version/writer drift shows: two observed families
   (class-string vs float) need separate parse paths in P2.
3. The R2026-era leg is UNVERIFIED on real material (blocked_on_sample). A
   reader built strictly from the old-era corpus may need adjustment when a
   modern sample is procured; P2 adds a tolerant unknown-block model
   (P2-M1-T04) and a version catalog (P2-M1-T02) to absorb that risk.
4. No direct CGR-in-`V5_CFV2` or `__3_NEWPNFCT` sample was observed in the
   eleven-file real corpus; those wrapper families are [uncertain] external
   observations to confirm on a modern sample before designing for them.

## 8. Open items

- Procure a license-clean modern CGR (V5-6R2026 or 3DEXPERIENCE R2026x) and
  re-run `cgr_drift.py` with it; flip the modern leg from blocked to verified.
- Confirm the 0x64 landscape u32 semantics (P1-M2-T01 section 9) once ≥2
  re-decorated versions of the same part exist (P1-M2-T06 diff harness).
- Wire the drift tool into the corpus harness runner once P1-M1-T04/T05 sync
  lands (corpus manifest entries for cgr samples).

## 9. Invariant / clean-room statement

All probe values above were read from the byte content of the named public
sample files on 2026-10-15. No Dassault/Spatial/Okino library read any CGR for
us and no vendor binary was executed. Observations only; proprietary binaries
are not redistributed (P1-M1-T01 license lane).
