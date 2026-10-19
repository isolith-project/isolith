// Independent STEP reader probe for the P4-M3-T02 golden-diff gate.
//
// Wraps the Open CASCADE Technology (OCCT) STEP importer -- STEPControl_Reader,
// the exact reader FreeCAD uses for STEP import (FreeCAD's Part workbench is a
// thin layer over OCCT). This probe exists so the gate measures what an
// INDEPENDENT kernel recovers from our exported STEP, never what our own
// parsers/writers say.
//
// Deterministic key=value stdout contract (parsed by
// python/oep_tooling/step_export_validation.py):
//   read_status=<int>   IFSelect_ReturnStatus (1 == IFSelect_RetDone)
//   roots=<int>         NbRootsForTransfer()
//   shape_type=<TopAbs int|NULL>   NULL/NONE = parsed, no transferable shape
//   solids=<n> shells=<n> faces=<n> edges=<n> vertices=<n>
//   volume=<float>       BRepGProp::VolumeProperties (0 for non-closed shells)
//   area=<float>         BRepGProp::SurfaceProperties
//   bbox_min_* / bbox_max_*   BRepBndLib bounds
//
// Build: corpus/tools/build_occ_probe.sh  (needs libocct-*-dev, e.g.
// `apt install libocct-data-exchange-dev libocct-modeling-algorithms-dev
//  libocct-modeling-data-dev libocct-foundation-dev` on Debian/Ubuntu).
#include <STEPControl_Reader.hxx>
#include <TopExp_Explorer.hxx>
#include <TopAbs_ShapeEnum.hxx>
#include <TopoDS_Shape.hxx>
#include <BRepGProp.hxx>
#include <GProp_GProps.hxx>
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <iostream>
#include <iomanip>
#include <cstdlib>

static int count_occ(TopoDS_Shape const& sh, TopAbs_ShapeEnum t) {
    int n = 0;
    TopExp_Explorer e(sh, t);
    for (; e.More(); e.Next()) ++n;
    return n;
}

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr << "usage: occ_probe <file.step>\n";
        return 2;
    }
    STEPControl_Reader reader;
    IFSelect_ReturnStatus st = reader.ReadFile(argv[1]);
    std::cout << "read_status=" << static_cast<int>(st) << "\n";
    if (st != IFSelect_RetDone) {
        std::cout << "roots=0\nshape_type=NULL\n";
        return 1;
    }
    int roots = reader.NbRootsForTransfer();
    reader.TransferRoots();
    TopoDS_Shape shape = reader.OneShape();
    std::cout << "roots=" << roots << "\n";
    if (shape.IsNull()) {
        std::cout << "shape_type=NULL\n";
        return 1;
    }
    std::cout << "shape_type=" << shape.ShapeType() << "\n";
    std::cout << "solids=" << count_occ(shape, TopAbs_SOLID) << "\n";
    std::cout << "shells=" << count_occ(shape, TopAbs_SHELL) << "\n";
    std::cout << "faces=" << count_occ(shape, TopAbs_FACE) << "\n";
    std::cout << "edges=" << count_occ(shape, TopAbs_EDGE) << "\n";
    std::cout << "vertices=" << count_occ(shape, TopAbs_VERTEX) << "\n";

    GProp_GProps props;
    BRepGProp::VolumeProperties(shape, props);
    std::cout << "volume=" << std::setprecision(17) << props.Mass() << "\n";
    GProp_GProps surf;
    BRepGProp::SurfaceProperties(shape, surf);
    std::cout << "area=" << std::setprecision(17) << surf.Mass() << "\n";

    Bnd_Box box;
    BRepBndLib::Add(shape, box);
    if (!box.IsVoid()) {
        Standard_Real x1, y1, z1, x2, y2, z2;
        box.Get(x1, y1, z1, x2, y2, z2);
        std::cout << std::setprecision(17)
                  << "bbox_min_x=" << x1 << "\nbbox_min_y=" << y1
                  << "\nbbox_min_z=" << z1 << "\n"
                  << "bbox_max_x=" << x2 << "\nbbox_max_y=" << y2
                  << "\nbbox_max_z=" << z2 << "\n";
    }
    return 0;
}
