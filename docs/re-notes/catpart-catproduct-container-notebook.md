# CATPart vs CATProduct container: differences and reference-graph discovery

Format: Hex-level reverse-engineering observation notebook
Task: P1-M2-T03 (Keelworks wave 1, P1 format reconnaissance)
Version: 1.0
Date: 2026-10-15
Status: verified on real material (121 real files, see "Verification record")

Owner: Keelworks P1 corpus / RE lane. Companion notebook: docs/re-notes/cgr-hex-notebook.md (P1-M2-T01, CGR). Shape-record hypothesis carryover: P1-M2-T05.

---

## 1. Scope, method, clean-room

Goal: document, from byte observation alone, the container-level differences
between CATPart files (mechanical part documents) and CATProduct files
(mechanical product/assembly documents), and how the product-to-part
reference graph is stored. This feeds the P5 native reader feasibility and the
P6 importer hub: a reader must know the root container type before it can walk
blocks, and the graph edges tell it where children live.

Method: pure observation, no vendor tools, no Dassault libraries. The
observation commands used (Python stdlib: read 8-byte magic, scan printable
strings with offsets, read little-endian u32 header words, corpus-wide value
histograms) are reproduced in section 8 so every table here is re-derivable.

Clean-room invariant (board.program.json): no proprietary binary is committed
to this repo. This notebook records evidence (hex dumps, offsets, string
tables, counts, hashes). The sample binaries stay on the observation machine
and in the P1-M1 corpus registry with provenance and license records.

## 2. Corpus used

- 121 files: 114 .CATPart + 7 .CATProduct, verified identity (magic V5_CFV2)
  per file before analysis.
- Source: mohanevs/solid-modeling-catia-v6 (GitHub, tutorial/education pack of
  computer-aided design models), LICENSE = MIT (Copyright (c) 2026 Vaibhavsingh
  Mohane). Flattened here as "3D Models__3DM0NN.CATPart",
  "Assembly__N.<name>__<part>.CATPart" and
  "Assembly__N.<name>__@<name> assembled.CATProduct".
- Every file carries the writer marker "V5R29SP0HF0" (CATIA V5 R29 SP0 HF0),
  121/121. P1-M2-T02 handles version drift separately; this corpus is one
  writer, which makes the container differences intrinsic, not version noise.
- Assembly sub-sets observed: screw jack (1), knuckle joint (2), pipe vice (3),
  plummer block (4), press tool (5), tool head of shaper (6), motor blower (7).
  The 3D Models and SM Application parts are standalone (not referenced by any
  product in the pack); motor-blower parts carry a distinct header batch (sec 6).

## 3. Shared container anatomy (both families)

Everything in this section holds on 121/121 files.

### 3.1 Magic and string idiom

- Bytes 0x00..0x07: "V5_CFV2" + NUL (56 35 5f 43 46 56 32 00) on every file.
- Strings are LENGTH-PREFIXED, not NUL-terminated: a string value is preceded
  by a single byte giving its ASCII length (observed: prefix byte == strlen + 1;
  the +1 likely counts the terminator that follows, see below).
  Evidence (offset-annotated dump from the knuckle-joint product):
      0x32a7 ... _Version (+len) ... "fork end"
      0x32af 0x0e "_InstanceName"   (_InstanceName = 13 chars; prefix 0x0e = 14)
      0x32bc 0x0b "fork end.1"      (10 chars; prefix 0x0b = 11)
      0x32c8 0x07 "PRDRE"           ("PRDREP" = 6 chars; prefix 0x07 = 7)
  Same idiom on the root token, see 3.3.
- The ">" and "|" bytes seen around key/value pairs in dumps are field framing
  bytes, not separators in the ASCII sense; treat them as opaque [uncertain].

### 3.2 Header token block (0x00..0x68)

