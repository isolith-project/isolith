# P1-M1-T01: Public CGR Sample Files — Location and License Verification

Version: 1.0 (2026-10-15)
Owner: Keelworks program, P1 corpus
Status: verified. This task LOCATES public CGR (CATIA Graphical Representation) sample
files and VERIFIES the license of each sample. Program rule: no proprietary
redistribution (clean-room invariant, board.program.json). Real CGR binaries are used
for observation and evidence only; the shared repo records source + license + identity
evidence, not the proprietary binaries.

## 1. What CGR is

CGR = CATIA Graphical Representation, a lightweight tessellated (triangulated,
faceted) visualization format from Dassault Systemes, produced by CATIA V5 and used
for DMU, visualization-mode, and supplier view-only exchange. It stores display
geometry (triangles, visual attributes, limited structure/annotation) with no exact
B-Rep. It is a proprietary Dassault format with no public official specification;
all loaders either license Dassault components (Okino, Spatial, TransMagic, Datakit,
HOOPS Exchange) or are closed. CGR is therefore a lawful-observation / clean-room
target for the P2 reader workstream: we read bytes from publicly available files, we
do not redistribute vendor or copyrighted sample binaries.

References (public format background, not specs):
- http://justsolve.archiveteam.org/wiki/Cgr
- https://www.spatial.com/glossary/cgr-file
- https://resources.rand3d.com/insights-from-within/extracting-geometry-from-cgr-files

## 2. What was located

### 2.1 CATIA V5 online documentation tutorial samples (DMU Space Analysis)

Dassault's own CATIA V5 documentation (R11-era tutorial tree, mirrored at several
public doc mirrors) ships a set of small CGR tutorial samples used in the
"DMU Space Analysis" exercises: ATOMIZER.cgr, BODY1.cgr, BODY2.cgr, LOCK.cgr,
NOZZLE1.cgr, NOZZLE2.cgr, REGULATION_COMMAND.cgr, REGULATOR.cgr, TRIGGER.cgr,
VALVE.cgr. They are referenced in the online docs at
`cfysm/samples` (common functionalities sample folder).

The definitive download mirror we verified is the CATIA V5 R11 doc tree at
sual.narod.ru (cfysa.zip, 370 KB), whose DMU Space Analysis page explicitly links
the sample archive. The same sample set is cross-referenced by:
- kev009 CATIA-B18 doc mirror: http://ps-2.kev009.com/CATIA-B18/cfyugspa_C2/cfyugspabt0104.htm
- City University London academic mirror:
  https://www.staff.city.ac.uk/~ra600/ME2105/Catia%20course/CATIA%20Tutorials/
- anarkia333data CATIA help center mirror
- catiadoc.free.fr (references the same cfysm/samples set)

License verdict: Dassault Systemes proprietary tutorial/documentation sample data.
Free to download and use for learning the documented workflows; redistribution of the
binaries into an open-source repo is NOT granted. => reference / observe only.

### 2.2 5axes piston sample (independent author sample)

The 5axes personal CAD/CNC tutorial site publishes a piston model in CGR for free
download as a format-evaluation sample (piston_cgr.zip, piston.cgr, 188 KB).
Site terms allow personal non-commercial / educational use with attribution; no
explicit redistribution grant for a shared repo; the CGR format itself is
proprietary. => reference / observe only; contact author before any sync.

### 2.3 Located but not independently downloadable

- CUBES.cgr: referenced by Dassault CAA V5 documentation (CAAVisBasics use case).
  The binary ships only inside a licensed CAA RADE / CAAVisualization.edu install
  (resources/graphic/cgr/CUBES.cgr); not independently downloadable.
- GrabCAD "cgr" tag: per-uploader terms, account-gated, not auditable at scale.
- TransMagic sample pack: ships inside their install, vendor test/eval data.
- GitHub CATIA practice repos: CATPart/CATProduct study models, mostly no license
  field (all-rights-reserved by default); none contained a CGR in tree scans.
  Relevant to P1-M1-T03 (CATPart/CATProduct), not this CGR task.

