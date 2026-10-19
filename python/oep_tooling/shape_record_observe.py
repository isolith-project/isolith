#!/usr/bin/env python3
"""P1-M2-T05: "shape record" observation tool.

Pure-stdlib, read-only. Classifies and reports the shape-record evidence for
each Keelworks target format from REAL bytes:

  - CGR:        header magic, preamble, invariant template, rep-graph class
                strings (CAT3DBagRep / CATSurfacicRepy), alignment.
  - CATPart:    V5_CFV2 magic, ASMPRODUCT root, MechanicalPart class, BagRep /
                _BagRepsList shape-record tokens.
  - CATProduct: V5_CFV2 magic, CATMechProdCont class, instance/rep edges, no
                child file paths.
  - 3DXML ASCII: ZIP or plain XML; XMLRepresentation / PolygonalRep /
                VertexBuffer / Faces presence; inner .3DRep ASCII-XML.
  - 3DXML binary .3DRep: V5_CFV3 container magic + CATIA_V5 CB0001 + CB__END.

It deliberately does NOT parse geometry - P2/P3 own that. It re-derives what
docs/re-notes/shape-record-hypothesis.md records, so claims never drift from
material.

Usage:
    python -m oep_tooling.shape_record_observe <path> [<path> ...]

Exit 0 if every path was classified (or skipped as unclassifiable), 1 if a
file cannot be read.
"""
import os
import re
import struct
import sys
import zipfile

CGR_MAGIC = b"\x01\x00\x00\x00"
CGR_TEMPLATE = b"\x00\x07\x00\x00\x00\x07\x00\x00\x00\xb1\x01\x0b\x01\x08\x02\x4f"
V5_CFV2 = b"V5_CFV2\x00"
V5_CFV3 = b"V5_CFV3\x00"


def u32(data, off):
    return struct.unpack_from("<I", data, off)[0]


def printable_strings(data, minlen=5):
    cur_start = None
    cur = bytearray()
    for i, b in enumerate(data):
        if 32 <= b < 127:
            if cur_start is None:
                cur_start = i
            cur.append(b)
        else:
            if cur and len(cur) >= minlen:
                yield (cur_start, bytes(cur).decode("latin1"))
            cur = bytearray()
            cur_start = None
    if cur and len(cur) >= minlen:
        yield (cur_start, bytes(cur).decode("latin1"))


def cat_strings(data, maxlen=48):
    return [(o, s) for o, s in printable_strings(data)
            if s.startswith("CAT") and len(s) <= maxlen]


def observe_cgr(data):
    n = len(data)
    print("  kind        : CGR (CATIA Graphics Representation)")
    print("  magic       : %s (u32 LE %d)" % (data[:4].hex(" "), u32(data, 0)))
    print("  preamble    : 0x04..0x63 all-zero = %s"
          % (data[4:0x64] == bytes(0x60)))
    if n >= 0x78:
        t = data[0x68:0x78]
        print("  template    : 0x68..0x77 %s  invariant=%s"
              % (t.hex(" "), t == CGR_TEMPLATE))
        try:
            print("  f32 @0x7c   : %g (LE)" % struct.unpack_from("<f", data, 0x7c)[0])
        except Exception:
            pass
    print("  size        : %d (%%0x1000 = %d)" % (n, n % 0x1000))
    cats = cat_strings(data)
    if cats:
        print("  class names : %s" % ", ".join("%r@%#x" % (s, o) for o, s in cats[:8]))


def observe_ascii_3drep(blob, label=".3DRep"):
    hits = []
    for kw in ("XMLRepresentation", "BagRep", "PolygonalRep", "VertexBuffer",
               "Positions", "Normals", "Faces", "Edges", "Polyline"):
        hits.append("%s=%d" % (kw, blob.count(kw.encode())))
    print("  %s (ascii-xml) : %s" % (label, ", ".join(hits)))


