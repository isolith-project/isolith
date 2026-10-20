# ASME Y14.41 — Digital Product Definition Data Practices

## Standard

ASME Y14.41, *Digital Product Definition Data Practices*. This American national
standard establishes requirements and reference documents for the preparation and
revision of digital product definition data (data sets), and defines exceptions and
additional requirements to existing ASME standards for using product definition
digital data sets or drawings in digital format. It is the foundational MBD practice
standard: how a 3D model plus its annotations ("product definition data") replaces
or accompanies a 2D drawing.

## Publisher

American Society of Mechanical Engineers (ASME), Y14 Engineering Product Definition
and Related Documentation Practices committee. Published and sold by ASME; sold under
the ASME Y14 standards series.

## Current edition

- Y14.41-2026 (current; ISBN 978-0-7918-7782-1, 161 pages) — this is the edition
  ASME currently lists for sale.
- Earlier: Y14.41-2019; original edition 2003. It is regularly reaffirmed and is
  closely tied to ASME Y14.5 (GD&T).

## Availability

- **Paywalled, copyrighted**: available for purchase (~USD 240 list for digital,
  per ASME store). Digital products are restricted to one per purchase.
- ASME posts only the public scope/abstract on its product page; the full body is
  behind purchase. Any open "preview" material is minimal.
- No free official full text exists; this repo records the pointer and the public
  scope only (fair quote below).

## License / redistribution status

- **ASME copyright.** Do NOT vendor any part of Y14.41 (beyond brief fair quotation,
  marked `>` with attribution) into this repo. Purchase from ASME is the only
  authorized full access.
- The Action Engineering MBE Dictionary publishes individual term definitions citing
  Y14.41, which may be linked as secondary public material (fair use) but not bulk
  copied.

## Official source

- ASME Y14.41 product page:
  https://www.asme.org/codes-standards/find-codes-standards/y14-41-digital-product-definition-data-practices
- ASME Y14 standards collection:
  https://www.asme.org/codes-standards/y14-standards

## Scope

From the ASME product page public abstract (fair quote):

> This Standard establishes requirements and reference documents applicable to the
> preparation and revision of digital product definition data, hereafter referred to
> as data sets. This Standard defines exceptions and additional requirements to
> existing ASME standards for using product definition digital data sets or drawings
> in digital format. Where no exception or additional requirements are stated,
> existing ASME standards shall apply.

Y14.41 covers model-only (fully annotated 3D, i.e., true MBD) and model-plus-drawing
data sets, detailing how geometry and its associated requirements (GD&T per Y14.5,
notes, tolerances, datums) are presented and queryable in the digital model.

## PMI / MBD relevance

- Y14.41 is the **practice** standard behind MBD: it defines what makes a model
  "product definition". Isolith' P4-M5 MBD validator's rule set is squarely
  Y14.41-shaped (missing PMI, dangling datums, model completeness, units, annotations
  attached to the right geometry).
- It complements AP242: Y14.41 defines the human/business MBD practice; AP242 defines
  the machine-readable carrier. A robust Isolith validates Y14.41 conformance over
  AP242 data.
- The master plan's P4-M5 explicitly lists "Y14.41, prime requirements" as the rule
  basis for the validator service.
