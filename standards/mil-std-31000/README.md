# MIL-STD-31000 — Technical Data Packages (TDP)

## Standard

MIL-STD-31000, *DoD Standard Practice: Technical Data Packages (TDP)*. It defines the
requirements for a technical data package and its related TDP data management
products, providing an authoritative technical description of an item, clear,
complete, and accurate in a form and format adequate for its intended use. A TDP
contains elements, is described by a level and type, and may have associated metadata
and supplementary data.

## Publisher

- United States Department of Defense (DoD). Maintained and distributed by DLA
  (Defense Logistics Agency) via the ASSIST (Acquisition Streamlining and
  Standardization Information System) QuickSearch repository of record.
- Because it is a U.S. Government work, it is in the public domain and freely
  distributable (unclassified, Distribution A — approved for public release).

## Current edition

- Current revision: **MIL-STD-31000C**, issued 3 February 2025 (no change notices
  as of this writing).
- Previous: Rev B (31-Oct-2018), Rev A (Feb-2013), initial MIL-STD-31000 (Nov-2009).
- Rev B moved the 3D-Data appendix to ASME Y14.47 and included the definition of
  "3Di" technical data (3D PDF); Rev C notably adds Additive Manufacturing to the TDP
  (per NIST MBE Summit 2024 briefing, the "primary reason for the modification").

## Availability

- **Free, public domain**: download from ASSIST QuickSearch (DLA's repository of
  record). Mirrors exist on everyspec.com for convenience; ASSIST is authoritative.
- Public NIST MBE Summit briefing slides on MIL-STD-31000 in acquisition are also
  freely available.

## License / redistribution status

- **Unclassified, Distribution A — Approved for Public Release; Distribution is
  Unlimited.** As a U.S. Government work it is in the public domain and may be
  copied/redistributed without restriction (with normal attribution best practice).
- This is one of the few standards in this directory that could legitimately be
  vendored in full; we still prefer linking to ASSIST as the source of record and
  keeping the repo lean.

## Official source

- ASSIST QuickSearch (official DoD standards repository of record):
  https://quicksearch.dla.mil (search "MIL-STD-31000")
- Defense Standardization Program / DoD standards portal:
  https://www.dla.mil/HQ/InformationOperations/Offers/Services/Standards
- NIST MBE Summit 2024 briefing on MIL-STD-31000 in acquisition (public):
  https://www.nist.gov/document/2024-mbe-summit-windham-mil-std-31000-technical-data-packages-acquisition

## Scope

From the standard's public abstract (fair quote):

> This standard defines the requirements for a technical data package (TDP) and its
> related TDP data management products. The purpose of the TDP is to provide an
> authoritative technical description of an item which is clear, complete and
> accurate, and in a form and format adequate for its intended use.

The standard specifies the TDP's **elements** (engineering design data, special
inspection equipment design data, special tooling design data, specifications,
software documentation, packaging instructions, and associated lists), plus TDP
**levels** (developmental, acquisition, and/or sustainment) and **types** that define
the extent of a package for a given purpose. Rev C integrates additive-manufacturing
design data into the package.

## PMI / MBD relevance

- MIL-STD-31000 is how the US DoD specifies **what a deliverable data package must
  contain**. For Isolith, it defines the customer-facing packaging contract —
  which data (design, 3D models, drawings, PMI annotations, specifications) must be
  present and in what form for defense customers.
- It references ASME Y14.47 (3D model data / 3D PDF interchange) for 3D data formats
  and, via the TDP Option Selection Worksheet, lets a customer pick the data types
  they want — including "native CAD" and "3D PDF".
