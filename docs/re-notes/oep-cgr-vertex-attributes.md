# CGR Vertex-Attribute / Normal Record Observation

P2-M3-T01 · Isolith / OEP P2 reader
Status: observation draft, verified on real material 2026-10-15
Version: 1.0

## 1. Purpose

This note records what the CGR payload looks like around **normal and
per-vertex attribute** data, derived strictly by observing real files
(clean-room, lawful observation only; see `cgr-hex-notebook.md` P1-M2-T01 and
the P1-M1-T01 sample registry for corpus provenance). It is the evidence the
P2-M3-T01 recovery module and its acceptance test are built against: what
records exist, where they sit, and how "present" vs "absent" is classified.

Claims about what a field *means* are marked `[uncertain]`; byte values are
not guesses.

## 2. Method and corpus

Corpus: the eleven real CGR files from P1-M1-T01 (R19-era `cfysa/*.cgr` doc
set + older `piston.cgr`). Every number below was read back from real bytes on
2026-10-15. The observation tool is `python/oep_tooling/cgr_attributes.py`;
its output is re-derived by `tests/test_cgr_attributes_acceptance.py`.

## 3. Headline observations

### 3.1 Raw unit-length f32 normal runs are NOT present in this corpus

Scanning for contiguous runs of unit-length LE f32 triples (`|n|≈1.0`, ≥8 in
a row) finds **zero** runs in either family:

- piston.cgr: 0 runs
- VALVE.cgr, ATOMIZER.cgr, other cfysa files: 0 runs

Two readings `[uncertain]`:
1. the observed samples genuinely carry no explicit per-vertex normal table
   (CGR stores shading data differently on this writer path), or
2. normals live in a quantized/delta representation that the raw-f32 scan
   does not see.

Either way the **negative evidence is the contract**: an honest reader must
report "attributes absent" for these files rather than fabricating a default
normal. The acceptance test asserts exactly this.

### 3.2 Quantized-3-byte "normal-ish" runs exist but are noise, not tables

A weak filter (dequantize byte triples `[0,255]→[−1,1]`, accept `|n|` in
0.05..2) fires across dense payload regions and flags runs of thousands of
triples. Statistically dense packed floats satisfy the filter by chance, so
**this is not evidence of a normal table**; it is a noise floor. Reported for
honesty, deliberately NOT used as a recovery signal.

### 3.3 Recurring record marker in the older family: `20 41 01 ff ff ef d1 ca`

The old-family piston.cgr carries 154 occurrences of

    20 41 01 ff ff ef d1 ca [50 | 51]

each immediately preceded by a short run of small u16 values (index/vertex
counts `[uncertain]`) and immediately followed by dense data that decodes as
plausible f32 triples. Example at offset 0x0001c5:

    ... 6e 00 | 20 41 01 ff ff ef d1 ca | 51 | <float payload>

Readings `[uncertain]`:
- the `20 41 ...` spine is a record-header signature opening an attribute /
  geometry sub-record that follows a face/index record;
- the `50` vs `51` discriminator tags a record-variant (width or side), since
  it correlates with the sign of following float data.

The newer cfysa family does NOT contain this verbatim signature — version
drift (P1-M2-T02) means the newer writer encodes its record headers
differently. So a single recovery rule cannot span both families yet; that is
P2-M1-T02/T03 (version catalog + block walker) follow-on work.

## 4. Where the recovery module stands on this evidence

- Recovery operates **on the internal mesh** (`oep_cgr::mesh::Mesh`,
  P2-M2-T01): it annotates the mesh with per-vertex attributes "where
  present."
- A unit-normal table whose length matches the vertex list is recovered
  (`VertexAttributes::normals == Some`).
- A payload with no such table returns "absent" (`normals == None`) — an
  honest outcome, not an error.
- Structurally inconsistent claims (a table whose length cannot match the
  vertex list) yield a typed `AttributeError`, never a panic — the P2-M1-T04
  tolerant model applied to attributes.
- Real material today classifies every sample as "absent" (mesh-level
  classified skip → no vertex list to annotate); the synthetic-with-normals
  acceptance case proves the recovery path latches normals when a mesh and a
  matching table are both present.

## 5. Open questions / next probes (P2-M3-T02/T03, P2-M1-T02/T03)

1. Catalog the cfsa-family record header so its attribute payload can be
   addressed (P2-M1-T02/T03).
2. Run the P2-M1-T06 diff harness: save/re-save a CGR in a vendor-free tool
   and bisect which bytes change — that will expose the attribute encoding
   empirically.
3. Quantized/octahedral normal hypothesis: once the block walker lands,
   re-scan record payloads at record strides for 2-byte/3-byte quantized
   normals with the pool-length pairing, not the whole-file noise scan.

## 6. Invariant

Clean-room: all observations above come from public sample bytes (P1-M1-T01);
no vendor library read CGR for us, no proprietary bytes are committed. This
note is evidence for the P2 reader, not a redistribution license for the
samples.
