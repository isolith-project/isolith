# Located public CATPart/CATProduct sample sources — P1-M1-T03

Version 1.0, 2026-10-15.

Task: locate public CATPart/CATProduct samples (education packs, sample vaults
from tutorials) and verify file identity (magic `V5_CFV2`) per file. This file
is the located-source inventory; the compact repo-backed subset committed under
`corpus/data/catpart/` is registered in `corpus/catpart-samples.json`.

## Magic identity rule (applied per file)

A real CATIA V5 `.CATPart` / `.CATProduct` starts with the 8 bytes
`56 35 5F 43 46 56 32 00` = ASCII `V5_CFV2` then a NUL, i.e. the "CATIA V5
Container File Version 2" outer header. Confirmed by:

- PRONOM x-fmt/439 (magic bytes list published for CATPart)
- cadmpeg project format notes (CC-BY-4.0), docs/formats/catia.md §3.1
- Direct hex inspection on every file below (this task).

The same 8-byte magic appears at the start of `.CATPart` and `.CATProduct`
files and of `.CATShape`, `.cgr` (a CGR may be wrapped differently) — identity
must be checked per file, never assumed from the extension.

## Verified sources

### 1. mohanevs/solid-modeling-catia-v6 (GitHub, MIT) — committed subset

- URL: https://github.com/mohanevs/solid-modeling-catia-v6
- License: MIT (verified via GitHub license API, 2026-10-15). Redistribution
  permitted with attribution.
- Content: 121 `.CATPart`/`.CATProduct` files — 57 standalone 3D-model parts,
  7 full assemblies in per-assembly folders (screw jack, knuckle joint, pipe
  vice, plummer block, press tool, tool head of shaper, motor blower) each with
  all of its part files, plus a few part/thread/plane fixtures (star, hex nut,
  square thread, class plane, sphere, boat, etc.).
- Identity check: **121/121 files start with `V5_CFV2`** — full set downloaded
  to scratch and hex-checked per file, 2026-10-15. 114 `.CATPart`, 7
  `.CATProduct`.
- Committed to this repo: compact representative subset (10 files, 1.9 MB) =
  the complete knuckle-joint assembly (1 CATProduct + 6 CATParts: collar, eye
  end, fork end, pin, taper pin) + 4 standalone parts (3DM016, hex-nut,
  square-thread, star). Repo-backed per the corpus harness ADR (small fixtures
  committed; the full 121-file / 30 MB set belongs on the NAS object store via
  P1-M1-T05). The full set's sha256 manifest can be regenerated from the
  upstream GitHub tree.
- Generation/origin: "independently designed and rendered in CATIA", inspired
  by standard mechanical components from engineering drawing textbooks. Not
  derived from Dassault sample data; user-authored. Age/version: V6-era files
  (CATIA V6 / 3DEXPERIENCE-compatible tooling); container magic is still
  `V5_CFV2` (V6 writes the V5-compatible container for these saves).

### 2. City, University of London — public CATIA V5 teaching vault (education pack, recorded only)

- URL (asmug_C2 = Assembly Design User Guide samples):
  https://www.staff.city.ac.uk/~ra600/ME2105/Catia course/CATIA Tutorials/asmug_C2/samples/
- Parent: https://www.staff.city.ac.uk/~ra600/ME2105/ (ME2105 course materials,
  CATIA V5R11-era tutorials, hosted since 2007 on the academic staff site).
- Content: the Dassault "User Guides" sample packs used for the course —
  `asmug_C2` (assembly), `prtug_C2` (part), `draug_C2` (drafting), `driug_C2`,
  `estug_C2`, `kwrug_C2`, `wfsug_C2` (wireframe/surface), each a directory of
  `.CATPart`/`.CATProduct` training files (e.g. `Analysis.CATProduct`,
  `BluePart.CATPart`, `BulkheadA/B/C.CATPart`, `CATASMDisk.CATPart`,
  `MoldedPart.CATPart`, `Stiffener.CATPart`, `Support.CATPart`, `Pad.CATPart`,
  `GreenPart.CATPart`, `Constraint1..7.CATProduct`, `chain.CATProduct`,
  `link.CATPart`, `bottom_plate.CATPart`, `top_plate.CATPart`, ...).
