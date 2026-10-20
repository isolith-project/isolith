# P5-M3-T01: B-Rep extraction attempt - CATPart topology + NURBS into oep-geo

Task: P5-M3-T01 (Isolith / Open Engineering Platform, P5 native-reader
lane). Status: attempt completed, extraction parity classified honestly (no
fabricated PASS). Version: 1.0. Date: 2026-10-16. Companion tool:
`python/oep_tooling/catpart_brep_extract.py`; acceptance test:
`tests/test_catpart_brep_extract_acceptance.py`; committed report:
`docs/re-notes/brep-extraction-report.json`.

Owner note: this is the hardest clean-room work in the program. The saved
geometry of a real CATPart is split between byte-attributable surface records
(the container rep-graph, the B-Rep record-class markers, the analytic
surface-family references) and an encoded exact-geometry payload. The attempt
below extracts the first class exactly and marks the second honestly as
[uncertain] / blocked_on_encoded_payload wherever the record bytes do not
yield a gated decode. Everything in this note is read back from real corpus
files; nothing is inferred from vendor documentation or a vendor binary.

---

## 1. Question

Can a real CATPart's B-Rep (topology record chain + NURBS/analytic
surfaces) be extracted, clean-room, into the oep-geo native model? The task
body asks for "at least one real CATPart extracts a B-Rep face/surface chain
into oep-geo; extraction parity classified honestly (no fabricated PASS);
the hardest clean-room work in the program, mark uncertainty where it
exists."

## 2. Method

Byte-observation only: each real corpus file (resolved via the P1-M1-T04
registry, sha256-verified) is scanned with pure stdlib Python for the
byte-attributable markers documented below, with offsets pinned per file. A
separate strict structural gate attempts a canonical-analytic-surface decode
of the numeric payload; a passed gate is reported as a candidate with parity
[uncertain], never as verified geometry. The ADR P5-M1-T03 honesty budget
(from P4-M1) is applied verbatim: no fabricated geometry is ever emitted in
place of a failed decode.

Real corpus analysed: 131 native entries (123 catpart-v5 + 8 catproduct-v5),
all resolved (131/131, 0 unresolved), all verified magic `V5_CFV2` after
sha256 checksum against the registry.

## 3. What is byte-attributable (parity: exact, offsets pinned)

On every real part (123/123 catpart-v5), the tool finds and pins:

1. **Rep-graph chain**: `MechanicalPart` (part self-label), `PRDREP` +
   `Shape 1` (default representation), `_BagRepsList` + `BagRep` (the
   geometry bag). This is the container-level path to the B-Rep payload.
2. **Body containers**: `CATSeeBodyCont` (body), `CATBRepModeContainer`
   (B-Rep mode container), `CATCGMBODY`/`CGMGeom`/`CATMFBRP` (the CGM
   geometry containers holding the exact-geometry stream).
3. **B-Rep record-class markers**: `BrpRsur` (surface record), `BrpIRedge`
   (edge record), `BrpVertex` (vertex record), `BrpWire` (wire record),
   `SuperBRep`, `OperBReps`. 123/123 parts carry at least the surface-record
   marker; most carry the full topology record-class set. These are the
   record classes the part's B-Rep payload itself labels - faces/edges/
   vertices/wires of the saved boundary representation.
4. **Analytic surface-family references**: `GSMCylinder`, `GSMExtrude`,
   `GSMRevol`, `GSMPlane`, `GSMSphere`, `PlanarSupport`, `Fill` - the
   feature-tree references naming the family of each analytic surface the
   part uses, with per-file counts and byte offsets.

These records map into the oep-geo native model as
Part -> Body (solid) -> Feature records, each carrying the pinned offset and
an honest evidence string, as the committed per-file report and the single
part's oep-geo document in the acceptance test show.

## 4. What is NOT byte-attributable from a static sample (parity: [uncertain] / blocked)

