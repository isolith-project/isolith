# Diff harness: save/load/modify in a vendor-free tool, bisect which bytes change

Format: reverse-engineering observation notebook
task: P1-M2-T06 (Keelworks wave 1, P1 format reconnaissance)
Version: 1.0
Date: 2026-10-15
Status: verified on real material (6 real 3DXML samples, see "Verification record")

Owner: Keelworks P1 corpus / RE lane. Companion notebooks:
docs/re-notes/cgr-hex-notebook.md (P1-M2-T01, CGR header/block),
docs/re-notes/cgr-version-drift.md (P1-M2-T02, era drift),
docs/re-notes/catpart-catproduct-container-notebook.md (P1-M2-T03, containers).

---

## 1. Scope, method, clean-room

Goal: build and document a DIFF HARNESS - an observation tool that takes a
real sample, round-trips it through a vendor-free writer (save / load /
modify / save), then BISECTS exactly which bytes changed. The point is
byte-level attribution: "change semantic field X -> these bytes move, inside
this member, at these offsets." That attribution is the ground the P2/P3
readers and the P5 native-reader feasibility build on.

Why a diff harness matters (program doctrine, P1-M2-T06): file-format
reverse engineering is much stronger when we can WATCH a semantic change
propagate to specific bytes than when we only stare at static hex. A
controlled save/load/modify experiment turns "guess which field this is"
into "observe which field this is, by the bytes it flips."