def observe_catia(data):
    print("  magic       : %s (V5_CFV2 container)" % data[:8].hex(" "))
    print("  root token  : ASMPRODUCT present=%s" % (b"ASMPRODUCT" in data))
    mech = b"MechanicalPart" in data
    prod = b"CATMechProdCont" in data
    print("  family      : %s" % ("CATPart (MechanicalPart)" if mech
                                  else "CATProduct (CATMechProdCont)" if prod
                                  else "unknown"))
    print("  shape rec   : BagRep=%s _BagRepsList=%s PRDREP=%s"
          % (b"BagRep" in data, b"_BagRepsList" in data, b"PRDREP" in data))
    if prod:
        print("  graph edges : _InstanceName=%s CtorToProd=%s ProdToRep=%s"
              % (b"_InstanceName" in data, b"CtorToProd" in data,
                 b"ProdToRep" in data))
        print("  no child fn : .CATPart absent=%s" % (b".CATPart" not in data))


def observe_3dxml(path, data):
    if data[:2] == b"PK":
        z = zipfile.ZipFile(path)
        names = z.namelist()
        print("  kind        : 3DXML ZIP (%d members)" % len(names))
        models = [n for n in names if n.lower().endswith(".3dxml")
                  and not n.lower().endswith(".3drep")]
        reps = [n for n in names if n.lower().endswith(".3drep")]
        xml_reps = [n for n in reps
                    if z.read(n).startswith(b"<?xml")
                    or b"<Root".startswith(b"<")]  # placeholder
        for n in models[:3]:
            m = z.read(n).decode("utf-8", "replace")
            print("  model %s: ProductStructure=%s ReferenceRep=%s"
                  % (n[:28], "<ProductStructure" in m, "<ReferenceRep" in m))
            fmts = re.findall(r'format="([^"]+)"', m)
            if fmts:
                print("      formats: %s" % ", ".join(sorted(set(fmts))))
        for n in reps[:4]:
            blob = z.read(n)
            xmlish = blob.startswith(b"<?xml") or b"<XMLRepresentation" in blob[:200]
            if xmlish:
                observe_ascii_3drep(blob, os.path.basename(n))
            else:
                end = blob.rfind(b"CB__END")
                print("  %-28s : BINARY V5_CFV3=%s CATIA_V5 CB0001=%s CB__END=%s"
                      % (os.path.basename(n), blob[:8] == V5_CFV3,
                         b"CATIA_V5 CB0001" in blob, end >= 0))
    else:
        txt = data.decode("utf-8", "replace")
        print("  kind        : plain ASCII 3DXML")
        print("  structure   : Model_3dxml=%s ProductStructure=%s ReferenceRep=%s associatedFile=%s"
              % ("<Model_3dxml" in txt, "<ProductStructure" in txt,
                 "<ReferenceRep" in txt, "associatedFile" in txt))
        fmts = sorted(set(re.findall(r'format="([^"]+)"', txt)))
        if fmts:
            print("  formats     : %s" % ", ".join(fmts))


def classify(path):
    with open(path, "rb") as fh:
        data = fh.read()
    print("== %s (%d bytes) ==" % (os.path.abspath(path), len(data)))
    head8 = data[:8]
    if head8[:4] == CGR_MAGIC and data[4:0x64] == bytes(0x60):
        observe_cgr(data)
    elif head8 == V5_CFV2:
        observe_catia(data)
    elif head8[:3] == b"PK\x03\x04" or head8[:2] == b"PK":
        try:
            observe_3dxml(path, data)
        except zipfile.BadZipFile:
            print("  kind        : unclassifiable (PK prefix but bad zip)")
    else:
        # plain ASCII 3DXML? check for XML start / Model_3dxml
        if data[:5] in (b"<?xml", b"<?XML") or b"<Model_3dxml" in data[:4096]:
            try:
                observe_3dxml(path, data)
            except Exception as e:
                print("  kind        : XML-looking, err %s" % e)
        else:
            print("  kind        : unclassifiable (no known magic)")
    print()


def main(argv):
    if not argv:
        print(__doc__)
        return 1
    rc = 0
    for p in argv:
        if not os.path.isfile(p):
            print("cannot read %s" % p)
            rc = 1
            continue
        try:
            classify(p)
        except Exception as e:
            print("== %s == error: %s" % (p, e))
            rc = 1
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
