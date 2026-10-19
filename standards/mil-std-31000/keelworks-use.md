# MIL-STD-31000 — Keelworks Use

## Why Keelworks cares

Keelworks' stated wedge is the "small defense subcontractor shop" (P14 reference
vertical). Real defense deliverable contracts are governed by MIL-STD-31000 TDP
requirements — the TDP Option Selection Worksheet decides what data formats a vendor
must hand over. If Keelworks cannot produce a compliant TDP (or at least the design/
3D-data portions of one), it cannot win or satisfy those contracts. This standard
defines the *customer-visible packaging contract* that P4 exports and P8 PDM must
serve.

## Workstream mapping

- **P14-M1-T01 — Reference vertical spec** — the "small defense subcontractor"
  scenario includes releasing a TDP-shaped package (part, assembly, drawing, render,
  share). MIL-STD-31000 defines the shape of "share".
- **P14-M2/P14-M3 (demo + dogfood)** — a scripted demo can show assembling a
  TDP-style export from oep-step output and PDM revisions.
- **P4-M3 (STEP export)** — since DoD TDPs commonly require neutral CAD formats,
  Keelworks' STEP output must be packaged per TDP-level/type expectations.
- **P6-M4 (exporters)** — STL, STEP, glTF, OBJ, PDF/3D-PDF outputs feed the data
  elements a TDP lists; 3D PDF is explicitly a TDP-option format.
- **P8 (Self-hosted PDM)** — the revisions/items/maturity model maps naturally onto
  TDP data management products (authoritative real, not a separate schema need, but
  the vocab aligns).

## What we take from the standard (observational, license-safe)

- **TDP elements / levels / types taxonomy**: the checklist of what a deliverable
  data package contains and how complete it is. This will inform a "TDP checklist"
  export feature and our demo scenario.
- **Option Selection Worksheet semantics**: which item/format options (3D model,
  drawings, 3D PDF, PMI data, software docs) a customer can select. Directly informs
  the format matrix (P13-T03) and honesty about capability coverage.
- **Rev C additive-manufacturing extension**: signals a design-data surface Keelworks
  should support (mesh/STL/AMF/B-Rep inputs for printed parts).

## Risks / notes

- MIL-STD-31000 is public domain (Distribution A, U.S. Government work): the full
  standard may be mirrored in-repo if ever needed, but we keep pointers to ASSIST as
  the source of record.
- It references ASME Y14.47 for 3D data formats; Y14.47 is itself an ASME (paywalled)
  standard — treat it like ASME Y14.41 (catalog pointer only) unless a free version
  surfaces.
- TDP compliance is a contract matter: Keelworks aims for "can populate a conformant
  TDP data set for the design/3D-data elements", not a claim of certification, which
  remains the customer's responsibility.
