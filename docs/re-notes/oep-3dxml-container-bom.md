# oep-3dxml Container + BOM: Outer ZIP Parse and Manifest

P3-M1-T01 · Isolith / OEP P3 3DXML Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

The first working slice of the `oep-3dxml` Cargo crate: classify the 3DXML
container branch, parse the outer ZIP archive (member inventory), and read
the `Manifest.xml` container BOM (primary model document + optional
`<Date>`/`<WithAuthoringData>`), with a TDD acceptance test anchored to real
license-clean corpus files. The milestone boundary stops at the container
+BOM: `<Model_3dxml>` rep parsing is P3-M1-T02, tessellated mesh reps are
P3-M1-T03, and the binary `V5_CFV3` `.3DRep` branch is P3-M3.

Deliverables:

- `crates/oep-3dxml/src/container.rs` — branch detector (`detect`),
  ZIP member inventory (`parse_archive`), member decompression
  (`read_member`); typed, non-panicking errors.
- `crates/oep-3dxml/src/manifest.rs` — bounded std-only XML element
  extractor + `parse_manifest` (`root`, `date`, `with_authoring_data`).
- `crates/oep-3dxml/src/bom.rs` — `parse(bytes)`: container classification,
  outer ZIP parse, BOM decode, and BOM-coherence check (declared `<Root>`
  must name an existing member).
- `crates/oep-3dxml/tests/bom_zip_parse.rs` — TDD acceptance test (written
  RED against the skeleton crate, then GREEN).
- This RE note (evidence + provenance + verification transcript).

## 2. Grounding and clean-room posture

All asserted facts come from the license-clean MIT/Apache-2.0 corpus of
P1-M1-T02 (`corpus/data/3dxml` on branch `task/3dxml-samples-1015a`) and the
hex notebook P1-M2-T04 (`docs/re-notes/3dxml-hex-notebook.md`). The only
library linked is the `zip` crate (public PKWARE APPNOTE format; not Dassault
IP) with its pure-Rust deflate path — no vendor binary, no copied source, no
CGR/CATIA parsing. No version currently published for the `zip` 2.x line is
non-yanked on crates.io, so the crate resolves `zip = "9"` (9.0.3), features
`deflate` only (flate2 + zlib-rs + crc32fast; aes/bzip2/lzma/zstd excluded for
a lean, supply-chain-minimal build, P11-T06).

## 3. Observed container facts locked by the acceptance test

Golden map (verified byte-for-byte this session, 2026-10-15):

| File | Size (B) | head | Members | Manifest `<Root>` | Branch |
|---|---|---|---|---|---|
| blocks.3dxml | 2583 | `50 4B 03 04` | 3 | `Abaqus_Model.3dxml` | ZIP |
| cube1.3dxml | 4834 | `50 4B 03 04` | 7 | `Abaqus_Model.3dxml` | ZIP |
| machinist.3DXML | 38849 | `50 4B 03 04` | 9 | `NameSetTree.3dxml` | ZIP |
| cycloidal.3DXML | 742428 | `50 4B 03 04` | 28 | `NameSetTree.3dxml` | ZIP |
| FM1_satellite.3DXML | 1763813 | `50 4B 03 04` | 69 | `NameSetTree.3dxml` | ZIP |
| quad.3dxml | 14317 | `3C 3F 78 6D` | n/a | n/a | standalone XML |

`[observation]` All ZIP members are DEFLATE (method 8), including
`Manifest.xml`; a reader cannot read the BOM without inflate. Manifests are
ASCII XML; the Abaqus writer uses LF + pretty-printed lines + only `<Root>`;
the CATIA V5 writer uses CRLF and may emit the whole document on one line
with `<Date>` and `<WithAuthoringData>` added. `[uncertain: <Date> and
<WithAuthoringData> are writer-dependent, not format-required]`.

## 4. Detection order (entry point, per hex notebook §6)

1. bytes 0..4 == `50 4B 03 04` -> ZIP-archive 3DXML -> `parse_archive` +
   read `Manifest.xml` -> BOM.
2. bytes 0..4 == `3C 3F 78 6D` (`<?xm`) -> standalone ASCII model document
   -> reported as `StandaloneXml`, no archive (XML rep parse = P3-M1-T02).
3. anything else (including empty input) -> typed `ContainerError` /
   `BomError`, never a panic (fuzz-friendly, P2-M1-T05 discipline applied).

## 5. Verification transcript (real material, 2026-10-15)

`OEP_3DXML_SAMPLES=<corpus> cargo test -p oep-3dxml` (all 6 files:

```
PASS  blocks.3dxml: 3 entries, root=Abaqus_Model.3dxml
PASS  cube1.3dxml: 7 entries, root=Abaqus_Model.3dxml
PASS  machinist.3DXML: 9 entries, root=NameSetTree.3dxml
PASS  cycloidal.3DXML: 28 entries, root=NameSetTree.3dxml
PASS  FM1_satellite.3DXML: 69 entries, root=NameSetTree.3dxml
PASS  quad.3dxml: standalone ASCII model document detected (no ZIP)
test result: ok. 11 passed; 0 failed
```

When no sample is supplied (e.g. Gitea CI before the P1 corpus PR merges) the
real-material leg prints a tolerant SKIP so CI stays green; the test searches
`$OEP_3DXML_SAMPLES`, `samples`/`../samples` next to the crate, and the repo
corpus `corpus/data/3dxml`.

## 6. Open questions / next probes

1. Pre-4.0 schema manifests (hex notebook §8.4): corpus has no pre-4.0
   samples; the BOM shape may differ. `[uncertain]`.
2. `WithAuthoringData` semantics (authoring-data retention flag in the CATIA
   exporter) not yet tied out; stored opaquely.
3. ZIP64 / data-descriptor handling is delegated to the `zip` crate; no
   observed 3DXML uses them (largest member ~2.8 MB).
4. A future fuzz round (P3-M5) should point at `parse`/`detect`/`parse_manifest`
   to confirm the no-panic contract at scale.

## 7. Invariant

Clean-room only: the repo records observed facts, never proprietary bytes.
The synthetic test fixtures are built from public ZIP + observed manifest
structure. The corpus itself is committed by P1-M1-T02, not duplicated here
(no merge hotspot introduced).