Read as little-endian u32s. Values observed (L = standard batch, 109 parts +
7 products = 116 files; X = 5 motor-blower parts, see section 6):

    offset  meaning (observed)                    value(s)
    0x00    magic "V5_CFV2\0"                     56 35 5f 43 46 56 32 00   (121/121)
    0x10..17 0xFF fill                              ff ff ff ff ff ff ff ff  (121/121)
    0x18..3b zero fill                              00 x 36                (121/121)
    0x40    fixed token                             L: 0x01ACF45E (116/121), X: 0x00
    0x44    u32 == 1                                L: 1, X: 0xE10DFFFF
    0x48    u32 == 0xF0 (240) (candidate: header size)  L: 240, X: 0x464A1000
    0x4c    u32 == 2                                L: 2
    0x50    u32 == 2                                L: 2
    0x54    u32: PRODUCT == 3 (7/7); PART in {30, 0x5B00}   <-- family-correlated, sec 5
    0x58    u32 == 0x620 (1568)  (candidate: stream/fixed flag)  L: 1568, X: 0x83FFFFFF
    0x64    u32 variable (see 5.2)                  parts {148,160,172,7670}, products {1779..5009}
    0x3e..0x3f  "0d 0a" CR/LF followed by 0x00     end of the first text token, both families

The tokens at 0x40/0x44/0x48/0x4c/0x50/0x58 are NOT family discriminators
(they agree between parts and products in the same batch); they are
file-format constants (or per-save settings) shared by both containers.

### 3.3 Root product token (both families)

Every file, part or product, carries exactly one root-product token:

    "ASMPRODUCT" + <1-byte length> + <root name>

Length-prefix uses the strlen+1 idiom from 3.1. Observed:

    pin.CATPart                         ASMPRODUCT 0x04 "pin"
    @knuckle joint assembled.CATProduct ASMPRODUCT 0x0d "knuckle joint"

So a CATPart is itself a product root with a one-child product structure;
"ASMPRODUCT" is not a part/product discriminator. The root name for parts is
the part's own name; for products the assembly name.

### 3.4 Catalog registry and provenance

- "CATCatalogManager" / "catalogLinks" registry block appears in both families
  (parts counted 3x on the pin part; products 4x on the knuckle product). This
  is the document-to-library link table (where parametrized catalogs would
  point) and is present in both containers.
- Writer provenance is exposed as a "CATIAV5R29" / build-level block:
      <HotFix>0/<HotFix> ... MinimalVersionToRead ... CATIAV5R29 ... CATBuildLevel ... 10-11-2018.20.00
  plus the "V5R29SP0HF0" marker (121/121). Useful as a writer fingerprint; it
  describes the SAVING installation, not the file's spec revision.

## 4. The container difference (CATPart vs CATProduct)

### 4.1 Class-string discriminator: the decisive byte-level signal

The two families are perfectly separated by which class strings their blocks
register. No strings were guessed: these are raw ASCII tokens found in byte
scans, with corpus-wide counts:

    token              in CATPart (114)       in CATProduct (7)
    MechanicalPart     114/114 (228 hits)     0/7
    CATMechProdCont    0/114                  7/7  (28 hits)
    CATPart (class)    114/114 (342 hits)     0/7     <-- product never self-labels CATPart
    CATProduct (class) 0/114                  7/7 (14 hits)  <-- part never labels CATProduct
    FromCATPart        parts                   0        ("is-root-from-CATPart" marker)
    ASMGEOMETRY        ~none                   7/7 (assembly geometry bag)

Interpretation (observed + safe reading): a CATPart registers a
"MechanicalPart.PartN" mechanical body model (with a feature container,
default representation "Shape 1"). A CATProduct registers a
"CATMechProdCont" (mechanical product container) whose children are product
instances of the mechanical parts. The container's own member/authoring
class strings ("CATPart" vs "CATProduct") match the file extension, so the
type is in the bytes, not just the name.

### 4.2 Header discriminator: u32@0x54

    CATProduct: u32 at 0x54 == 3  on 7/7
    CATPart:    u32 at 0x54 in {30, 0x5B00}     (109 + 5 files; never 3)

The semantic of this field is [uncertain] (not the instance count: the
4-child knuckle joint product still reads 3). It plausibly counts top-level
container sub-blocks (document, product structure, geometry) or is a graph
version token. Recorded as a discriminator, meaning undetermined.

A second header-region family correlation: u32@0x64 is small in parts
({148,160,172,7670}) and larger in products ({1779,1864,2098,2437,2812,2999,
5009}); candidate early section size/offset token [uncertain].

