# JT corpus (P4-M4-T03 wedge; dev-only, decision-7 posture)

Catalogued under the corpus dev-only reference posture: these files are the
program's own synthesized bytes, used to pin the reader's decode behavior.
They are never shipped and are not redistributable third-party material.

## Files

- golden-box-a.jt (golden): a JT file assembled from the documented byte
  layout by python/oep_tooling/jt_fixtures.py. Carries one LSG block
  (partition -> part node "golden-box-a", a string property atom providing
  the name, a late-loaded property atom referencing a Shape LOD0 block) and
  one Shape LOD0 block containing a Primitive Set Shape Element with a
  single Box primitive (Params: min corner (0,0,0), side lengths (1,2,3),
  identity orientation). Decodes to 8 vertices / 12 triangles, bounding
  volume 6.0.
- Corrupt variants (each exercises a distinct typed-error path, never a
  panic):
  - empty.jt (0 bytes -> NotJt)
  - junk-not-jt.bin (no version marker -> NotJt)
  - truncated-header.jt (40 bytes -> NotJt/Truncated)
  - bad-byte-order.jt (byte order 3 -> BadHeader)
  - toc-offset-oob.jt (directory offset past EOF -> BadDirectory)
  - segment-overrun.jt (block length overruns file -> BadStructure)
  - root-not-lsg.jt (root block kind 99 -> BadDirectory)

## Provenance

- Synthesizer: python/oep_tooling/jt_fixtures.py (run at build time to
  regenerate; bytes are committed so the golden is a stable pin).
- Spec basis: every field is laid out per Siemens JT v10 File Format
  Reference Rev-B sections 5.1.1 / 5.1.2 / 5.1.3 / 6 / 7.2 and JTIAP PSI
  14-1 V3; see docs/re-notes/oep-jt-container-and-lsg.md for the
  field-by-field citations.
- License: our own authored bytes, Apache-2.0 by the program posture.
  No third-party JT file is contained or redistributed.
