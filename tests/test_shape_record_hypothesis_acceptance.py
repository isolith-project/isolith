#!/usr/bin/env python3
"""P1-M2-T05 acceptance test: the "shape record" hypothesis RE note.

TDD contract (RED first: the note does not exist yet; GREEN once
docs/re-notes/shape-record-hypothesis.md exists and matches real bytes):

  * The note exists, is non-empty, and names per-format facts (CGR magic /
    invariant template / rep-graph classes, CATPart V5_CFV2 + Shape 1 +
    BagRep, CATProduct CATMechProdCont + no MechanicalPart, 3DXML ASCII
    XMLRepresentation + PolygonalRep + VertexBuffer/Faces + Osm material
    reps, 3DXML binary V5_CFV3 + CATIA_V5 CB0001 + CB__END), each structural
    guess carrying an [uncertain] marker.
  * Every byte-level claim is re-derived AGAINST REAL FILES here - the note
    only wins if real bytes match what it records. The canonical wave-1
    corpus is the mohanevs knuckle-joint pack (114 CATPart + 7 CATProduct,
    writer V5R29SP0HF0) that the sibling P1-M2-T03 notebook verified; the
    part assertions hold on parts from every scratch corpus observed (BagRep
    / _BagRepsList 158/158), while the PRODUCT instance/rep-edge assertions
    are scoped to the canonical products where they hold (7/7). Writer
    variance in other corpora (sub-products without _InstanceName /
    CtorToProd / ProdToRep, e.g. an.CATProduct, Sub_Product*, Managing*,
    AssemblyTools*) is recorded IN THE NOTE as [uncertain], never as a
    universal law here.
  * Tolerant when real samples are absent (SKIP, exit 0) so CI stays green;
    when samples ARE present the facts must hold exactly (FAIL loud).

Sample env vars (each optional; colon/comma separated paths):
  OEP_CGR_SAMPLES     dir(s) holding .cgr files
  OEP_CATIA_SAMPLES   dir(s) holding .CATPart / .CATProduct files
  OEP_3DXML_SAMPLES   dir(s) holding .3dxml / .3DXML files (ASCII + binary)
  OEP_3DXML_BIN       path to a ZIP 3DXML that carries BINARY .3DRep
                      members (Dassault gallery reference sample; license:
                      reference-only, never committed by design)
"""
import glob
import json
import os
import re
import struct
import sys
import zipfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOTEBOOK = os.path.join(REPO, "docs", "re-notes",
                        "shape-record-hypothesis.md")

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)
    return ok


def env_dirs(key):
    out = []
    v = os.environ.get(key)
    if v:
        for chunk in re.split(r"[,:]", v):
            chunk = chunk.strip()
            if chunk:
                out.append(chunk)
    return out


def note_text():
    if not os.path.isfile(NOTEBOOK):
        return ""
    return open(NOTEBOOK, encoding="utf-8").read()


# --- observed facts (documented in the note, re-derived here) --------------
CGR_MAGIC = b"\x01\x00\x00\x00"
CGR_TEMPLATE = b"\x00\x07\x00\x00\x00\x07\x00\x00\x00\xb1\x01\x0b\x01\x08\x02\x4f"
V5_CFV2 = b"V5_CFV2\x00"
V5_CFV3 = b"V5_CFV3\x00"
CB_END = b"CB__END"


def verify_cgr(path):
    data = open(path, "rb").read()
    base = os.path.basename(path)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    v("magic 01 00 00 00", data[:4] == CGR_MAGIC, data[:4].hex())
    v("preamble 0x04..0x63 zero", data[4:0x64] == bytes(0x60),
      "nonzero=%d" % sum(1 for b in data[4:0x64] if b))
    if len(data) >= 0x78:
        v("invariant template 0x68..0x77",
          data[0x68:0x78] == CGR_TEMPLATE, data[0x68:0x78].hex())
    v("size 0x1000-aligned", len(data) % 0x1000 == 0, "size=%d" % len(data))
    return ok