## 3. Verification on real material

11 real CGR files were downloaded and verified locally on 2026-10-15:

- 10 DMU tutorial samples (ATOMIZER...VALVE) from cfysa.zip (sual.narod.ru).
- 1 piston.cgr from 5axes piston_cgr.zip.

Identity signals observed on all 11 files:
- Shared 8-byte binary prefix `01 00 00 00 00 00 00 00` (0x0100000000000000).
- piston.cgr (R11-era) additionally contains internal CAT type strings
  `CAT3DBagRep`, `CATSurfacicRep`, `BagRep`.
- The 10 DMU samples are denser binary with no ASCII string table in the first
  64 KiB (consistent with a cache-style R18-era representation). Their exact
  header/block decoding is P1-M2-T01 scope; this task only records identity evidence
  and checksums so later tasks can confirm provenance.

sha256 digests, sizes, and observed headers are recorded for every file in
`workstreams/format-corpus/samples/cgr-sample-registry.json`. No binary file is committed to this repo.

## 4. License discipline (the rule we keep)

Program rule: no proprietary redistribution. Concretely:

1. A sample may be marked `redistribution.allowed = true` ONLY under an explicitly
   permissive/open license (CC0, CC-BY, MIT, Apache-2.0, BSD, GPL, LGPL, public
   domain), with the verdict carried in the registry.
2. Every proprietary / unlicensed / author-restricted sample must be `reference-only`
   with a recorded reason, exactly as done above.
3. The identity record (observed header + sha256) is what lets a later task confirm
   it is looking at the same bytes the registry describes, without the repo carrying
   the proprietary content.

This discipline is enforced by `tests/test_cgr_registry_acceptance.py` (TDD: written
before the registry; it fails if any sample is marked redistributable under a
proprietary license, or if required evidence fields are missing).

## 5. Handoff to downstream tasks

- P1-M1-T04 (corpus manifest): consume this registry as the CGR segment.
- P1-M1-T05 (LFS/object-store sync): for these samples, sync = record source URLs +
  evidence; DO NOT publish proprietary binaries. Seed a CGR corpus only from
  explicitly-permissive sources or owner permission.
- P1-M1-T06 (privacy/ITAR sweep): apply the same evidence fields; these tutorial
  sample parts (atomizer, valve, piston) are generic mechanical teaching models with
  no sensitive provenance.
- P1-M2-T01/T02 (hex notebook, version drift): the observed header prefix + the
  R11-vs-R18-era difference (string table present vs absent) are initial data points
  for the CGR version-drift comparison.
- The clean-room corpus policy means the P2 CGR reader should be developed against
  these observed formats using source-URL pinning; binaries live in a private
  observation cache (NAS, not the shared repo) if and when ownership/permission is
  established.

## 6. Sources

| Sample set | Source | License verdict |
| --- | --- | --- |
| DMU Space Analysis tutorial CGRs (10 files) | sual.narod.ru CATIA V5 R11 doc tree (cfysa.zip); cross-ref kev009 / City.ac.uk / anarkia333 / catiadoc mirrors | Dassault tutorial sample, proprietary, observe-only |
| piston.cgr | 5axes.free.fr Format Echange page (piston_cgr.zip) | author-published educational, no OSS grant, observe-only |
| CUBES.cgr | CAA V5 docs (maruf.ca); ships in licensed CAA install | proprietary, not downloadable |
| GrabCAD cgr tag | grabcad.com/library/tag/cgr | per-uploader, account-gated, not auditable |
| TransMagic sample pack | transmagic.com/free-sample-cad-files | vendor test data, not OSS |
| GitHub CATIA repos | github.com/topics/catia-v5 | mostly unlicensed CATPart/CATProduct, no CGR found |

All registry entries, evidence URLs, and checksums: `workstreams/format-corpus/samples/cgr-sample-registry.json`.
