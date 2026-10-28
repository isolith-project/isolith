---
title: Isolith SolidCore Evaluation (second oep-geo Evaluator implementation)
decision: evaluate_solidcore_as_second_kernel_behind_the_oep_geo_seam
version: 1.0.0
date: 2026-10-27
status: Proposed
owner: Isolith program governance, P6-M3 (B-Rep ops); Don directive 2026-10-27
applies_to: oep-geo kernel seam (P6-M3-T04 / ADR v1.2.0+), oep-engine-occ (decision #19), the wasm/browser lane (oep-web/oep-worker), and any future second kernel engine crate
supersedes: nothing (evaluation card SOK-20261027; the OCC decision #19 stands unless the verdict below finds a hard blocker, which it does not)
evidence: workstreams/modeler/kernel/proof/*.log
---

# Isolith SolidCore Evaluation

This memo evaluates SolidCore (https://github.com/tangency-labs/solidcore,
Apache-2.0, a Rust port of NVIDIA's SMLib B-rep kernel from
NVIDIA-Omniverse/usd-brep) as a SECOND implementation behind the oep-geo
Evaluator seam. It is an evaluation card, not a migration: nothing in this
memo switches a production code path from the OCCT kernel
(decision #19, `oep-engine-occ`, LGPL-2.1 with exception). Per decision #15
(branch from main, one change at a time) this card only adds evidence, a
memo, and a standalone probe fixture; the OCC decision stands unless a hard
blocker is found, and none was. The verdict offers three ranked options for
Don to choose from.

Bottom line up front, stated plainly:

- **License and provenance: clean.** Apache-2.0 all the way down (the port
  and the NVIDIA upstream), verified by reading the license files and the
  source headers, not the sidebar. Using it is fully compatible with the
  clean-room invariant and the no-vendor-binaries rule, and it is strictly
  cleaner for this program than the ratified OCCT dependency (LGPL-2.1 +
  exception) because it is Apache-2.0 with no copyleft at all.
- **It builds and tests green on this host for real**, including the full
  differential harness Rust-vs-C++ (`difftest --green`: 261 units, 255
  passed, 1 partial, 0 failed, 5 platform skips, EXIT 0), the
  `wasm32-unknown-unknown` target, and the whole thing with no `unsafe` in
  the kernel.
- **The capability bar the OCC path already passes is met.** Boolean add
  and subtract, extrude, revolve, and single-edge fillet all reproduce the
  same analytic / kernel-seam golden volumes to 1e-6 relative (measured to
  ~1e-10). A full reference-bracket shape (80x40x6mm plate + 6mm through
  hole + fillet r2) builds and measures correctly.
- **Two honest gaps**: (1) there is no per-edge chamfer entry point in the
  0.0.x API (chamfer runs whole-solid), and (2) the API is 0.0.x, fifteen
  days old, and one sweep unit is "partial" for a documented reason inside
  the C++ reference itself.
- **Recommended verdict (option 1):** adopt as the second implementation
  behind the seam with OCC staying the native default, and use SolidCore for
  the browser/wasm lane that OCCT cannot serve.

## 1. License and provenance audit

Conclusion: **fully compatible with the clean-room invariant and the
no-vendor-binaries rule. No violation.**

Source of truth, in order:

- **SolidCore `LICENSE`** (repo root, 11,449 bytes): the Apache License 2.0
  text with the upstream header "Copyright (c) 2020-2026 NVIDIA CORPORATION &
  AFFILIATES. All rights reserved." Verified by reading the file start and
  end; this is the canonical Apache-2.0 text.
- **SolidCore `NOTICE`** (1,494 bytes): solidcore contributors notice, states
  the product is a Rust port (derivative work) of the SMLib and SM_API
  components of USD BRep (v1.0.1, commit 3fd5b95), Apache-2.0, quotes the
  upstream notice, and records the modification summary (translated to Rust,
  arena topology, typed ids, error values, explicit kernel state instead of a
  global context).
- **SolidCore `ATTRIBUTION.md`**: pins upstream v1.0.1 / commit `3fd5b95`,
  declares the exact upstream paths used (`source/SMLib`, `source/SM_API`,
  `tests/SM_API_test`, `tests/prog_test`, `TestFiles/pt_TestFiles`,
  upstream dev docs) and NOT used (OCCT translators, USD/Hydra, Python
  bindings, proposals), carries the independent-project disclaimer (not
  affiliated with or endorsed by NVIDIA), and requires a per-file provenance
  header on every ported Rust file with a Rust-item-to-upstream-name table
  (AGENTS.md section). It also states "No GPL/LGPL code is used anywhere in
  solidcore."
- **Upstream `NVIDIA-Omniverse/usd-brep`** (cloned locally at the pinned
  commit `3fd5b95`): `LICENSE` is Apache-2.0 (NVIDIA). Every one of the 490
  `.cpp`/`.h` files under `source/SMLib` + `source/SM_API` carries the SPDX
  header `SPDX-License-Identifier: Apache-2.0` (492 SPDX markers found via
  grep). `THIRD_PARTY_NOTICES.md` at that commit covers OpenUSD, oneTBB,
  hwloc, MaterialX, OpenSubdiv, CPython, HACL*, pybind11, and the other
  layers, none of which is part of SMLib/SM_API and none of which solidcore
  ports or redistributes.
- The kernel crate enforces `#![forbid(unsafe_code)]` at the crate root and
  the workspace lint gates `unsafe_code = "deny"` (only `solidcore-ffi`, the
  C++ shim crate, is allowed `unsafe`, and the kernel has an explicit
  statement of "no global state, no panics on bad input, deterministic
  results" as design requirements).

Clean-room analysis against the program invariant (as recorded in
`boolean-ops-occ-adr.md` section 8: forbids vendor binaries, copied source,
and any Dassault/Autodesk/Siemens library being used or linked into
Isolith):

1. SolidCore is not a vendor CAD binary and is not extracted from any vendor
   product. It is a public, license-clean open-source Rust library porting a
   public NVIDIA SDK component that is itself Apache-2.0 licensed open
   source. NVIDIA is not a Dassault/Autodesk/Siemens product vendor in this
   sense, and none of the invariant's named vendor surfaces (CATIA/NX/
   SolidWorks/DWG/CATPart) is involved. Linking it through its public Rust
   API is use of licensed open source, which the invariant expressly
   permits ("public specs, lawful observation, license-clean sources").
2. No copied source enters our crates. A future adapter crate would depend
   on `solidcore` via Cargo (git or crates.io), never vendor the source,
   exactly like the existing no-vendored-OCCT guardrail. We extend the same
   CI scan to refuse a vendored solidcore source tree.
3. No vendor-format back door. SolidCore's input surface is its own B-rep
   model plus STEP (its `step-brep` / `step-import` / `step-export` crates
   are Apache-2.0 and out of scope for this card). It does not read CATPart
   or DWG. The P5 native-reader lane continues on its own observation work
   (corpus + hex notebooks + behavior specs) regardless of which kernel is
   behind the seam, exactly as the boolean ADR already records for OCCT's
   own importers.
4. Program-posture compatibility. Our crates are Apache-2.0; SolidCore is
   Apache-2.0. There is no LGPL/AGPL/GPL anywhere in it, so it introduces
   nothing that changes a crate's declared license and no copyleft
   obligation. That is a strictly simpler license posture than oep-engine-occ
   (LGPL-2.1 + exception, separately linked).

Residual risk worth recording: none material. The habit to guard is the same
one already named in the boolean ADR for OCCT -- "the kernel does it, so we
can copy the approach" in our own geometry code. Apache-2.0 does not forbid
copying, so the honest approach is: any code we deliberately take from
solidcore comes with Apache-2.0 attribution (NOTICE), and we prefer calling
the crate to copying from it. There is no license barrier either way.

## 2. Build and test for real on this host

All commands were actually executed on this host (U1, aarch64, Ubuntu
24.04.5 LTS, glibc 2.39, GCC 13.3.0, CMake 3.28.3, make 4.3, zstd 1.5.5,
20 cores, 121 GiB RAM; rustup toolchain pinned by the repo to Rust 1.98.0,
repo builds also verified under rustc 1.98.1). Raw logs are committed under
`workstreams/modeler/kernel/proof/`.

| Step | Command | Result |
|---|---|---|
| Clone | `git clone https://github.com/tangency-labs/solidcore` | HEAD `3c931f4c6f85a026b186954d5742152b7de8b42a` (2026-10-27) |
| Build | `cargo build -p solidcore` | EXIT 0, 36s debug |
| Test | `cargo test -p solidcore` | green (see log; full suite incl. slow regression cases) |
| Wasm | `cargo check -p solidcore --target wasm32-unknown-unknown` | EXIT 0, 1m17s -- builds for `wasm32-unknown-unknown` |
| C++ reference | `scripts/fetch-cpp.sh` | "no prebuilt reference for Linux aarch64 (only Linux x86_64)" -> built from source via `harness/cpp/build.sh`: "C++ build finished in 71 s; libs in bin/cpp/lib" (libSMLib.a, libSM_API.a) |
| Differential | `cargo run --profile difftest -p difftest --features cpp -- --green --jobs 4` | `difftest: 261 unit(s), 255 passed, 1 partial, 0 failed, 5 skipped(platform)`; EXIT 0; wall 327.5s |

What the differential harness proves: every green unit runs the SAME input
through the Rust port and the actual C++ SMLib reference and requires
identical topology and values within 1e-9 relative. Operation families that
map to Isolith's needs all pass with max relative error 0 in the committed
cases: `boolean.api_brep` (337 cases), `boolean.merge_p1..p5`,
`boolean.topology_intersector_p1..p3`, the blend/fillet stack
(`blend.fillet_corner_p1..p3`, `fillet_executive_p1..p3`,
`fillet_geom_p1..p6`, `fillet_intersector`, `fillet_solver_p1..p5`,
`fillet_standard_solver_p1..p2`, `fillet_custom_solver`), `mass.integrator`
(1,528 cases), `primitive.api_primitives` (2,035), `primitive.api_curves`,
`sweep.api_brep` (597), `sweep.api_surfaces` (885),
`sweep.rotational_sweep_geometry` (823).

What does not pass / is not covered, reported honestly:

- `sweep.topology_sweep` is the single PARTIAL unit (692 passed,
  max_rel_err 3.6e-14, 1 known mismatch, 8 parked, 2 rejected). Root cause is
  documented inside upstream itself: in non-manifold stitch cases the C++
  reference's silhouette visiting order is pointer-hash (std::unordered_map)
  order, i.e. heap-layout dependent (upstream notes ND-2 / ND-492). Rust's
  order is deterministic and in one case Rust succeeds where the C++
  reference returns an error. This is not a Rust divergence on ordinary
  input.
- 5 units SKIP on this host with the documented reason "C++ float-to-INT
  conversions require x86-64" (UB-35, issue #448): `geom.hash_table`,
  `heal.heal_data_p1..p3`, `surface.twist_volume`. They are unrelated to
  the boolean/fillet/sweep/primitive path Isolith needs; on x86-64 they run.
- `--green` covers status=green and status=partial units; deferred cases are
  not failures, and not every branch of every green unit is independently
  proven (the project's own docs say so).

## 3. Capability probe against Isolith's actual needs

The probe (`workstreams/modeler/kernel/probe/`, standalone crate
`oep-solidcore-probe` pinning the exact tested rev) drives SolidCore through
the same geometry and the same acceptance bar as the committed OCCT
kernel-seam test (`crates/oep-geo/tests/kernel_seam.rs` + the reference
bracket fixture `example-bracket.oepgeo.json`), comparing volume to 1e-6
relative of the analytic / OCC golden values. Full output:
`proof/probe.log`. Result: **ALL PROBE GATES PASSED**.

| Operation | SolidCore measured volume | OCC golden / analytic | Relative error |
|---|---|---|---|
| box-reference (unit cube) | 1.000000000000 | 0.9999999999999998 | ~2e-16 |
| union (two half-overlapping cubes) | 1.500000000000 | 1.4999999999999993 | ~5e-16 |
| difference | 0.500000000000 | 0.4999999999999999 | ~2e-16 |
| intersection | 0.500000000000 | 0.4999999999999999 | ~2e-16 |
| extrude (unit square, h=1) | 1.000000000000 | 0.9999999999999998 | ~2e-16 |
| revolve (annulus r in [0.4,1], h=1, 360 deg) | 2.638937829025 | 2.638937829015426 | ~4e-12 |
| fillet (one box edge, r=0.2) | 0.991415926539 | 0.9914159265358977 | ~3e-12 |
| chamfer d=0.2 (whole-solid op) | 0.896970562749 | (n/a -- see note) | scope differs |
| bracket 80x40x6mm minus 6mm hole | 19030.353996716018 | 19030.353996706152 | ~5e-13 |
| bracket + fillet r2 | 18571.250405863615 | (no closed form) | ran clean |

The bracket story works end to end: a real 80x40x6mm plate extruded, a 6mm
diameter through-hole subtracted without, and a radius-2 fillet applied to
the remaining solid -- exactly the Extrude + Hole + Fillet feature graph of
the reference bracket fixture, at the same 1e-6 relative tolerance bar the
OCC path already passes (actually measured to 1e-10 .. 1e-13).

Typed-error probes (mirror of the seam's "typed error, never a panic"
guardrail) both return `Err` as expected: zero-radius sphere and zero-size
box.

Honest caveats from the probe:

- **Topology counting convention differs.** SolidCore reports unique
  topology ids (unit cube: 6 faces / 12 edges / 8 vertices); the OCCT path
  reports per-face-occurrence counts (6 / 24 / 48). Volume and face count
  compare directly; edge and vertex counts do not. Any cross-engine golden
  or canonical trace that records edge/vertex counts is engine-specific.
- **No per-edge chamfer entry point in the 0.0.x API.** `SmApiChamferFillet`
  chamfers every blendable edge as a whole-solid operation (measured
  0.89697 on the unit cube at d=0.2, i.e. all 12 edges chamfered), which is
  not comparable to the OCCT single-edge chamfer golden (0.98). Chamfer
  works; per-edge selection does not.
- Volume integration accuracy is clamped upstream to [1e-8, 1e-1]; the probe
  used 1e-8, the seam's tolerance is passed by the caller as in the current
  design (P6-M1-T03).

## 4. Seam fit

The oep-geo seam (ADR v1.2.0/v1.3.0, `crates/oep-geo/src/kernel.rs`) is
already engine-agnostic: a typed public surface (`solid_boolean`,
`solid_fillet`, `solid_chamfer`, `make_box/cylinder/sphere`,
`solid_extrude`, `solid_revolve`, `solid_bbox`, `solid_translate`,
`solid_mirror`) over an internal `engine_iface` module that dispatches on
`#[cfg(feature = "kernel")]` to `oep_engine_occ` functions returning tokens
and `ShapeInfo`. A SolidCore-backed engine fits the same shape with one
adapter crate; the probe already proves the mapping (Kernel session +
Brep arena in, measured SolidMeasures out, typed errors).

Card-sized steps to implement as a second implementation:

1. **oep-engine-solidcore crate (adapter).** New Apache-2.0 crate exposing
   the same interface surface as oep-engine-occ (`sys_new`, `sys_drop`,
   `make_box/cylinder/sphere`, `boolean`, `edges`, `fillet`, `chamfer`,
   `extrude`, `revolve`, `bbox`, `translate`, `mirror`, `info`) implemented
   on the SolidCore API, mapping `SolidError` to the existing `EngineError`.
   The probe logic ports almost directly. No seam changes.
2. **Feature-gate the engine choice in oep-geo.** Add a
   `kernel-solidcore` cargo feature alongside the existing `kernel` (OCCT)
   feature; `engine_iface` routes on the feature pair. Both engines compile
   in the same workspace unambiguously behind their features; the default
   native build keeps OCCT. This is where decision #15's "one change at a
   time" applies -- the adapter lands first, the routing second, and no
   production path switches kernels inside this card.
3. **Session and measure translation.** SolidCore `Kernel` value (holds all
   tolerance, RNG, caches -- no global state) + an id->Brep arena is the
   natural "session" carrying oep-geo's `SysHandle`. Measure translation:
   `brep_volume`/`brep_area`/unique topology counts -> `SolidMeasures`
   (with the counting-convention caveat above; face counts and volume match,
   edge/vertex counts require an engine-specific golden set).
4. **Edge-index contract.** The seam names fillet/chamfer edges by index
   (`EdgeRef`). Indexing order is engine-specific, so a hardcoded edge index
   would name different edges on each engine. The adapter must either expose
   stable edge selection by geometry (near a point, as SolidCore's own
   whole-op runner does) or document that edge indices are engine-local.
   The current seam test only fillets `edges[0]`, which is fine.
5. **wasm lane.** SolidCore builds for `wasm32-unknown-unknown` (verified,
   EXIT 0). OCC cannot build for wasm, so SolidCore is the only kernel
   available on a browser lane today. Consumers must link the wasm side with
   a larger stack (project documents `-C link-arg=-zstack-size=8388608`) and
   the seam functions must stay sync/`Result`-shaped for wasm. Wire it behind
   `kernel-solidcore` in `oep-web`/`oep-worker`.
6. **Guardrails + gate.** Extend the existing seam-denial CI scan for the
   SolidCore identifier set (or keep it engine-generic), add the
   no-vendored-solidcore-source scan, pin the exact `solidcore` rev (cron/
   Dependabot-style review per bump, not floating), and add the probe to CI
   as the through-the-seam gate for the second engine.

Both implementations behind a feature flag in the same workspace: **yes,
feasible and low-friction** -- this is precisely what the `kernel` feature
already does for oep-engine-occ, and a second feature + engine crate slots
into the same `engine_iface` dispatch. The one material design decision for
the orchestrator to settle: the exact feature names (`kernel` stays OCCT;
suggest `kernel-solidcore` for the second), and whether the wasm lane implies
its own crate (`oep-engine-solidcore-wasm` or a build profile).

## 5. Verdict

No hard blocker was found. The OCC decision (#19) stands; nothing here
changes a production code path. Three ranked options:

**Option 1 (recommended): Adopt SolidCore as a second implementation behind
the seam; OCC remains the native default; SolidCore carries the
browser/wasm lane.**
Rationale: it is the only route to a real solid kernel on `wasm32-unknown-unknown`
today (OCC has no wasm target), it is Apache-2.0 (cleaner than the ratified
LGPL dependency), pure Rust with no unsafe and no C++ build on any target,
deterministic and panic-free by design with unusually strong differential
evidence (261-unit Rust-vs-C++ agreement, exit 0, on this host), and the
capability bar the OCC path passes is met on the exact bracket geometry. The
native path gains a drop-in second engine that exercises the seam's
documented replaceability (ADR sections 5 and 9) for real, at the cost of
two to three focused cards.

**Option 2: Adopt for the browser/wasm lane only; keep the native seam
OCCT-only for now.**
Rationale: the cheapest way to unlock the wasm story without a second native
engine in the near term. Downside: the native path has no second
implementation exercising the replaceability story, and the wasm lane cannot
be validated against the OCC golden without a native build anyway.
Choose this if the browser lane is urgent and the native second engine is not
on the roadmap this quarter.

**Option 3: Do not adopt.**
The only specific supporting reasons are (a) the API is 0.0.x and the project
is fifteen days old, so maintenance is a commitment to a fast-moving
dependency, and (b) one sweep unit is partial. Neither is a functional
blocker today: the operations Isolith needs are green and differentially
verified, errors are typed, and the rev is pinned. Rejecting purely on age
would forfeit the program's strongest wasm story; this option is not
recommended.

Honest obstacles to record regardless of the choice made:

- API 0.0.x volatility: every upstream bump must be reviewed before the
  pinned rev moves; two in-flight cards (adapter, routing) must agree on the
  pin. Plan a re-evaluation at each version bump.
- The seam's committed goldens and canonical trace bytes
  (`.oepkrt`/`kernel-seam-golden.json`) are OCC-specific for edge/vertex
  counts; a second engine needs its own golden set or convention-normalised
  comparison (volume + face count compare engine-agnostically).
- Edge indices are engine-local; the seam must not promise cross-engine edge
  numbering for fillet/chamfer targets.
- No mesh tessellation is produced by the SMLib port (upstream has no B-rep
  face-to-mesh tessellation; only boundary wires). A renderer or export path
  that needs exact B-rep meshing must tessellate itself; this matters most
  on the wasm lane. (Isolith's viewer already tessellates for display.)
- "No panic on bad input" is a design claim with strong lint enforcement and
  tests; the project's `--fuzz` corpus run was not part of this evaluation's
  evidence, so treat it as verified-by-construction-plus-difftest, not
  independently fuzz-gated here.
- wasm stack size: consumers link the wasm side with a larger stack (no
  guard page on wasm32-unknown-unknown).

## What works and what does not, today

Works on this host at rev `3c931f4` (recorded in the proof logs): cargo
build, cargo test, wasm32 build, full Rust-vs-C++ differential green
(255/261 + 1 partial + 5 x86-64-only skips), boolean union/difference/
intersection, single-edge fillet, extrude, revolve, primitives, mass
properties, typed errors, and the full bracket shape (extrude + hole +
fillet) at the 1e-6 relative bar.

Does not work today: per-edge chamfer selection (whole-solid chamfer only);
the 5 x86-64-only units cannot be differentially compared on this aarch64
host (they run on x86-64); `sweep.topology_sweep` stays partial for the
documented C++ pointer-order reason; and no production Isolith code path is
wired to SolidCore yet (by design, this card adds none).

## Evidence index

All raw logs committed under `workstreams/modeler/kernel/proof/`:
`build-solidcore.log`, `test-solidcore.log`, `build-wasm.log`,
`cpp-fetch.log`, `difftest-green.log`, `probe.log`; probe fixture under
`workstreams/modeler/kernel/probe/` (Cargo.toml pins solidcore rev
`3c931f4c6f85a026b186954d5742152b7de8b42a`).

## Change log

- 1.0.0 (2026-10-27): Initial evaluation memo. License/provenance audit
  (Apache-2.0 chain, clean-room compatible, no-vendor-binaries compliant),
  real build/test evidence on U1 (cargo build/test, wasm32, C++ reference
  built from source, `difftest --green` 255 passed / 1 partial / 0 failed /
  5 platform skips, EXIT 0), capability probe on the reference bracket
  geometry at the 1e-6 relative bar (all gates passed; separate note on
  topology-count convention and whole-solid chamfer), seam fit as six
  card-sized steps, and a three-option verdict recommending adoption as the
  second implementation with OCC as default and SolidCore on the wasm lane.
