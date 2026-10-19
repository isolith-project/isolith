#!/usr/bin/env python3
"""P1-M2-T04 acceptance test: the 3DXML ASCII vs binary (.3DRep) hex notebook.

TDD contract (RED first, then GREEN on real material):
  * docs/re-notes/3dxml-hex-notebook.md exists, is non-empty, and names the
    observed facts the card carries (ZIP container, ASCII XML relaxation,
    XMLRepresentation/BagRep/PolygonalRep tessellation, urn:3DXML references,
    format="TESSELLATED" vs format="UVR", and the binary V5_CFV3 header).
  * Every byte-level claim in the notebook is asserted AGAINST REAL FILES
    here -- the notebook only wins if real bytes match the tables it records
    (observed facts, not guessed structure).
  * Tolerant when real samples are absent (SKIP, exit 0) so CI does not break
    before the P1-M1 corpus lane (PR #26, corpus/data/3dxml) lands; when
    samples ARE present the facts must hold exactly (FAIL loud otherwise).

Two independent real-material sets are probed:
  A) ASCII branch -- the committed/open corpus of 3DXML samples (PR #26
     corpus/data/3dxml). Located via $OEP_3DXML_SAMPLES or ./corpus/data/3dxml.
  B) Binary branch -- the V5_CFV3 .3DRep family, observed ONLY on the
     Dassault-hosted public gallery sample press_rink_axe_fta.3dxml
     (reference-only; NOT committed, per P1-M1-T02 license verdict).
     Located via $OEP_3DXML_BINARY_REPS (a directory of .3DRep files) or
     $OEP_3DXML_BINARY_ARCHIVE (the .3dxml archive itself). Absent = SKIP.
"""
import glob
import os
import re
import struct
import sys
import zipfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOTEBOOK = os.path.join(REPO, "docs", "re-notes", "3dxml-hex-notebook.md")

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


# --- observed facts (documented in the notebook) ---------------------------
# ASCII branch (committed corpus, verified 2026-10-15).
PK_MAGIC = b"\x50\x4b\x03\x04"                 # ZIP local-file header
XML_HEAD = b"<?xm"                              # XML declaration head
# Binary branch (reference-only Dassault gallery sample, 18 parts, 2026-10-15)
V5_MAGIC = b"V5_CFV3\x00"                      # offset 0x00..0x07
V5_TOKEN = b"CATIA_V5 CB0001\x00"              # offset 0x40..0x4f
V5_TOKEN_OFF = 0x40
V5_PRE = b"\x5f\x0d\x0a\x00"                   # 4 bytes immediately before token
V5_MARKER = b"CB__END\x00"                     # last 8 bytes of every part


def find_samples():
    dirs = []
    env = os.environ.get("OEP_3DXML_SAMPLES")
    if env:
        for chunk in re.split(r"[,:]", env):
            chunk = chunk.strip()
            if chunk:
                dirs.append(chunk)
    for rel in ("corpus/data/3dxml", os.path.join("..", "corpus", "data", "3dxml")):
        p = os.path.join(REPO, rel)
        if os.path.isdir(p):
            dirs.append(p)
    hits = []
    for d in dirs:
        if os.path.isdir(d):
            hits += glob.glob(os.path.join(d, "*.3dxml"))
            hits += glob.glob(os.path.join(d, "*.3DXML"))
    return sorted(set(hits))


def find_binary_reps():
    """Return list of binary .3DRep paths (reference-only material)."""
    env = os.environ.get("OEP_3DXML_BINARY_REPS")
    if env:
        paths = []
        for chunk in re.split(r"[,:]", env):
            chunk = chunk.strip()
            if not chunk:
                continue
            if os.path.isdir(chunk):
                paths += glob.glob(os.path.join(chunk, "*.3DRep"))
                paths += glob.glob(os.path.join(chunk, "*.3drep"))
            elif os.path.isfile(chunk):
                paths.append(chunk)
        return sorted(set(paths))
    arch = os.environ.get("OEP_3DXML_BINARY_ARCHIVE")
    if arch and os.path.isfile(arch):
        try:
            z = zipfile.ZipFile(arch)
            return [m for m in z.namelist() if m.endswith(".3DRep")]
        except zipfile.BadZipFile:
            return []
    return []


def notebook_text():
    if not os.path.isfile(NOTEBOOK):
        return ""
    return open(NOTEBOOK, encoding="utf-8").read()


# --- notebook existence + self-audit ----------------------------------------
def test_notebook_exists():
    txt = notebook_text()
    check("notebook exists: docs/re-notes/3dxml-hex-notebook.md",
          bool(txt), NOTEBOOK if not txt else "")
    if not txt:
        return
    for needle in ("V5_CFV3", "XMLRepresentation", "BagRepType", "TESSELLATED",
                   "UVR", "urn:3DXML:", "PK\\x03\\x04", "0x40", "CB__END",
                   "CATIA_V5", "zlib", "SchemaVersion"):
        check("notebook mentions %r" % needle, needle in txt,
              "needle not found in notebook")