### 4.3 Side-by-side header (0x00..0x5f)

    off   pin.CATPart (part)              @knuckle joint.CATProduct (product)
    0x00  56 35 5f 43 46 56 32 00 00 02 31 ff 00 00 29 04   56 35 5f 43 46 56 32 00 00 00 ba 51 00 00 18 64
    0x10  ff ff ff ff ff ff ff ff 00 ... 00                 ff ff ff ff ff ff ff ff 00 ... 00
    0x30  ... 00 02 5f 0d 0a 00 5e f4 ac 01 01 00 00 00      ... 00 02 5f 0d 0a 00 5e f4 ac 01 01 00 00 00
    0x40  5e f4 ac 01 01 00 00 00 f0 00 00 00 02 00 00 00   5e f4 ac 01 01 00 00 00 f0 00 00 00 02 00 00 00
    0x50  02 00 00 00 1e 00 00 00 20 06 00 00 00 00 00 00   02 00 00 00 03 00 00 00 20 06 00 00 00 00 00 00
                                          ^--- u32@0x54 = 30 (part)    ^--- u32@0x54 = 3 (product)

The only header difference between these two specific files is inside the
0x54..0x57 word (30 vs 3) plus the size-ish 0x64 region. Everything else in
0x00..0x5f agrees.

## 5. Reference-graph discovery (how a product links its parts)

This is the part of the notebook that matters for a reader: where are the
edges, and are they file paths or logical references?

### 5.1 What a CATProduct stores (composition graph)

A product holds, at the byte level:

1. Child product identities (logical names): for the knuckle-joint product
   the tokens "fork end", "eye end", "pin", "taper pin" appear as bare names.
2. Instance occurrence labels: "_InstanceName" + length-prefixed "<name>.<n>"
   ("fork end.1", "pin.1", ...). The ".1" is the first-occurrence counter
   (V5 names subsequent copies .2, .3 ...).
3. Representation attachment edges: each instance carries a rep entry
   "_Position PRDREP 'Shape 1'" and, in the noticeably parallel byte runs,
   "CtorToProd:Product!ProdToRep:Shape 1!RepToShape:Shape!Geometry:<code>!"
   proxy blocks; "CtorToProd" and "ProdToRep" counts are equal per product
   (13..50) and scale with instance count. These are the edges from the
   product node to the child part's default representation "Shape 1".
4. Constraint edges live in the product, not the parts: "Coincidence.N"
   names (Coincidence.1, Coincidence.2, ...) and a "ConstraintASM" container.
   A product's constraints are therefore read from the product file.
5. An assembly-geometry bag ("ASMGEOMETRY"/"Product1_geom") attached to the
   root (observed on the knuckle product).

Observed per-product edge counters (object -> edge labels found):

    screw jack         body.1, cup.1, ...      Coincidence=11  ProdToRep=13
    knuckle joint      fork end.1, ...         Coincidence=8   ProdToRep=13
    pipe vice          base.1, ...             Coincidence=8   ProdToRep=14
    plummer block      casting.1, ...          Coincidence=16  ProdToRep=17
    press tool         bottom plate.1, ...     Coincidence=18  ProdToRep=17
    tool head of shaper vertical slide.1, ...  Coincidence=27  ProdToRep=50
    motor blower       lower housing.1, ...    Coincidence=9   ProdToRep=25

### 5.2 The notable negative in PRODUCTS: no child file paths

Across all 7 products (checked exhaustively):
- no ".CATPart" substring (no child filename with extension),
- no drive-letter paths (no "C:\..." or "<X>:/"),
- no relative path with separator, no UNC "\\server" string.

The only backslash bytes present anywhere in the product files are isolated
0x5c bytes inside binary streams, never inside a printable path-shaped run.
So: a CATProduct stores its edge to each child CATPart as a LOGICAL reference
(product name + instance name + rep name), and resolution of child CATPART
FILES happens at open time via search rules (same-folder/filename
convention), not from an embedded filesystem link. A reader/PDM that wants
the assembly tree walks logical names, not file paths.

### 5.3 Incidental absolute paths that DO appear (in PARTS, not products)

Parts are not empty of path strings: scanning the corpus for drive-letter
paths found two genuine path records and nothing else (the remaining
"<X>:/"-shaped hits are coincidental runs of IEEE-754 float bytes such as
"f:/@", verified byte-by-byte):

- 3D Models/3DM055.CATPart, offset 0x59e7f: a feature value literally
  "File name ... c:\image.jpeg ... Scale" -- an embedded picture/image asset
  reference (a document feature pointing at a local image file).
- SM Application/sphere.CATPart, offset 0x10cce4: "C:\Program Files\Dassault
  Systemes\B29\..." -- an absolute install-directory record (matches the
  V5R29SP0HF0 writer marker: B29 is the V5R29 program folder on the author
  machine's Windows drive).