- Identity check: **72/72 files from the asmug_C2 samples dir start with
  `V5_CFV2`** — full directory downloaded and hex-checked per file,
  2026-10-15. Results cached in scratch (`city-magic.json`).
- License: public academic hosting on the staff website for a taught course,
  no explicit license/CSP statement on the page. **Not committed to this
  repo** — redistributing Dassault-created User Guide sample files would
  violate the clean-room "no proprietary redistribution" invariant. These are
  LOCATED + identity-verified but must be treated as observation-only reference
  material, not corpus payload. If a redistribution-approved copy is ever
  needed, it requires a written license/teaching-exception and P0 legal signoff
  (see P0-M2-T01 legal review).

### 3. Schroff Development Corp. workbook samples (education pack, recorded only)

- URL: http://www.schroff1.com/catia/1-58503-321-9.htm ("Advanced CATIA V5
  Workbook", Richard Cozzens, SDC Publications).
- Content: free downloads of workbook part/assembly sample packs — CaseBracket
  (R16), EngineAssembly.zip, SliderAssembly.zip, U-jointAssemblyTrace.zip,
  TestHouse1..3.zip, ArborPressParts.zip, ArborPressAssembly.zip.
- License (stated on page): "You are welcome to use CATProducts and/or
  CATParts in part or whole as long as there is no charge (cost) for the
  finished product! All the CATParts and CATProduct files are the property of
  Schroff Development Corporation." — use is explicitly permitted with no
  charge for the finished product; files remain SDC property. Not committed
  here pending a decision on whether "no charge for finished product" is
  compatible with storage in a shared open-source corpus; recorded as a
  located candidate for P0/P1 legal review.

### 4. Additional located but unverified/low-value candidates (recorded only)

- `editablefiles.com/download-catia-editablle-files-for-free` — claims 100+
  free CATIA files (CATDrawing/CATPart/CATProduct/CDDX). No stated license;
  untrusted distribution channel. Not downloaded.
- `transmagic.com/free-sample-cad-files/` — advertises CATIA V5 CATPart/
  CATProduct sample files shipped inside the TransMagic installer (Windows,
  Public Documents\TransMagic\Sample Files). Proprietary vendor samples; not
  redistributable. Not downloaded.
- Moodle "Université Numérique" kinematics course (moodle.luniversitenumerique
  .fr) hosts French-language CATIA V5 kinematic mechanism files
  (COMPRESSEUR.CATProduct, FILAIRE_*, P11..P24.CATPart, ...). Requires Moodle
  login; academic hosting, license unclear. Not downloaded.

## Mismatch / false-extension observations (feed P1-M2-T03)

- Several "full-set" HTTP 404 bodies were 14-byte "404: Not Found" text — a
  reminder that extension alone is not identity; the magic check catches
  non-container files. The downstream corpus runner (P2-M5-T03) must classify
  these as SKIP, never crash.

## Decisions / handoff

- Committed subset: compact (1.9 MB) and MIT-clean → safe for a shared repo,
  hermetic acceptance test, CI clone.
- Full 121-file mohanevs set (~30 MB) → NAS object store target for P1-M1-T05
  (corpus LFS/object-store sync), entries to be folded into the single
  `corpus/manifest.json` by P1-M1-T04.
- City Uni / SDC education packs → located, identity-verified, but redistributed
  only after P0 legal review (clean-room invariant). Recorded here so P1-M2
  notebooks can reference them for observation without committing payload.
- Geometry class for parts is a best-effort label from set context (see
  catpart-samples.json descriptions); confirm on real read in P1-M2.
