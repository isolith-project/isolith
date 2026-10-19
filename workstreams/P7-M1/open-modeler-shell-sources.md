# P7-M1-T01 Open Modeler Shell Eval: Evidence Sources

Every claim that drives the decision record
(workstreams/P7-M1/open-modeler-shell-eval-memo.md v1.0.0) is checked against a live
source below. Check date for all live web sources: 2026-10-16. Point-in-time
numbers (stars, issues, parity %) are labeled and dated; re-check on any
version bump of the memo.

## 1. FreeCAD (the open modeler candidate)

- Wikipedia, "FreeCAD" - https://en.wikipedia.org/wiki/FreeCAD
  - FreeCAD is "a general-purpose parametric 3D computer-aided design (CAD)
    modeler"; CATIA/ Creo/ SolidWorks-class; written in C++/Python; LGPL
    (project text LGPL-2.0-or-later; current infobox LGPL-2.1-or-later);
    built on Open CASCADE Technology, Coin3D (Open Inventor), Qt, Python;
    "FreeCAD itself can also be used as a library by other programs".
  - Stable release 1.1 (2026-03-24); preview "26.3 (formerly known as 1.2)".
- FreeCAD releases (GitHub) - https://github.com/FreeCAD/FreeCAD/releases
  - 26.3-RC1 published 2026-10-08, "the first complete build of the work
    done since 1.1"; FreeCAD moved to three releases per year on fixed dates
    with calendar-based versions (next: 27.1, 27.2, 27.3); weekly
    development builds published (weekly-2026.10.14 in the list).
- FreeCAD Sketcher solver, PlaneGCS - source tree:
  https://github.com/FreeCAD/FreeCAD/tree/main/src/Mod/Sketcher/App/planegcs
  (GCS.cpp header: LGPL, "either version 2 of the License or any later
  version" for the planegcs line; Constraints.h declares the constraint
  types: parallelism, perpendicularity, tangency, distance, angle, point-on,
  equal, symmetry, Snell, curve value, BSpline constraints, etc.)
  - Standalone extraction: https://pypi.org/project/planegcs/ (v0.8.0,
    "Python bindings for FreeCAD's PlaneGCS 2D geometric constraint solver",
    license LGPL-2.1-or-later) and https://github.com/spookylukey/planegcs
    (LICENSE states the package contains FreeCAD's PlaneGCS solver code,
    LGPL-2.1-or-later).
  - Point: FreeCAD's 2D geometric constraint solver is LGPL and exists as a
    standalone library outside FreeCAD, so "adopt an OSS solver, do not
    rebuild" (P7-M3-T01) does not require adopting FreeCAD.

## 2. Open CASCADE Technology (the OCC / keep-shelf path)

- OCCT releases - https://dev.opencascade.org/release
  - Latest release: OCCT 8.0.1 (8.x family); OCCT is distributed under
    LGPL 2.1 with the OCCT exception; modular C++ toolkits (math, geometry,
    topology, modeling algorithms, visualization, exchange, and OCAF
    application data with transactions/undo/persistence).
- Keelworks boolean record (internal, committed) - workstreams/P6-M3/boolean-ops-occ-
  adr.md: OCCT 8.0.x adopted as the boolean engine behind the oep-engine-occ
  seam; the "P7 modeler shell" is named a downstream caller of oep-geo
  through the clean interface.

## 3. CADCraft / ArtCraft lineage (the second acquire candidate)

- CADCraft repository (GitHub) - https://github.com/storytold/cadcraft
  - Created 2026-10-07; license Apache-2.0; "an open-source, clean-room
    reimplementation of Autodesk AutoCAD, rebuilt in pure Rust"; egui (no
    Electron); runs natively on macOS/Windows/Linux/FreeBSD and in the
    browser via WebAssembly; DXF native read/write (R12 through 2018), DWG
    (R13 through 2018) through the open-source acadrust library; SVG/PNG
    export; agent-first: "every menu item, tool and prompt is a command ...
    over MCP, a JSON control channel, or the CLI".
  - Point-in-time (2026-10-16): 5,254 stars, 2,318 forks, 631 open issues.
