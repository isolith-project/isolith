# QIF — Quality Information Framework

## Standard

ANSI QIF 3.0 (also ISO 23952:2020), *Automation systems and integration — Quality
information framework (QIF) — An integrated model for manufacturing quality
information*. QIF is a unified XML framework standard for computer-aided quality
systems: measurement plans, results, part geometry and PMI, measurement templates,
resources, and statistical analysis.

## Publisher

- Digital Metrology Standards Consortium (DMSC), a non-profit consortium of metrology
  and manufacturing technology companies. QIF 3.0 was designated an American National
  Standard (ANS) and in August 2020 harvested, approved, and published by ISO as
  ISO 23952:2020.
- ISO entry: ISO 23952:2020 (ISO/TC 184/SC 4, Industrial data). The ISO sales text is
  copyrighted; the QIF standard itself is freely available from DMSC for implementers.

## Current edition

- QIF 3.0 (December 2018), ANSI-recognized; identical content harvested as
  ISO 23952:2020. It is the current published version (QIF GitHub community and DMSC
  remain the point of distribution).
- QIF retains a family of sub-models (MBD, Plans, Results, Resources, Statistics,
  etc.) defined by an XML Schema (XSD) library.

## Availability

- **Free to all implementers** via DMSC download (registration form at
  qifstandards.org/download). No purchase required.
- All QIF XML schema files are published at www.qifstandards.org; instance data and
  community tools live in the QIF Community on GitHub
  (https://qualityinformationframework.github.io).

## License / redistribution status

- The QIF standard and its XML schemas are **freely available for use and
  redistribution by implementers under DMSC's terms** (free XML framework standard;
  "available free to all implementers"). DMSC's own license terms govern — see the
  download page and the schemas' headers.
- The ISO 23952 PDF sold by ISO is ISO-copyrighted; prefer the DMSC free PDF/XSD set
  for development use.
- Clean-room posture applies as for all standards: observational use, no copying of
  vendor implementation code.

## Official source

- https://qifstandards.org (main DMSC portal)
- https://qifstandards.org/download (free QIF 3.0 download)
- https://www.iso.org/standard/77461.html (ISO 23952:2020 product page)
- https://qualityinformationframework.github.io (QIF Community / open tools)

## Scope

From the ISO 23952 abstract (fair quote):

> This document describes the general content and structure of the entire QIF
> information model. It describes the highest level data structures of QIF ...
> All QIF XML schema files can be found at www.qifstandards.org.

From the DMSC "About QIF" page (fair quote):

> QIF (Quality Information Framework) is a standard which supports Digital Thread
> concepts in engineering applications ranging from product design through
> manufacturing to quality inspection. Based on XML, the QIF standard contains a
> Library of XML Schema ensuring both data integrity and data interoperability in
> Model Based Enterprise implementation.

QIF therefore covers the metrology/quality side of the Model-Based Enterprise:
sending measurement plans and PMI downstream, collecting inspection results, and
feeding statistics back up the digital thread.

## PMI / MBD relevance

- QIF is the XML counterpart to AP242: it carries product geometry + PMI (the MBD
  model) but specializes in **quality workflow** (plans, results, statistical
  analysis), enabling reuse of design PMI for inspection.
- P4-M4-T01 targets QIF import/export (MBD quality data) per the master plan.
- Interoperating with QIF means Keelworks can participate in the metrology digital
  thread and, via MIL-STD-31000-style TDPs, deliver inspection-ready data packages.