# --- ASCII branch: real material (committed/open corpus) -------------------
def verify_ascii_sample(path):
    data = open(path, "rb").read()
    base = os.path.basename(path)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    is_zip = zipfile.is_zipfile(path) and data[:4] == PK_MAGIC
    if not is_zip:
        # standalone ASCII model document branch (observed: quad.3dxml)
        v("not a ZIP", True)
        v("xml head `<?xm`", data.startswith(XML_HEAD), data[:8].hex())
        txt = data.decode("utf-8", "replace")
        v("root <Model_3dxml", "<Model_3dxml" in txt)
        v("SchemaVersion", re.search(r"<SchemaVersion>\d+\.\d+</SchemaVersion>", txt) is not None)
        v("references urn:3DXML files", "urn:3DXML:" in txt)
        v("format TESSELLATED", 'format="TESSELLATED"' in txt)
        v("no inline VertexBuffer", "<VertexBuffer" not in txt)
        return ok
    # ZIP-archive 3DXML (dominant branch)
    v("ZIP magic PK\\x03\\x04", data[:4] == PK_MAGIC, data[:4].hex())
    z = zipfile.ZipFile(path)
    names = z.namelist()
    man = [n for n in names if n == "Manifest.xml"]
    v("contains Manifest.xml", len(man) == 1)
    if man:
        m = z.read("Manifest.xml")
        v("  Manifest xml head", m.startswith(XML_HEAD))
        mtxt = m.decode("utf-8", "replace")
        v("  Manifest <Manifest> root", "<Manifest" in mtxt)
        mroot = re.search(r"<Root>([^<]+\.3dxml)</Root>", mtxt, re.I)
        v("  Manifest <Root> names a present .3dxml",
          mroot is not None and mroot.group(1) in names,
          "Root member must exist in ZIP")
    # model document: at least one .3dxml member (not Manifest) is a
    # <Model_3dxml> document carrying the urn:3DXML rep wiring.
    xml_members = [n for n in names if n.lower().endswith(".3dxml") and n != "Manifest.xml"]
    v("contains XML .3dxml member(s)", len(xml_members) >= 1, str(xml_members[:4]))
    mdtexts = {n: z.read(n).decode("utf-8", "replace") for n in xml_members}
    model_docs = [n for n in xml_members
                  if "<Model_3dxml" in mdtexts[n] and "urn:3DXML:" in mdtexts[n]]
    v(">=1 <Model_3dxml> member with urn:3DXML refs", len(model_docs) >= 1,
      ("found %s" % model_docs) if model_docs else "none of %s" % xml_members)
    struct_docs = [n for n in xml_members if "<ProductStructure" in mdtexts[n]]
    v(">=1 member carries <ProductStructure>", len(struct_docs) >= 1,
      ("found %s" % struct_docs) if struct_docs else "none of %s" % xml_members)
    sv = next((re.search(r"<SchemaVersion>(\d+\.\d+)</SchemaVersion>", t)
               for t in mdtexts.values() if re.search(r"<SchemaVersion>", t)), None)
    v("SchemaVersion present in XML member(s)", sv is not None,
      sv.group(1) if sv else "")
    if struct_docs:
        sd = mdtexts[struct_docs[0]]
        repo = re.search(r'associatedFile="urn:3DXML:([^"]+)"', sd)
        v("product-structure doc associatedFile urn:3DXML:", repo is not None,
          repo.group(1) if repo else "")
        v("product-structure doc format TESSELLATED",
          'format="TESSELLATED"' in sd)
    # .3DRep member classification: ASCII XML, OSM material, and the
    # tessellated XMLRepresentation profile.
    rep_members = [n for n in names if n.lower().endswith(".3drep")]
    if rep_members:
        ascii_reps = all(z.read(n).lstrip().startswith(b"<") for n in rep_members)
        v("all .3DRep members ASCII XML (start `<`)", ascii_reps,
          "%d rep member(s)" % len(rep_members))
        xmlrep_seen = False
        osm_seen = False
        xmlrep_pattern = rb'<\?xml[^>]*>\s*<XMLRepresentation version="([\d.]+)"'
        for n in rep_members:
            r = z.read(n).lstrip()
            mv = re.match(xmlrep_pattern, r)
            if mv:
                if not xmlrep_seen:
                    v("  first tessellated rep %s is XMLRepresentation %s"
                      % (n, mv.group(1).decode()), True)
                    v("  BagRepType root", b'xsi:type="BagRepType"' in r)
                    v("  PolygonalRep present", b"PolygonalRepType" in r)
                    v("  VertexBuffer+Positions", b"<VertexBuffer" in r and b"<Positions>" in r)
                    v("  Faces present", b"<Faces>" in r)
                xmlrep_seen = True
            elif re.match(rb'<\?xml[^>]*>\s*<Osm\b', r) or b'xmlns="http://www.3ds.com/xsd/osm.xsd"' in r[:256]:
                osm_seen = True
        v(">=1 tessellated XMLRepresentation .3DRep", xmlrep_seen,
          "none matched" if not xmlrep_seen else "")
        if osm_seen:
            v("OSM material rep(s) present (osm.xsd)", True)
    return ok