- CADCraft ROADMAP.md - https://raw.githubusercontent.com/storytold/cadcraft/
  main/ROADMAP.md (also linked from the repo)
  - Stage: pre-alpha; "next: alpha ~2 points (38% to 40% ready)".
  - Feature breadth: about 50% (245 / 491 AutoCAD-for-Mac menu items; 2D
    menus only about 65%).
  - "Ready for real work" about 38%; mainstream 2D drafter about 28%;
    essentials user about 46%.
  - Roadmap order: "2D drafting first, then annotation, layouts and plotting,
    DWG, parametrics and 3D"; target AutoCAD 2027 parity.
  - 3D kernel estimated 300-450 agent-hours out; remaining to full parity
    roughly 1,300-2,100 agent-hours; beta roughly 600-930 hours out.
  - Point: the lineage is a 2D drafting tool today; 3D and parametrics are
    scheduled strictly after the drafting alpha.
- ArtCraft site (CADCraft page) - https://www.getartcraft.com/
  - ArtCraft family: twelve pure-Rust apps (PhotoCraft, VectorCraft,
    FilmCraft, LightCraft, PdfCraft, EffectCraft, DesignCraft, WordCraft,
    GridCraft, DeckCraft, CADCraft, SoundCraft); free, open source.
- licenseware.io essay, "When AI Can Rebuild Any App" (2026-10-08) -
  https://licenseware.io/
  - ArtCraft published its clean-room family over 2026-09-30 to
    2026-10-07; CADCraft modeled on AutoCAD with an explicit Autodesk
    non-affiliation disclaimer; works from public specifications and
    observed behaviour; uses the MPL-2.0 acadrust library for DWG; sources
    the same EU Software Directive / SAS v WPL and US Sega v Accolade
    clean-room reasoning the Keelworks P0-M2 legal lane uses.
  - Point: the CADCraft run is direct evidence that small agent-driven
    clean-room teams can build CAD-class software fast (a data point for the
    own-shell feasibility argument), while its AutoCAD-class 2D scope
    confirms the capability gap versus the CATIA class.

## 4. Program records (internal, committed to keelworks/keelworks)

- Keelworks master plan decomposition (2026-10-15, v1.2) - .hermes/plans/
  2026-10-15_084708-open-catia-program-decomposition.md
  - P7 "Open Modeler Integration (FreeCAD-class core)" with P7-M1-T01 (this
    card), P7-M2 workbench, P7-M3 assembly, P7-M4 sketcher/parametrics, P7-M5
    UX; decisions 1-7 (codename, two-year native-reader scope, license
    posture, wave 1, private-first, Rust-first core, corpus dev-only).
- P6-M3 boolean record - workstreams/P6-M3/boolean-ops-occ-adr.md (v1.0.0): OCCT adopted
  behind oep-engine-occ; seam rule; license/clean-room sections.
- P6-M5 renderer record - workstreams/P6-M5/renderer-backend-wgpu-adr.md (v1.0.0): wgpu +
  WGSL, headless, shared preview and P2P render path, committed real-material
  spike; P6-M5-T02 preview viewer is the own desktop app shell.
- P5-M1-T03 native-reader record - workstreams/P5-M1/alpha-target-brep-first-adr.md
  (v1.0.0): alpha targets B-Rep/geometry first; feature-tree deferred to
  P5-M4 research.
- Card registry (this board): downstream cards P7-M2-T01 (workbench, gated on
  mesh latch t_899eb193), P7-M3-T01 (assembly workbench, "adopt an OSS solver
  lib, do not rebuild"), P7-M4-T01 (sketcher "if FreeCAD is the chosen
  shell"), P7-M5-T01/T02 (UX), and P7-M1-T02 (Blender-BSD surfacing, optional,
  running in parallel to this card).

## Honesty and currency

- The memo's numbered claims map to the sections above; any structural
  reading of FreeCAD or CADCraft internals is flagged in the memo as
  reference-material observation under the program clean-room rule, not as
  copied implementation.
- Point-in-time numbers (CADCraft stars/issues/parity, FreeCAD release dates,
  OCCT version) are check-dated 2026-10-16 and must be re-checked on the next
  version bump of the memo.
