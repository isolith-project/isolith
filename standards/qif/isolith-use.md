# QIF — Isolith Use

## Why Isolith cares

QIF gives Isolith the quality/metrology side of the Model-Based Enterprise. While
AP242 is the design-side carrier of PMI, QIF is how that PMI flows into inspection
plans, results, and statistics. A Isolith that can emit and consume QIF closes the
digital thread instead of stopping at the CAD model.

## Workstream mapping

- **P4-M4-T01 — QIF import/export basic (MBD quality data)** — the direct consumer
  card: read/write QIF documents per the QIF standard documents and XSD schemas.
- **P4-M4-T02 — JT import decision** — QIF coexists with JT for visualization/PMI;
  the P4-M4 milestone covers both quality data and visualization-oriented PMI.
- **P4-M5 (MBD validator)** — a supplied model/drawing validated against rule sets;
  QIF MBD models are one validation input shape.
- **P6-M1-T02 (PMI model schema)** — QIF's representation of geometry + PMI informs
  Isolith' internal PMI model so both AP242 and QIF can map onto it.
- **P14 reference vertical** — the "small defense subcontractor" scenario includes
  quality data exchange; QIF capability rounds out the demo.

## What we take from the standard (observational, license-safe)

- **XSD schema structure**: QIF's XML schemas define the information model; using the
  free downloadable XSDs is explicitly the intended implementer path (DMSC publishes
  them for this purpose). We read the schema to design our own internal quality/PMI
  model and QIF exporter.
- **Sub-model partitioning** (MBD, Plans, Results, Resources, Statistics): informs the
  internal data model layering.
- **Instance document conventions**: how QIF "documents" (XML instances) are formed,
  per the standard's own guidance, to write valid output.

## Risks / notes

- Respect DMSC's license terms: the QIF standard and XSDs are free for implementers,
  but their license governs redistribution. Link to official downloads; do not
  re-host the full standard bundle in this repo without recording the license.
- ISO 23952:2020 text sold by ISO is copyrighted; prefer the free DMSC distribution
  for development.
- QIF 3.0 is current; be alert to version drifts between the ANSI and ISO designations
  (content is identical per DMSC, but the edition labels differ).