def test_ascii_real_material():
    samples = find_samples()
    if not samples:
        print("[SKIP] no real 3DXML corpus available "
              "(set OEP_3DXML_SAMPLES or wait for PR #26) - nothing to verify")
        return
    print("verifying %d real 3DXML sample(s):" % len(samples))
    for s in samples:
        print("  ", s)
    allok = all(verify_ascii_sample(s) for s in samples)
    check("all ASCII-branch real-sample facts hold", allok,
          "exactly one bad sample fails the whole run")


# --- binary branch: reference-only Dassault material -----------------------
def verify_binary_rep(path_or_member, data):
    base = os.path.basename(path_or_member)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    v("NOT ASCII XML", not data.lstrip().startswith(b"<"))
    v("magic V5_CFV3\\x00 at 0x00", data[0:8] == V5_MAGIC, data[:8].hex())
    n = struct.unpack_from(">I", data, 0x08)[0]
    m = struct.unpack_from(">I", data, 0x0C)[0]
    v("u32BE@0x08 + u32BE@0x0C == len", n + m == len(data),
      "%d + %d vs %d" % (n, m, len(data)))
    v("0xFF x8 at 0x10..0x17", data[0x10:0x18] == b"\xff" * 8, data[0x10:0x18].hex())
    v("CATIA_V5 CB0001\\x00 at 0x40",
      data[V5_TOKEN_OFF:V5_TOKEN_OFF + len(V5_TOKEN)] == V5_TOKEN,
      data[V5_TOKEN_OFF:V5_TOKEN_OFF + 16].hex())
    v("4 bytes before token == 5f 0d 0a 00",
      data[V5_TOKEN_OFF - 4:V5_TOKEN_OFF] == V5_PRE,
      data[V5_TOKEN_OFF - 4:V5_TOKEN_OFF].hex())
    v("'CATIA_V5' occurs exactly once", data.count(b"CATIA_V5") == 1)
    v("last 8 bytes == CB__END\\x00", data[-8:] == V5_MARKER,
      data[-8:].hex())
    # parts do NOT start with a deflate/zlib stream (observed); incidental
    # 78 9C bytes can appear deep inside binary payload, so only the head is
    # an invariant.
    v("does not start with zlib/deflate header",
      data[:2] not in (b"\x78\x9c", b"\x78\x01", b"\x78\xda"),
      data[:2].hex())
    return ok


def test_binary_real_material():
    reps = find_binary_reps()
    if not reps:
        print("[SKIP] no real binary .3DRep material available "
              "(set OEP_3DXML_BINARY_REPS or OEP_3DXML_BINARY_ARCHIVE) -- "
              "binary-branch facts not re-verified here")
        return
    arch = os.environ.get("OEP_3DXML_BINARY_ARCHIVE")
    archive_mode = bool(arch and os.path.isfile(arch)
                        and zipfile.is_zipfile(arch)
                        and all(not os.path.isfile(r) for r in reps))
    if archive_mode and isinstance(arch, str):
        z = zipfile.ZipFile(arch)
        print("verifying %d binary .3DRep member(s) from archive:" % len(reps))
        allok = all(verify_binary_rep(r, z.read(r)) for r in reps)
    else:
        print("verifying %d binary .3DRep file(s):" % len(reps))
        allok = True
        for r in reps:
            with open(r, "rb") as fh:
                data = fh.read()
            if not verify_binary_rep(r, data):
                allok = False
    check("all binary-branch real-sample facts hold", allok,
          "exactly one bad sample fails the whole run")
    txt = notebook_text()
    check("notebook documents binary material is reference-only (not committed)",
          "reference" in txt.lower() and "not committed" in txt.lower(),
          "notebook must state the binary sample is reference-only")


def main():
    print("P1-M2-T04: 3DXML ASCII vs binary (.3DRep) hex notebook acceptance test")
    print("=" * 68)
    test_notebook_exists()
    test_ascii_real_material()
    test_binary_real_material()
    print("=" * 68)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