Method (clean-room invariant, decision 5; mirrors P1-M2-T02/T03):
  * The vendor-free writer is Python stdlib ONLY (zipfile + ElementTree +
    re). No Dassault tool, no commercial CAD library, no vendor binary is
    executed anywhere in the harness.
  * Every byte in every table below was read, written, and re-read by the
    harness from the real sample files passed in; nothing is assumed.
  * A format with no vendor-free writer (CGR, CATPart, CATProduct) is
    reported status='blocked_on_tool' by the modify/bisect leg - never a
    fabricated diff, never a fake "no change" (same honesty doctrine as
    P1-M2-T02's blocked_on_sample).

Tooling delivered with this task (see section 8 for reproduction commands):
  python/oep_tooling/diff_harness.py
  tests/test_diff_harness_acceptance.py
  docs/re-notes/diff-harness.md   (this file)

## 2. Corpus used

6 real 3DXML samples from the wave-1 corpus (P1-M1-T02, committed under
permissive licences in corpus/data/3dxml/):

  name               licence     container     members   SchemaVersion in
  cube1.3dxml        MIT         zip (deflate)      7     Abaqus_Model.3dxml = 4.0
  blocks.3dxml       MIT         zip (deflate)      3     Abaqus_Model.3dxml = 4.0
  machinist.3DXML    MIT         zip (deflate)      9     CATMaterialRef.3dxml = 4.2
  cycloidal.3DXML    MIT         zip (deflate)     28     CATMaterialRef.3dxml = 4.2
  FM1_satellite.3DXML MIT        zip (deflate)     69     CATMaterialRef.3dxml = 4.2
  quad.3dxml         Apache-2.0  plain ASCII XML   -      (Header/SchemaVersion, text file)

The two writer lineages are deliberate: the trimesh fixtures (Abaqus_Model
naming, SchemaVersion 4.0) and the CATIA-V5-generated ASCII/LATE samples
(SchemaVersion 4.2/4.3). The harness treats any real 3DXML the same way.

## 3. What the harness does

Public API (all pure stdlib):

  detect(path) / detect_bytes(data)
      -> {'format': '3dxml-zip'|'3dxml-ascii'|'cgr'|'catpart'|'binary',
          'status': 'ok'|'blocked_on_tool', 'probe': {...}}
      Classification uses observed leading bytes: PK\x03\x04 (zip),
      01 00 00 00 u32-le (CGR magic, P1-M2-T01), "V5_CFV2\0" (CATPart/
      CATProduct, P1-M2-T03), "<?xml"/"<Model_3dxml" (ASCII 3DXML).

  load(path) / load_bytes(data)   -> sample dict (members for zip, text
                                     for ASCII, raw bytes always)
  save(sample)                    -> bytes written by the vendor-free,
                                     DETERMINISTIC writer
  modify(sample, {'SchemaVersion': '4.3'})
                                  -> new sample with ONE semantic field
                                     changed, all other members unchanged
  bisect(a, b)                    -> minimal list of changed byte-range
                                     dicts {start, length, old_hex, new_hex}
  member_diff(members_a, members_b) -> per-member changed ranges
  members_of(sample)              -> [(name, content_bytes), ...]

Two diff "levels" are reported deliberately:

  MEMBER level - compare decompressed member-by-member. Answer: which ZIP
  member carries the edited field, and at which byte offset inside it.
  This is the SEMANTIC answer (clean, localised, 1-2 bytes).

  CONTAINER level - bisect the whole archive bytes. Answer: which raw bytes
  of the file moved as a CONSEQUENCE of the edit (zip local-header CRC,
  deflate stream, central directory). Tagged zip-metadata; never
  mislabelled as a content change.

## 4. Determinism: why the harness's own churn is zero

Before believing any diff the harness reports, we proved the writer is
churn-free - because a save/load that added its own bytes would corrupt
every attribution:

  * content preservation : load -> save -> load reproduces every member's
    decompressed bytes exactly (checked on all 5 zip samples; 69-member
    satellite included).
  * idempotence           : save(save(x)) == save(x) byte-for-byte. The
    writer pins zip timestamps (1980-01-01) and deflate, so its output is
    a pure function of member contents. Any diff the user then sees is
    caused by THEIR edit, never by the harness.

Check on real material: all 5 zip samples produced identical re-save
output across two consecutive saves (len stable, bytes equal). See
verification record.

## 5. Observation on real material: bump one field, watch the bytes

Experiment (reproduced in section 8): load each zip sample, modify
SchemaVersion to 4.3, re-save, then bisect original vs patched at both
levels.

cube1.3dxml (SchemaVersion 4.0 -> 4.3):
  member level : Abaqus_Model.3dxml changed; range [{start: 270, length: 1,
                 old_hex: 30, new_hex: 33}]   (ASCII '0' -> '3')
  container    : 3 ranges, all zip metadata (local-header CRC + central
                 directory); member content bytes elsewhere untouched.
  7 members, size stays 3847 bytes.

machinist.3DXML (SchemaVersion 4.2 -> 4.3):
  member level : CATMaterialRef.3dxml changed; range [{start: 129,
                 length: 1, old_hex: 32, new_hex: 33}]  ('2' -> '3')
  container    : 5 ranges; the largest is a 301-byte deflate-stream run
                 (the compressed bytes of that member re-flow because the
                 edited byte sits mid-stream [uncertain: exact deflate
                 window not decoded here]; local/central header CRCs and
                 sizes move with it).
  9 members, size stays 39243 bytes.

cycloidal.3DXML and FM1_satellite.3DXML behave like machinist (edited
member = CATMaterialRef.3dxml, single 1-byte flip 32 -> 33, container
shows the compressed-stream consequence + central directory). FM1 is the
69-member stress case and still yields exactly ONE changed member.

blocks.3dxml behaves like cube1 (Abaqus_Model.3dxml, offset 270).

## 6. Findings (observation-based, honest)

F1. A 3DXML schema-version edit flips EXACTLY ONE decompressed byte in
    EXACTLY ONE member, and its offset is stable per writer lineage:
      4.0-lineage (Abaqus_Model.3dxml): offset 270 in that member
      4.2-lineage (CATMaterialRef.3dxml): offset 129 in that member
    The other 4+ members are bit-for-bit untouched. So SchemaVersion is a
    per-member, text-encoded leaf - consistent with the documented 3DXML
    manipulation API (ElementTree finds it as free text).

F2. The member carrying SchemaVersion differs by writer lineage, NOT by
    file size: the small trimesh fixtures keep it in the model document,
    the CATIA-lineage samples carry it in the material-reference document.
    A reader must therefore not hard-code "the version lives in the main
    .3dxml" - it must search members for the tag [uncertain: whether the
    4.0 lineage samples have a second copy elsewhere - not observed].