The exact NURBS control points / knot vectors / analytic surface
coefficients are stored in an encoded exact-geometry sub-stream: every real
file (131/131) carries the `CATIA_V5 CB0001` ... `CB__END` region and/or a
second `V5_CFV2` sub-stream and/or the `0B~}0%`-family NV marker. Generic
struct scans of that payload produce high-entropy, non-self-consistent
numbers; the strict analytic-cylinder gate (finite values, plausible radius
>= 0.05 mm, unit axis) passes ZERO candidates across the whole 131-file
corpus, including on the freeform boat-hull surface sample.

Honest classification therefore stands:

- rep-graph and record-class presence: exact (byte-observed, offsets
  pinned), 123/123 parts;
- analytic surface-family references: exact (byte-observed feature-tree
  tokens), 123/123 parts;
- analytic surface parameters: [uncertain] - no structural decode passed the
  gate; the numeric payload that would carry radius/axis/limits is inside
  the encoded CGM/NV sub-stream whose record boundaries are not
  byte-attributable from a static sample;
- NURBS control points: blocked_on_encoded_payload - same encoded sub-stream
  (ADR P5-M1-T03 eighth-section honesty rule applied; no fabricated
  geometry).

The 8 catproduct-v5 files correctly carry NO mechanical-part B-Rep record
classes (a product is a composition graph of part references, per the
P1-M2-T03 container notebook); the tool records this as the absence it is,
not a failure.

## 5. Extraction parity classification (the honest bottom line)

Per the task's acceptance wording, extraction parity is classified:

| layer | verdict | evidence |
|---|---|---|
| container identity + rep-graph | exact | magic + MechanicalPart + Shape 1 + BagRep, offsets pinned, 131/131 |
| body/B-Rep container chain | exact | CATSeeBodyCont/BRepModeContainer/CGMGeom/CATMFBRP, offsets pinned |
| B-Rep topology record classes | exact | BrpRsur/BrpIRedge/BrpVertex/BrpWire present with offsets, 123/123 parts |
| analytic surface-family references | exact | GSMCylinder/GSMExtrude/GSMRevol/GSMPlane/GSMSphere in feature tree, offsets pinned |
| analytic surface parameters | [uncertain] | no gated decode; payload encoded |
| NURBS control points | blocked_on_encoded_payload | encoded CGM/NV sub-stream |

No fabricated PASS: the committed report and the acceptance test assert the
no-fabrication gate literally (features claiming parity "exact" must carry
byte evidence; gated candidates carry parity "[uncertain]"; the tool never
reports `exact_geometry_coefficients == "none_observed"`/decoded when the
payload is encoded).

## 6. Uncertainties (marked per the task)

- Whether the encoded CGM/NV payload inside a real CATPart is a lossless
  exact NURBS store or a compressed shell is [uncertain] from byte
  structure alone; it is "exact-geometry carrier" by the P1-M2-T05
  hypothesis (medium confidence), not by a proven decode.
- Whether a canonical-analytic surface exists in these tutorial parts with a
  structurally provable record is [uncertain]; the strict gate found no
  candidate, and the absence is honest, not "no geometry".
- The `+1` string-length idiom, u32@0x54 semantics, and the BagRep
  tessellation-vs-exact question remain [uncertain] and are P5-M2-T02 /
  P5-M3-T02 scope (save-modify-save diffs and a test-only converter
  reference), exactly as the ADR sequencing intended.

## 7. Reproduction

    python3 python/oep_tooling/catpart_brep_extract.py --corpus . \
        --json docs/re-notes/brep-extraction-report.json
    python3 -m pytest -q tests/test_catpart_brep_extract_acceptance.py

The acceptance test is skip-tolerant (SKIP, exit 0) when the object-store
mirror is unreachable, and FAIL-loud on the real-corpus leg when the facts
above do not hold (same pattern as the P5-M1-T01 drift acceptance test).

## 8. Version history

- 1.0 2026-10-16 initial honest B-Rep extraction attempt (P5-M3-T01).