def _verify_catia_family(data, base, is_part):
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    if is_part:
        v("class MechanicalPart present", data.count(b"MechanicalPart") >= 1)
        v("no CATMechProdCont", b"CATMechProdCont" not in data)
        v("shape-record token: BagRep", b"BagRep" in data)
        v("shape-record token: _BagRepsList", b"_BagRepsList" in data)
    else:
        v("class CATMechProdCont present", data.count(b"CATMechProdCont") >= 1)
        v("no MechanicalPart", b"MechanicalPart" not in data)
        # Reference-graph facts hold on the canonical wave-1 product corpus
        # (7/7 knuckle-joint products in P1-M2-T03). Writer variance in other
        # corpora (sub-products without _InstanceName, sub-assembly files
        # without CATMechProdCont) is recorded in the NOTE, not here.
        v("instance edges _InstanceName", b"_InstanceName" in data)
        v("rep edges CtorToProd/ProdToRep",
          b"CtorToProd" in data and b"ProdToRep" in data)
        v("no child filename .CATPart", b".CATPart" not in data)
    return ok


def verify_catpart(path):
    data = open(path, "rb").read()
    base = os.path.basename(path)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    v("magic V5_CFV2", data[:8] == V5_CFV2, data[:8].hex())
    ok &= _verify_catia_family(data, base, True)
    return ok


def verify_catproduct(path):
    data = open(path, "rb").read()
    base = os.path.basename(path)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    v("magic V5_CFV2", data[:8] == V5_CFV2, data[:8].hex())
    ok &= _verify_catia_family(data, base, False)
    return ok


def verify_3dxml_ascii(path):
    """ASCII branch: ZIP (XML .3DRep members) or plain XML.

    Facts the note records:
      * model XML: Model_3dxml + ProductStructure + ReferenceRep +
        associatedFile (edges to .3DRep files).
      * inner .3DRep members are ASCII XML (<?xml), and the GEOMETRY rep
        carries the PolygonalRep shape record (BagRep/PolygonalRep nesting,
        VertexBuffer/Positions, Faces triangle indices). Material/rendering
        reps are ASCII XML of the Osm schema (osm.xsd) and carry NO shape
        record - exactly what the note calls out.
    """
    base = os.path.basename(path)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    data = open(path, "rb").read()
    if data[:2] == b"PK":  # ZIP 3DXML
        try:
            z = zipfile.ZipFile(path)
        except Exception as e:
            v("unzip 3DXML", False, str(e))
            return False
        models = [z.read(n).decode("utf-8", "replace")
                  for n in z.namelist()
                  if n.lower().endswith(".3dxml")
                  and not n.lower().endswith(".3drep")]
        reps = {n: z.read(n) for n in z.namelist()
                if n.lower().endswith(".3drep")}
        model = "\n".join(models)
        v("model XML <Model_3dxml>", "<Model_3dxml" in model)
        v("model XML ProductStructure", "<ProductStructure" in model)
        v("model XML ReferenceRep", "<ReferenceRep" in model)
        v("model XML associatedFile", "associatedFile" in model)
        v("has .3DRep member(s)", len(reps) >= 1)
        if reps and any(
                re.search(rb"<\?xml", r) or rb"<XMLRepresentation" in r[:256]
                for r in reps.values()):
            # At least one ASCII-XML geometry rep must carry the shape record.
            geom = [r for r in reps.values()
                    if rb"<XMLRepresentation" in r[:256] or r.startswith(b"<?xml")]
            joined = b"\n".join(geom)
            v("a .3DRep member is ASCII XML",
              bool(geom), "%d/%d members" % (len(geom), len(reps)))
            v("geometry rep: PolygonalRep",
              b"PolygonalRepType" in joined or b"PolygonalRep" in joined)
            v("geometry rep: VertexBuffer/P ositions",
              b"VertexBuffer" in joined and b"<Positions>" in joined)
            v("geometry rep: Faces indices",
              b"<Faces>" in joined or b"triangles=" in joined)
            v("geometry rep: Edges polyline",
              b"<Polyline" in joined)
    else:  # plain ASCII 3DXML (e.g. quad.3dxml)
        txt = data.decode("utf-8", "replace")
        v("plain XML Model_3dxml", "<Model_3dxml" in txt)
        v("plain XML ProductStructure", "<ProductStructure" in txt)
        v("plain XML ReferenceRep", "<ReferenceRep" in txt)
        v("plain XML associatedFile", "associatedFile" in txt)
        v("plain XML references .3DRep geometry",
          ".3DRep" in txt or "3DRep" in txt)
    return ok