F3. Container-level bisection is dominated by zip bookkeeping, not content:
    edit one byte -> the archive's local-header CRC and central-directory
    fields flip, and for compressed members near the edited byte a run of
    deflate-stream bytes reflows. The harness separates these from member
    content; a raw byte-diff alone would mislead (hundreds of "changed"
    bytes for a 1-byte semantic edit). Lesson recorded for the P3 writer
    (P3-M4): build diffs at member granularity, archive level only for
    metadata.

F4. ASCII 3DXML (quad.3dxml): the harness classifies it as 3dxml-ascii and
    reports it as vendor-free - but the save/modify leg for plain XML is a
    text rewrite (no zip container), so the interesting bisection applies
    only to the zip lineage. Text-level round-trip is trivially content
    preserving; noted honestly, not asserted as a container experiment.

F5. CGR and CATPart/CATProduct (magic 01 00 00 00 / V5_CFV2): no
    vendor-free writer exists in the corpus toolset today, so the save/
    modify/bisect leg for those families is status='blocked_on_tool'. This
    is a HARD wall for the diff-harness method on those formats until a
    vendor-free writer (or a licensed separate-account observation rig,
    per P5-M2-T02 doctrine) becomes available - recorded, never faked.

## 7. What this unlocks

* P3 reader/writer (P3-M1..M4): the member-level bisection pattern - edit
  a leaf, watch exactly which member and offset moves - is the test oracle
  for the 3DXML reader/writer round-trip stability test (P3-M4-T02).
* P1-M2-T05 (shape-record hypothesis) can use the same harness to attribute
  geometry blocks: change a dimension, bisect which record/bytes move.
* P5-M2-T02 (native save-modify-save diff tooling) is the direct successor
  of this harness for formats whose writer we do not yet have; the BLOCKED
  wall here is mapped into that card.

## 8. Reproduction

All commands are pure stdlib; corpus files under
corpus/data/3dxml/ (pass via $OEP_3DXML_SAMPLES or repo samples/).

  # full acceptance (hermetic + real material)
  OEP_3DXML_SAMPLES=corpus/data/3dxml \
      python3 tests/test_diff_harness_acceptance.py

  # CLI: detect + diff one sample
  python3 python/oep_tooling/diff_harness.py \
      corpus/data/3dxml/cube1.3dxml 4.3

  # observe a second flight
  python3 python/oep_tooling/diff_harness.py \
      corpus/data/3dxml/machinist.3DXML 4.3

## Verification record

2026-10-15, real material, all GREEN:
  * all 5 zip samples: content preserved on round-trip; writer idempotent;
    exactly one changed member after a SchemaVersion edit; container
    bisect() returns >= 1 range.
  * quad.3dxml (ASCII) classified 3dxml-ascii/ok.
  * synthetic CGR (01 00 00 00) and CATPart (V5_CFV2) headers classified
    with status blocked_on_tool - honest wall, no fabricated diff.
  * hermetic unit checks (bisect run-location, tiny zip round-trip, member
    diff detection) pass with no samples present, so CI stays green even
    before corpus sync.

## Status lines

- 3DXML zip lineage: VERIFIED (5/5 samples, one-member one-byte bisection).
- 3DXML ASCII lineage: CLASSIFIED ok; container experiment not applicable.
- CGR: blocked_on_tool (no vendor-free writer).
- CATPart/CATProduct: blocked_on_tool (no vendor-free writer).
- Confidence: observation strong where tested; [uncertain] markers above.
