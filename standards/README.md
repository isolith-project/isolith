# Isolith Standards Reference

Shared reference materials for the standards that define Isolith' interoperability
surface. This area is a **catalog of pointers and summaries, not a library of
copyrighted documents**: only public specs, official URLs, and brief fair-quoted
extracts (marked with `>` and attributed) live here. Full standards remain at their
official sources; no vendor PDFs are vendored in-tree.

## Standards

| Standard | What it is | Isolith consumer |
| --- | --- | --- |
| [AP242 (ISO 10303-242)](ap242/README.md) | Managed model-based 3D engineering: the STEP application protocol that merges AP203 (aerospace) and AP214 (automotive); carries geometry, topology, and semantic PMI. | P4 STEP interop, `oep-step`; PMI import/re-export; P4 MBD fidelity audits. |
| [QIF (ISO 23952)](qif/README.md) | Quality Information Framework: free XML framework standard for metrology/quality data (measurement plans, results, PMI), ANSI QIF 3.0 harvested to ISO 23952:2020. | P4-M4 QIF import/export; P6 PMI model; P4-M5 MBD validator data plumbing. |
| [MIL-STD-31000](mil-std-31000/README.md) | Technical Data Package (TDP) standard: defines the elements, levels, and types of an authoritative technical description of an item for US DoD acquisition. | P4 deliverables for defense customers; P8 PDM packaging; P14 reference vertical. |
| [ASME Y14.41](asme-y14.41/README.md) | Digital Product Definition Data Practices: the MBD practice standard for presenting products as 3D data sets (paywalled ASME document; only its public scope is quoted here). | P4-M5 MBD validator rule set; P6 PMI presentation model; Y14.41 rule checks. |

## Discipline

- **Public specs only.** Anything redistributed in full must be licensed for
  redistribution (e.g., US Government works like MIL-STD-31000). Copyrighted
  standards (ISO 10303-242, ASME Y14.41, ISO 23952) stay at their official sources;
  we record URLs, editions, and license status, and quote only short attributed
  fragments.
- **One standard, one directory** under `standards/`, each with `README.md`
  (metadata + scope), `isolith-use.md` (workstream mapping), `references.md`
  (official URLs).
- **Verify on real material.** The acceptance test `tests/standards_test.py` asserts
  structure, metadata completeness, fair-quote budget, and URL well-formedness; a
  live link check confirms the recorded official sources actually resolve.

## Cross-cutting notes

- AP242 and ASME Y14.41 complement each other: Y14.41 says *how* product definition
  is presented digitally; AP242 is the *machine-readable carrier*.
- QIF and AP242 both carry PMI; QIF is the quality/metrology-side twin (used for
  digital thread in a Model-Based Enterprise), AP242 the design-side carrier.
- MIL-STD-31000 governs the *deliverable package*, so it shapes how `oep-step`,
  `oep-pdm`, and the P14 reference vertical must export and organize data for DoD.
- License/redistribution details per standard are in each README's
  "License / redistribution status" section. See the P0-M2 legal workstream for the
  program-level clean-room posture that bounds all of this.
