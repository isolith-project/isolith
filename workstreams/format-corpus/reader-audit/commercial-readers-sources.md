# P1-M3-T02 Sources: Commercial Reader Feature Lists

Version: 1.0, 2026-10-15

All sources below were reachable and checked on 2026-10-15 (real-material
verification). Vendor format tables are primary sources; pages labeled
"marketing claim" are the vendor's own promotional descriptions and are
recorded as claims, not as independently verified behavior. None of these
sources contributed implementation technique; this file is citation-only.

## Spatial HOOPS (HOOPS Exchange SDK) - now Tech Soft 3D

- https://docs.techsoft3d.com/hoops/exchange/start/supported-formats.html
  (File Formats, HOOPS Exchange docs; supported-import table with Tess,
  B-Rep, PMI columns per format; CATIA V5 up to V5-6R2026, CATIA V6 via
  3DXML 4.3, STEP AP203/214/242 Ed1-3, JT up to v10.11, QIF 3.0, and more.
  Primary source.)
- https://docs.techsoft3d.com/exchange/latest/start/format/catia_v5_reader.html
  (CATIA V5 reader: extensions CATDrawing/CATPart/CATProduct/CATShape/CGR;
  B-Rep via CGM, PMI visual and semantic forms, CGR cache-activation mode.
  Primary source.)
- https://docs.techsoft3d.com/exchange/2025.4.0/start/format/catia_v5_reader.html
  (Older snapshot of the same CATIA V5 reader page, CATIA V5 up to
  V5-6R2025. Primary source.)
- https://docs.techsoft3d.com/exchange/2025.1.0/start/format/catia_v5_reader.html
  (Earlier snapshot, same content family. Primary source.)
- https://docs.techsoft3d.com/exchange/2025.4.0/start/supported-formats.html
  (2025.4 supported-formats table. Primary source.)

## CAD Exchanger SDK

- https://docs324x.cadexchanger.com/sdk/sdk_supported_formats.html
  (CAD Exchanger SDK supported formats: CATIA V5R16-V5R34 with CATPart,
  CATProduct, CGR scope (B-Rep, polygonal, assembly structure via external
  files, names, properties, colors, graphical PMI); JT, STEP AP203/214/242,
  and the entity-class scope list. Primary source.)
- https://cadexchanger.com/formats/
  (Supported Formats page: proprietary CAD/BIM list including CATIA V5 and
  CATIA V6 (3D XML), kernels ACIS/Open CASCADE/Parasolid/Rhino/C3D, mesh
  formats; capability notes on B-Rep/meshes, B-Rep-to-mesh conversion, and
  three-level PMI. Primary source.)
- https://cadexchanger.com/products/sdk/
  (CAD Exchanger SDK product page: 30+ formats, C++/Python/C#/Java/JS
  bindings, unified data model, visitors over part-assembly trees, B-Rep and
  mesh inspection, PMI access. Marketing claim.)
- https://cadexchanger.com/scenarios/3d-data-interoperability/
  (Interoperability scenario page: native CAD, neutral, kernel formats;
  analysis of B-Rep and polygonal representations; mesh generation; NURBS
  recognition to analytical form. Marketing claim.)
- https://cadexchanger.com/blog/five-applications-that-enabled-step-conversion-with-cad-exchanger-sdk/
  (Blog on STEP: AP203/214/242 including PMI and AP242 tessellated geometry,
  external references, parallel import. Marketing claim / engineering blog.)

## TransMagic

- https://transmagic.com/cad-file-converter
  (CAD File Converter page: read/write format lists including CATIA V4/V5/V6
  (Model, CATPart, CATProduct, CGM, CGR), NX, Solid Edge, SOLIDWORKS, Creo,
  Inventor, AutoCAD, JT, Parasolid, ACIS, STEP, IGES, VDA-FS; PRO/EXPERT
  write tiers. Primary source / marketing.)
- https://transmagic.com/catia-viewer
  (CATIA Viewer page: open CATIA V4 .Model, V5/V6 CATPart/CATProduct, CGR
  Visrep, xCGM kernel files; hybrid Brep/Visrep detection; PMI view browser
  and 3D PDF export. Primary source / marketing.)
- https://transmagic.com/catia-file-converter
  (CATIA File Converter page: .Model vs CATPart vs CATProduct vs CGM vs CGR
  definitions, CGR as polygonal Visrep, EXPERT-only CATIA write. Primary
  source / marketing.)
- https://transmagic.com/cad-cam-3d-data-translation
  (Product/trial page: 48 CAD formats, proprietary/licensed, kernel, open
  standards, documentation and graphics; MBD support. Marketing claim.)
- https://www.engineering.com/transmagic-updates-support-for-catia-creo-inventor-acis
  (Engineering.com news, Aug 1 2022: TransMagic release adds CATIA V5 2022
  (R32), CATIA V6 2022 (R32), Creo 9.0, Inventor View 2023, ACIS 2022 (R32).
  Third-party news, cited for maintenance status.)

## Okino (PolyTrans|CAD)

- https://okino.com/conv/filefrmt_3dimport.htm
  (Okino 3D import formats page: CAD/Pack, Granite/Pack, DCC/Pack scope,
  Inventor, DWG/DXF/DWF-3D, IFC, PRC/3D PDF, Solid Edge, SolidWorks, PRO
  importer notes; statement of no reverse-engineered technology. Primary
  source / marketing.)
- https://okino.com/new/brochures/okino_polytrans_brochure.pdf
  (PolyTrans brochure PDF: format matrix with Imp/Exp/Mat/Hier/u-v/L&C/NURBS/
  Anim/Skin columns; CAD table listing CATIA v5 (.catproduct, .catPart,
  .cgr) via Spatial-CATIA5 "utilizes CATIA v5 components", CATIA v4 via
  Spatial-CATIA4, STEP/IGES/Parasolid/ProE via Granite/Pack, JT Open.
  Primary source / marketing.)
- https://www.okino.com/conv/reseller_overview.htm?0=
  (PolyTrans product overview: CAD/Pack, Granite/Pack, CATIA v4, CATIA v5
  (via actual Dassault CATIA v5 runtime), JT Open modules; import list
  including CATIA v4/v5, IGES, STEP, Parasolid, and the "no reverse
  engineering" statement. Primary source / marketing.)
- https://okino.com/conv/order/order.htm
  (Pricing and ordering page: module pack contents and position; confirms
  perpetual license model and maintenance. Primary source / marketing.)
- https://www.okino.com/partners/visual_components.pdf
  (Partner add-on PDF: CAD/Pack importer extensions and Granite/Pack scope
  (ACIS, IGES solids, Parasolid, STEP, ProE, VDA-FS). Marketing claim.)

## Notes on verification method

Each URL above was fetched successfully on 2026-10-15. Format capability
tables from vendor documentation are treated as primary; marketing pages are
flagged as claims. The audit report records only what these sources claim
their products can read (entity classes and formats); no source contributed
implementation technique, per the clean-room invariant (master plan
invariant 5). We have not executed any of these products, and Isolith does
not link or copy any commercial reader code.