Both are asset/library records inside PARTS, not edges to other CATParts, and
neither appears in any product. Implication for the reader: path strings can
appear in the part feature/catalog record space and should be parsed
defensively (they are payload, not graph structure), while product files in
this corpus carry none.

## 6. Documented variance (same writer, different header batch)

5 of the 114 parts (Assembly__7.motor-blower except upper housing) carry a
distinct header batch while still matching magic, MechanicalPart class string,
and the V5R29SP0HF0 writer marker:

    token      standard batch (109 + 7 + 1)   motor blower batch (5)
    u32@0x40   0x01ACF45E (or 0x00 on 5)       0x00
    u32@0x44   1                                0xE10DFFFF
    u32@0x48   240                              0x464A1000
    u32@0x54   30 (parts) / 3 (products)        0x5B00
    u32@0x58   0x620                            0x83FFFFFF

Same edition, same tool generation: the delta is per-save-state, not a
container-type difference. Consequences for a reader: (a) header tokens are
only safe to compare within a batch; (b) class strings + magic are the robust
identity anchor (they held 121/121); (c) do not treat 0x54=={30,3} as a
specification, treat it as observed batch statistics to be re-verified on
each new corpus (this is exactly what P1-M2-T06's save/modify/bisect harness
should pin down).

## 7. Open questions / uncertainties

- [uncertain] Semantics of u32@0x54 (3 vs 30) and u32@0x64.
- [uncertain] The "+1" in the string-length prefix: does it count a trailing
  NUL/space, or is the visible length byte strlen+1? Consistent across
  ASMPRODUCT names and attribute keys; terminator byte not yet identified.
- [uncertain] Whether a reference-heavy corpus (parts built with "Insert -
  External references", P5-M2-T02) introduces a second link mechanism (a
  "LinkWindow"/file reference table was NOT observed in any product here;
  absence may be a corpus property of this one-author pack, not a format law).
- [uncertain] The two incidental path records found in parts (3DM055 image
  asset, sphere install-dir) show parts can carry absolute paths as asset/
  catalog payload; whether that payload can ever point at another CATPart
  file (as opposed to an image/install folder) is not exercised by this
  corpus.
- [uncertain] How instance "Shape 1" rep name maps to a part's actual rep
  list (parts observed expose PRDREP "Shape 1" too, so the edge target is
  the part's default rep; multi-rep parts may differ).
- The 3DM/SM Application standalone parts were written by the same tool but
  exercise no assembly graph; they are not evidence about graph behavior.

## 8. Reproducibility and verification record

Observation commands (Python stdlib, as executed during this task):
1. identity: read first 8 bytes; expect b"V5_CFV2\x00" per file.
2. strings: re.finditer(rb"[\x20-\x7e]{4,}", data), record (offset, string).
3. header words: struct.unpack_from("<I", data, off) for the 0x40..0x68 grid.
4. corpus histograms: count value sets per word across all parts vs products.
5. path scan: per product assert no ".CATPart" and no "\b[A-Za-z]:[\\/]"
   regex hit; then scan ALL files for drive-letter hits and classify each
   (path record vs IEEE-754 float coincidence) by inspecting the bytes.
6. length-prefix check: for "ASMPRODUCT", next byte L in 1..40 then L bytes
   mostly printable (accepted: >= L-1 printable).

Acceptance test (TDD, written before the notebook):
- tests/test_catpart_product_notebook_acceptance.py
- RED first (notebook absent), then GREEN on real material.
- Run:  OEP_CATIA_SAMPLES=<dir> python3 tests/test_catpart_product_notebook_acceptance.py
- Verified 2026-10-15 on the 121-file wave-1 corpus:
  - magic 121/121; writer marker 121/121; MechanicalPart/CATMechProdCont
    exclusivity 121/121; CATPart/CATProduct class exclusivity 121/121;
    product u32@0x54==3 (7/7); part u32@0x54 in {30,0x5B00} (114/114);
    ASMPRODUCT length-prefixed root name 121/121; catalogLinks present;
    every product carries _InstanceName, CtorToProd/ProdToRep, Coincidence;
    no product contains a drive-letter path or a ".CATPart" child filename;
    parts may carry incidental asset/install path records (2 found:
    3DM055 image asset, sphere install dir) which are documented, not graph
    edges.
  - ALL CHECKS PASSED.

## 9. Version history

- 1.0 2026-10-15 initial observation notebook (P1-M2-T03).
