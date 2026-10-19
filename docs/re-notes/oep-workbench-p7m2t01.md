# OEP interchange workbench (P7-M2-T01) RE note

Status: delivered, PR review pending
Card: P7-M2-T01 (wave-2 core, P7 modeler lane; gated on the P2-M7-T01 real-CGR mesh latch)

## What this card delivers

The "OEP interchange workbench" in the adopted shell (P7-M1-T01 keep-own-shell
decision): the shell's panel/command surface for CGR / 3DXML / STEP / PDM load
and save, realized headless-first as a typed command surface plus a
machine-callable CLI example.

* `crates/oep-shell` (new, P7): `workbench::Workbench` + `WorkbenchAction`
  (Load Cgr/ThreeDXml/Step, OpenFromPdm, SaveToPdm, Save) with a typed, never
  panicking `WorkbenchOutcome` and an ordered operation journal. Every load
  goes through the importer hub (`oep-import`) so P2/P3/P4 readers feed the
  one canonical `oep_geo::Document`. The `workbench` example binary drives the
  same surface from the command line (the P7-M5 CLI-parity target in embryo).
* `crates/oep-pdm` (grown from skeleton, P8): the PDM API contract
  (`api::PdmClient` with save_revision / open_revision / list_revisions /
  head_revision + typed `PdmError`) and a dependency-free in-memory local stub
  (`local::LocalPdmVault`, FNV-1a content addressing, documented as non-crypto
  until the P8 SHA-256 store lands). The workbench PDM save/open surface runs
  against this contract: this is the "save path stubbed against the PDM API
  contract" named by the acceptance. P7-M2-T02 wires the real server client
  behind the same trait.
* `oep-import` adapter work: the CGR adapter now latches BOTH eras into the
  canonical hub surface (classic .cgr via the old extractor, modern V5_CFV2
  material via the P2-M7-T01 latched `modern::extract_modern`, carrying the
  measured records/index/vertex/triangle/max_index onto the import feature);
  the STEP adapter seam is closed (real P4 read lane instead of the wave-1
  Reserved seam); `SourceKind::detect` recognizes the modern V5_CFV2 magic.
* Committed real-material goldens (byte-locked by Rust, re-parsed by the
  stdlib-only Python acceptance):
  * `cgr-modern-stc06-import.oepgeo.json` - the canonical Document produced
    from the real NIST STC-06 (V5-6R2023) latch: records=18, index=2003,
    vertex=1037, triangle=1389, max_index=1990, bbox x[-152,152] y[0,98]
    z[-248,0].
  * `box-reference-import.oepgeo.json` - the canonical Document from a real
    STEP part (unit cube).

## Acceptance status (measured)

* Workbench loads a real latched CGR mesh: MET. Live import of
  nist_stc_06_asme1_V56R2023.CATPart through Load(Cgr) reproduces the
  committed golden byte-for-byte and the feature carries the exact latch
  counts (Rust byte-lock in importer_hub.rs + workbench test; both PASS on
  real material, and the committed golden re-parses in CI with no corpus).
* Workbench loads a 3DXML: MET. cube1.3dxml imports through Load(ThreeDXml)
  to the committed golden structure (2 parts / 1 solid / 1 assembly); the
  static leg always runs.
* Save path stubbed against the PDM API contract: MET. SaveToPdm publishes
  the oep-geo JSON through the `PdmClient` contract into the local stub,
  OpenFromPdm restores it byte-exact (round trip test), and the stub contract
  is exercised independently (oep-pdm tests: byte-exact open, content-address
  dedup, newest-first list, typed missing-item/revision errors).
* STEP load: MET via the closed seam (reserved seam honestly removed).
* CGR/3DXML/STEP save buttons: typed `InterchangeSavePending` naming the
  owning writer lane (P3-M4 3DXML writer, P6-M4 exporters, no CGR writer
  card) - never fabricated output.

## Honest boundaries / findings

* Bbox float reparse is 1 ULP unstable for f32-widened values (e.g. the
  STC-06 bbox min z -247.64999389648438): `oep_geo::from_json_pretty`
  reconstructs it one ULP off under the current serde_json decimal parser.
  Pre-existing oep-geo/serde property, unrelated to this card, out of scope.
  The goldens lock SERIALIZATION, which is deterministic and byte-stable, so
  the real-material gate is unaffected. Flagged for a P6 maintenance card.
* The PDM stub is in-memory per process; on-disk persistence, revision
  immutability under release, RBAC and the audit log are P8 deliverables.
* The 3DXML live leg skips in CI (no private corpus); the committed golden
  keeps the structure locked everywhere. The CGR modern live leg likewise
  skips without OEP_CGR_SAMPLES; the committed golden keeps the counts locked.
* egui panel wiring is deferred to the P7-M5 UX pass per the P7-M1 decision;
  the command surface this card delivers is the part of the workbench the
  milestone gate measures.

## Verification

cargo build/test --workspace green; clippy -D warnings clean; fmt clean;
pytest 44+ green (5 new test functions); precommit gate pass; CI (rustfmt,
clippy, unit, integration, precommit) expected green on the PR.