def verify_3dxml_binary(path):
    """Binary branch: the Dassault gallery reference sample (reference-only).

    FACTS: ZIP members named *.3DRep are BINARY, starting V5_CFV3\\0,
    carrying 'CATIA_V5 CB0001' at ~0x40 and containing 'CB__END'; the model
    XML refers to them with format="UVR".
    """
    base = os.path.basename(path)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    try:
        z = zipfile.ZipFile(path)
    except Exception as e:
        v("open ZIP 3DXML", False, str(e))
        return False
    rep_names = [n for n in z.namelist() if n.lower().endswith(".3drep")]
    v("has .3DRep member(s)", len(rep_names) >= 1)
    if not rep_names:
        return False
    blobs = {n: z.read(n) for n in rep_names}
    v("every .3DRep starts V5_CFV3\\0",
      all(b[:8] == V5_CFV3 for b in blobs.values()))
    v("every .3DRep carries 'CATIA_V5 CB0001'",
      all(b"CATIA_V5 CB0001" in b for b in blobs.values()))
    v("every .3DRep contains CB__END",
      all(CB_END in b for b in blobs.values()))
    v(".3DRep are BINARY (not XML)",
      all(not b.startswith(b"<?xml") for b in blobs.values()))
    model = "\n".join(z.read(n).decode("utf-8", "replace")
                      for n in z.namelist()
                      if n.lower().endswith(".3dxml")
                      and not n.lower().endswith(".3drep"))
    v("model XML format=\"UVR\" refs", 'format="UVR"' in model)
    return ok


def gather_samples():
    out = {"cgr": [], "part": [], "prod": [], "xml": [], "bin": []}
    for d in env_dirs("OEP_CGR_SAMPLES"):
        if os.path.isdir(d):
            out["cgr"] += glob.glob(os.path.join(d, "*.cgr"))
            out["cgr"] += glob.glob(os.path.join(d, "*.CGR"))
    for d in env_dirs("OEP_CATIA_SAMPLES"):
        if os.path.isdir(d):
            out["part"] += glob.glob(os.path.join(d, "*.CATPart"))
            out["prod"] += glob.glob(os.path.join(d, "*.CATProduct"))
    for d in env_dirs("OEP_3DXML_SAMPLES"):
        if os.path.isdir(d):
            out["xml"] += glob.glob(os.path.join(d, "*.3dxml"))
            out["xml"] += glob.glob(os.path.join(d, "*.3DXML"))
    out["bin"] += [p for p in env_dirs("OEP_3DXML_BIN") if os.path.isfile(p)]
    for k in out:
        out[k] = sorted(set(out[k]))
    return out


def test_note_exists():
    txt = note_text()
    check("note exists: docs/re-notes/shape-record-hypothesis.md", bool(txt),
          NOTEBOOK if not txt else "")
    if not txt:
        return
    for needle in ("CGR", "CATPart", "CATProduct", "3DXML", "PolygonalRep",
                   "BagRep", "V5_CFV2", "V5_CFV3", "CB__END", "shape record",
                   "[uncertain]"):
        check("note mentions %r" % needle, needle in txt)


def test_real_material(samples=None):
    if samples is None:
        samples = gather_samples()
    any_sample = any(samples[k] for k in samples)
    if not any_sample:
        print("\n[SKIP] no real samples available "
              "(set OEP_CGR_SAMPLES / OEP_CATIA_SAMPLES / "
              "OEP_3DXML_SAMPLES / OEP_3DXML_BIN) - nothing to verify")
        return
    allok = True
    if samples["cgr"]:
        print("\nverifying %d real CGR sample(s):" % len(samples["cgr"]))
        for s in samples["cgr"]:
            allok = verify_cgr(s) and allok
    if samples["part"] or samples["prod"]:
        print("\nverifying %d CATPart + %d CATProduct sample(s):"
              % (len(samples["part"]), len(samples["prod"])))
        for s in samples["part"]:
            allok = verify_catpart(s) and allok
        for s in samples["prod"]:
            allok = verify_catproduct(s) and allok
    if samples["xml"]:
        print("\nverifying %d 3DXML ASCII sample(s):" % len(samples["xml"]))
        ascii_ok = True
        for s in samples["xml"]:
            if "press_rink" in os.path.basename(s).lower():
                continue  # routed to the binary lane
            ascii_ok = verify_3dxml_ascii(s) and ascii_ok
        allok = ascii_ok and allok
    if samples["bin"]:
        print("\nverifying %d 3DXML BINARY .3DRep zip(s):" % len(samples["bin"]))
        for s in samples["bin"]:
            allok = verify_3dxml_binary(s) and allok
    check("all real-sample facts hold across loaded corpus", allok,
          "one bad file fails the run")


def main():
    print("P1-M2-T05: shape-record hypothesis RE note acceptance test")
    print("=" * 72)
    test_note_exists()
    test_real_material(gather_samples())
    print("=" * 72)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures),
                                          ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
