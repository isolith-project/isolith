# CGR Color/Material Table: Observed Record Signature and glTF Export

P2-M4-T01 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

A bounded slice of the CGR material lane: a tolerant color/material-table
scanner over real CGR bytes plus a deterministic exporter to a glTF 2.0
`materials` array.

Deliverables:

- `crates/oep-cgr/src/material.rs` — `ColorRecord`, `scan_color_records`
  (never-panicking, bounds-checked; tolerant failure per P2-M1-T04),
  `to_gltf_materials_json` (public Khronos glTF 2.0 schema).
- `crates/oep-cgr/tests/golden_material.rs` — the TDD acceptance test
  (written RED against the skeleton crate, then GREEN).
- `crates/oep-cgr/examples/dump_recs.rs` — tiny driver that scans a real CGR
  and prints the glTF materials JSON (observation/export demo).
- This RE note (evidence + provenance + verification transcript).

Boundary (who owns what): the full glTF/glb MESH writer is P2-M2-T03 and the
general exporter lane is P6-M4-T01. This task owns ONLY the color/material
side: the parse plus the `materials`-array emitter. Downstream cards consume
`scan_color_records` as their color source and extend the exporter.

## 2. The golden sample

| Field | Value |
|---|---|
| File | `VALVE.cgr` (cfysa set, CATIA V5 documentation "DMU Space Analysis" tutorial) |
| Era | CATIA V5 R19-era doc tree (2008-2009) |
| Size | 176128 bytes (0x1000-aligned) |
| sha256 | `43cc0cb474b5fe10a4d0bf1b8c61f48d952a8bba849aa8b116e12b490d7dcbd6` |
| Registry | P1-M1-T01 `workstreams/P1-M1/cgr-sample-registry.json` id `dassault-dmu-space-analysis-valve` |

Clean-room rule (board.program.json): the proprietary binary is NEVER
committed. The repo records observed facts (this note + constants in
`material.rs`); the acceptance test re-derives them from real bytes when a
private observation directory is supplied via `OEP_CGR_SAMPLES`.

## 3. The observed color-record signature

All values read back from real bytes on 2026-10-15. On the R19-era cfysa
family, per-surface color data is carried as records whose head is:

```text
offset  size  value
+0      4     f32 LE color scalar c, c in (0.5, 1.0]   [uncertain: grayscale?]
+4      4     marker word: 00 00 00 01  (u32 LE == 0x01000000)
+8      4     marker word: 00 01 00 00  (u32 LE == 0x100)
```

- The leading scalar is read as a grayscale diffuse component (R=G=B=s).
  `[uncertain]` whether the writer stores pure grayscale or a scalar that
  quantises an RGB family; VALVE's repeated `0.812072` is consistent with
  CATIA default part gray (~0.808). The exporter maps it to
  `baseColorFactor = [s,s,s,1.0]`, documented in code.
- Marker words confirmed identical at every record head. Records are NOT
  4-byte-aligned in the file (e.g. heads at 0x010afb, 0x024193), so the
  scanner steps by single bytes.
- Record tail fields (metadata/specular block) are NOT asserted here; they
  belong to the P2-M3 attribution cards.

## 4. Corpus footprint (observed counts)

| Sample | Color records | First scalars |
|---|---|---|
| ATOMIZER.cgr | 0 | - |
| BODY1.cgr | 1 | 0.9191 |
| BODY2.cgr | 31 | 0.8087, 0.5441, 0.8283, ... |
| LOCK.cgr | 7 | 0.5435, 0.5693, 0.6272, ... |
| NOZZLE1.cgr | 0 | - |
| NOZZLE2.cgr | 0 | - |
| REGULATION_COMMAND.cgr | 4 | 0.6571, 0.8852, ... |
| REGULATOR.cgr | 0 | - |
| TRIGGER.cgr | 2 | 0.6922, 0.6845 |
| VALVE.cgr | 16 | 0.5397, 0.5394, 0.8121 x6, 0.5297, 0.8987, ... |
| piston.cgr | 0 | - |

The signature is verified on the R19-era cfysa family only. The older
piston-family (2007-era, with `CAT3DBagRep`/`CATSurfacicRep` class strings)
does NOT carry it in any sample seen; per P1-M2-T02 this family-pair needs a
separate parse path and is out of scope here (the tolerant scanner simply
finds nothing and reports it).

## 5. TDD transcript

RED (before `material.rs` existed):

```
error[E0432]: unresolved import `oep_cgr::material`
  --> crates/oep-cgr/tests/golden_material.rs:28:14
   | import fails because the skeleton crate has no material module
```

GREEN (deterministic suite, no samples):

```
running 5 tests (tests/golden_material.rs)
scan_finds_observed_signature        ... ok
scan_ignores_junk_without_panicking  ... ok
export_materials_valid_gltf          ... ok
export_empty_table_is_valid          ... ok
golden_sample_real_material          ... ok  (SKIP leg, no samples)
```

Real-material leg (OEP_CGR_SAMPLES=.../cfysa):

```
info  first record @0x10ab2 scalar=0.539713 (total 16)
PASS  real golden sample: 16 color records parsed
```

Note: the real leg caught TWO latent bugs during development:
(1) marker-word byte order (record bytes are `00 00 00 01`, and the initial
`01 00 00 00` constant found nothing); (2) byte-level vs 4-byte stepping
(unaligned heads at 0x010afb etc. were missed). Both fixed and re-verified —
the real-material leg is exactly why it exists.

Export check on real material (public glTF 2.0 materials schema):

```json
{"materials":[{"name":"cgr-material-0","pbrMetallicRoughness":{"baseColorFactor":
[0.539713,0.539713,0.539713,1.0],"metallicFactor":0.0,"roughnessFactor":1.0}},...]}
```

Valid JSON, 16 materials for VALVE.cgr, parsed back with the standard
library `json` module during verification.

## 6. Boundary statement

- Parses the color-record signature only; means of the scalar as a grayscale
  diffuse is `[uncertain]` (documented, not guessed).
- Full glTF/glb writing (nodes/meshes/primitives) is P2-M2-T03; mesh
  attribute attribution (which facets own which records) is P2-M3.
- piston-family CGR and any R2026-era wrappers (`V5_CFV2`,
  `__3_NEWPNFCT`) are unverified / not targeted; tolerant scanner reports
  zero records and never panics (P2-M1-T04).

## 7. Sources

- Real corpus: ~/.hermes/cache/scratch/cfysa/*.cgr (private observation
  cache; sha256 per file in the P1-M1-T01 registry).
- Header/endianness/alignment facts: docs/re-notes/cgr-hex-notebook.md
  (P1-M2-T01); version-family note: docs/re-notes/cgr-version-drift.md
  (P1-M2-T02); rep-graph hypothesis: docs/re-notes/shape-record-hypothesis.md
  (P1-M2-T05).
- Master plan: /home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md
  (P2-M4-T01); execution plan: /home/r1a/.hermes/plans/2026-10-15_171530-p2m4-color-material-gltf.md.
