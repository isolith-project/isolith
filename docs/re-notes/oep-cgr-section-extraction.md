# CGR Section/View Extraction: Observation Gate + Computed Planar Cut

P2-M3-T03 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15 (TDD GREEN; real-material
box.stl leg PASS)
Version: 1.0

## 1. Question and verdict

Task: "Section/view extraction (planar cuts) IF records observed."

Observation verdict on the eleven-sample real corpus (2026-10-15): **NO
stored planar-cut / section / clipping records are observed.** The task's
conditional therefore resolves to: implement the honest engineering
equivalent a CGR viewer actually performs — a *computed* planar section over
the tessellated mesh. Delivered as `oep_cgr::section` (`cut` + `loops`),
format-independent, tested against the same internal-mesh contract surface
`&[[f32;3]] / &[[u32;3]]` that P2-M3-T01/T02 use.

## 2. Why the verdict: byte-level evidence (clean-room, real files)

Method: pure-stdlib probe (worker scratch `probe_sections.py`) over all
eleven corpus samples (cfysa/*.cgr R19-era doc set + piston.cgr 2007-era;
sha256 in the P1-M1-T01 registry; binaries never committed).

### 2.1 No string records
Case-insensitive scan for `section / sect / cutting / clipping / clip /
plane / planar / sectioning / CutView / SectionView / PROFILE / prof /
VIEW / view`: **0 hits in all 11 files.** A stored section/clip record
would carry a name/type tag; none exists at the string level.

### 2.2 No section type tag in the observed catalog
P2-M1-T02 (`crates/oep-cgr/src/catalog.rs`, verified on real material)
maps the observed CGR record type tags:

| Tag | Meaning |
|---|---|
| `CAT3DBagRep` | bag-of-reps container (3D representation aggregate) `[uncertain-but-consistent]` |
| `CATSurfacicRep` | surface representation `[uncertain-but-consistent]` |

plus the magic/landscape/tail markers. The observed record population is:
header spine, geometry/tessellation rep records, color/material records
(P2-M4-T01). There is no section/clip/cut/view record type.

### 2.3 Plane-equation records are not separable from float noise
Scanned every 4-byte-aligned offset for an exact clip-plane signature —
`(a,b,c,d)` LE f32 with unit-ish normal |(a,b,c)| in [0.9,1.1] and finite
offset |d|<1e4. Result per file (200KB-uniform-random baseline included):

| Sample | Plane-quad candidate hits |
|---|---|
| ATOMIZER.cgr / BODY1.cgr / BODY2.cgr | 5 / 3 / 16 |
| LOCK.cgr / NOZZLE1.cgr / NOZZLE2.cgr | 6 / 0 / 2 |
| REGULATION_COMMAND.cgr / REGULATOR.cgr / TRIGGER.cgr | 2 / 3 / 29 |
| VALVE.cgr / piston.cgr | 23 / 29 |
| **noise baseline (200KB random)** | **21** |

Real-file counts sit in the same band as pure random bytes (21/200KB), so
these are accidental dense-float matches, NOT structured section records —
the normals-card honesty rule (P2-M3-T01 §3.2) applied verbatim: a signal
indistinguishable from the density floor is reported as noise.

### 2.4 Confounding context (why this is expected)
`[uncertain]` design reasoning, not asserted vendor source: CGR is a
tessellated *display* representation of an already-computed part — it has no
exact B-Rep. A CATIA "section view / planar cut" is a viewer operation
(clipping plane) applied to that tessellation at display time, and is not
serialized into the .cgr (which is why every sample is a DMU-Space-Analysis
display cache with zero such records).

## 3. What is delivered instead (the computed planar cut)

`crates/oep-cgr/src/section.rs`:

- `Plane { origin, normal }` — oriented cutting plane.
- `cut(vertices, triangles, &plane) -> Result<Section, SectionError>` —
  strict-interior-crossing plane/triangle intersection. Contributions:
  - typed errors: `NoTriangles`, `IndexOutOfRange`, `NonFiniteVertex`,
    `InvalidPlane` (non-finite origin/normal, zero-length normal);
  - a triangle with ANY vertex on the plane is skipped (coincident /
    boundary cuts are degenerate, not sections — a documented convention,
    not an asserted vendor behavior);
  - a plane that misses the mesh returns Ok with zero segments (a missing
    clip is a fine, empty section, never an error);
  - the result is invariant to the sign of `normal`;
  - never panics on adversarial input (P2-M1-T04).
- `Section::loops(eps) -> Vec<Vec<[f32;3]>>` — chains raw segments
  end-to-end into contours; a closed contour repeats its first point as its
  last so callers can detect closure; non-chaining segments stay open
  polylines and are never dropped silently.
- `ON_PLANE_EPS = 1e-5`.

Wire-in is a one-liner once the P2-M2-T01 `Mesh` merges:
`cut(mesh.vertices(), mesh.triangles(), &plane)`. Section contours are the
input a P6/2D drawing lane would project to a section drawing `[future]`.

## 4. Interpretation honesty / boundaries

- The "no records" verdict is for THIS corpus (old V5 band). Modern
  CGR wrappers (`V5_CFV2`, `__3_NEWPNFCT`) are unverified
  (`blocked_on_sample`, P1-M2-T02); a modern sample may or may not carry
  annotation/section records — re-run the §2 probes when one is procured.
- The strict-crossing / on-plane semantics are OUR documented convention,
  not a claimed Dassault behavior.
- This card does NOT re-parse CGR bytes to find the triangle stream — that
  is P2-M2-T01's extraction job (currently 11/11 classified skips; the
  section module is deliberately format-independent so it compiles and is
  verified before raw extraction lands).

## 5. TDD transcript

RED (before `section.rs` existed):

```
error[E0432]: unresolved import `oep_cgr::section`
  --> crates/oep-cgr/tests/section_cut.rs:46:14
```

GREEN (workspace, 2026-10-15, cargo 1.98.1):

```
running 9 tests (crates/oep-cgr/tests/section_cut.rs)
never_panics_on_adversarial_inputs            ... ok
plane_missing_mesh_is_empty                   ... ok
plane_normal_orientation_does_not_change_cut  ... ok
rejects_bad_mesh                              ... ok
rejects_invalid_plane                         ... ok
single_triangle_oblique_cut_one_segment       ... ok
tangent_coplanar_face_is_not_a_cut            ... ok
unit_box_midplane_cut_closed_square           ... ok
real_box_stl_midplane_cut_closed_square       ... ok
test result: ok. 9 passed; 0 failed
```

Real-material leg (box.stl, unit box [0,1]^3, watertight, mid-height plane
z=0.5 with normal [0,0,1]):

```
box.stl: 8 vertices, 12 triangles
summary: 8 segments, 1 loops, loop pts=9, perimeter=4 (real material verified)
```

The cut correctly reports the 8 per-triangle crossings (4 walls x 2
triangles), the contour closes into exactly one loop with the 8 distinct
edge-crossing vertices (4 wall-wall corners + 4 triangle-diagonal
crossings) and unit-square perimeter 4.0.

Gates (mirror CI): cargo fmt --check PASS; clippy --workspace --all-targets
-D warnings PASS (0 findings); cargo test --workspace PASS (9/9 section_cut
+ sibling suites); scripts/gate/precommit-gate.sh PASS (pytest 14 + cargo).

## 6. Files

- Create: `crates/oep-cgr/src/section.rs`, `crates/oep-cgr/tests/section_cut.rs`
- Modify: `crates/oep-cgr/src/lib.rs` (one additive module line; hotspot)
- Create: this note.

## 7. Sources / invariant

- Real corpus: ~/.hermes/cache/scratch/cfysa/*.cgr + piston/piston.cgr
  (private observation cache; sha256s in the P1-M1-T01 registry; binaries
  never committed).
- Sibling evidence: `docs/re-notes/oep-cgr-vertex-attributes.md`
  (P2-M3-T01), `oep-cgr-topology-reconstruction.md` (P2-M3-T02),
  `cgr-color-material-observation.md` (P2-M4-T01), `cgr-version-drift.md`
  (P1-M2-T02), `cgr-version-catalog.md` (P2-M1-T02).
- Clean-room: all byte facts above were read from the named public sample
  by a pure-stdlib probe on 2026-10-15; no vendor binary/library read any
  CGR; no proprietary bytes are committed.
- Master plan: `2026-10-15_084708-open-catia-program-decomposition.md`
  (P2-M3-T03); execution plan:
  `2026-10-15_p2m3t03-section-planes.md`.

## 8. Version history

- 1.0 2026-10-15 initial section-extraction task (P2-M3-T03).
