// oep-engine-occ C++ glue (P6-M3-T04 / ADR v1.1.0).
//
// This is the ONE translation unit that includes OCCT headers. Everything
// OCCT-shaped (TopoDS_*, Handle_, BRepAlgo*, BRepFilletAPI, BRepPrimAPI, gp_*)
// lives here and behind the cxx bridge; the Rust side and the oep-geo seam
// never see a single OCCT identifier.
//
// Roles (cxx layout):
//   - This header declares the extern "C++" function surface cxx expects.
//   - The cxx bridge in src/lib.rs generates the shared result structs
//     (OpOut / InfoOut / EdgesOut) into the cxxbridge header
//     `oep-engine-occ/src/lib.rs.h`, which includes this header; glue.cc pulls
//     that generated header in for the full struct definitions. Struct bodies
//     therefore live ONLY in the generated header (this file forward-declares
//     them so the function declarations below type-check).
//
// Session model: the registry is owned HERE in C++. occNewSession allocates a
// session id (u64); every later call references it. The Rust side only moves
// u64 tokens and plain structs across the seam -- it never touches kernel
// memory. A mutex guards the registry for concurrent calls.
//
// License: OCCT is LGPL-2.1 with the OCCT Exception 1.0 (ADR section 7). The
// P0-M2 legal pass writes the required OCCT notice before any release.
#pragma once

#include <cstdint>

#include "rust/cxx.h"

namespace oep::occ {

// Forward declarations; full definitions come from the generated cxxbridge
// header `oep-engine-occ/src/lib.rs.h` (shared structs declared in src/lib.rs).
struct OpOut;
struct InfoOut;
struct EdgesOut;
struct BoxOut;

// Session lifecycle.
uint64_t occNewSession();
void occDropSession(uint64_t s);

// Primitives (BRepPrimAPI). Degenerate parameters (zero size/radius) are
// reported as code 3, not thrown.
OpOut occMakeBox(uint64_t s, double x0, double y0, double z0, double x1, double y1,
                 double z1, double tol);
OpOut occMakeCylinder(uint64_t s, double radius, double height, double tol);
OpOut occMakeSphere(uint64_t s, double radius, double tol);

// Booleans (BRepAlgoAPI_Fuse/Cut/Common). op: 0 union, 1 difference, 2 common.
OpOut occBoolean(uint64_t s, int32_t op, uint64_t a, uint64_t b, double tol);

// Fillet / chamfer on the edges of `subject` (indices into the subject's own
// edge topology). mode: 0 fillet (amount = radius), 1 chamfer (amount =
// symmetric distance).
OpOut occFilletChamfer(uint64_t s, int32_t mode, uint64_t subject,
                       rust::Vec<uint64_t> edges, double amount, double tol);

// Sweeps of a planar XY profile (closed loop, implicit closing edge, in the
// Z=0 plane): `xy` is x0,y0,x1,y1,... -- extrude pushes height along +Z
// (BRepPrimAPI_MakePrism), revolve rotates angle radians about the Z axis
// (BRepPrimAPI_MakeRevol).
OpOut occExtrude(uint64_t s, rust::Vec<double> xy, double height, double tol);
OpOut occRevolve(uint64_t s, rust::Vec<double> xy, double angle, double tol);

// Edge indices of a shape (TopExp_Explorer order, deterministic per OCCT build).
EdgesOut occEdges(uint64_t s, uint64_t subject);

// Measured properties (BRepGProp volume + surface area, topology counts).
InfoOut occInfo(uint64_t s, uint64_t token);

// Axis-aligned bounding box of a shape as a closed extent (`Bnd_Box` +
// `BRepBndLib`), used by feature-graph replay trims and goldens.
BoxOut occBBox(uint64_t s, uint64_t subject);

// Rigid placement by copy (BRepBuilderAPI_Transform with a gp_Trsf):
//   - occTranslate: translate by the vector (dx, dy, dz);
//   - occMirrorPlane: mirror about the axis-aligned plane whose normal is the
//     axis UNIT direction (axis 0=x, 1=y, 2=z) through coordinate `at`.
OpOut occTranslate(uint64_t s, uint64_t subject, double dx, double dy, double dz);
OpOut occMirrorPlane(uint64_t s, uint64_t subject, int32_t axis, double at);

// Number of shapes currently held in the session registry (diagnostic/test).
uint64_t occCountShapes(uint64_t s);

}  // namespace oep::occ
