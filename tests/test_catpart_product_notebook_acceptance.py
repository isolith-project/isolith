#!/usr/bin/env python3
"""P1-M2-T03 acceptance test: CATPart vs CATProduct container notebook.

TDD contract (RED first, then GREEN on real material):
  * docs/re-notes/catpart-catproduct-container-notebook.md exists, is
    non-empty, and names every observed fact this hour carries (magic
    V5_CFV2, length-prefixed strings, root token ASMPRODUCT, the family
    class strings MechanicalPart / CATMechProdCont, the header token at
    0x54, catalogLinks, the no-path reference-graph finding).
  * Every byte-level claim is asserted AGAINST REAL CATPart/CATProduct
    files here -- the notebook only wins if real bytes match the tables it
    records (observed facts, not guessed structure).
  * Tolerant when real samples are absent (SKIP, exit 0) so CI stays green
    before P1-M1 corpus sync lands; when samples ARE present the facts must
    hold exactly (FAIL loud otherwise).

Samples: searched in order
  1. $OEP_CATIA_SAMPLES  (explicit corpus dir; accepts several paths
     separated by ':' or ',')
  2. ./samples/  ../samples/  (repo-relative local corpus)

Facts verified on the wave-1 corpus (2026-10-15, 114 CATPart + 7 CATProduct,
all from mohanevs/solid-modeling-catia-v6, MIT, saved with marker
V5R29SP0HF0): every claim below held on 121/121 files.
"""
import glob
import os
import re
import struct
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
NOTEBOOK = os.path.join(REPO, "docs", "re-notes",
                        "catpart-catproduct-container-notebook.md")

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


def find_samples():
    dirs = []
    env = os.environ.get("OEP_CATIA_SAMPLES")
    if env:
        for chunk in re.split(r"[,:]", env):
            chunk = chunk.strip()
            if chunk:
                dirs.append(chunk)
    for rel in ("samples", os.path.join("..", "samples")):
        p = os.path.join(REPO, rel)
        if os.path.isdir(p):
            dirs.append(p)
    parts, prods = [], []
    for d in dirs:
        if not os.path.isdir(d):
            continue
        for f in sorted(glob.glob(os.path.join(d, "*"))):
            low = f.lower()
            if low.endswith(".catpart"):
                parts.append(f)
            elif low.endswith(".catproduct"):
                prods.append(f)
    return sorted(set(parts)), sorted(set(prods))


def notebook_text():
    if not os.path.isfile(NOTEBOOK):
        return ""
    return open(NOTEBOOK, encoding="utf-8").read()


# --- observed facts (documented in the notebook + re-derived here) ----------
V5_MAGIC = b"V5_CFV2\x00"                 # 8-byte container magic, both families
WRITER = re.compile(rb"V5R\d\dSP\d+HF\d+")  # writer marker observed in all files
# Header region tokens (little-endian u32s, family-verified on wave-1 corpus):
HDR_HDRLEN = 0x48        # u32 == 0xF0 (240) products (7/7); 240 standard parts
HDR_54 = 0x54            # u32: products == 3 (7/7); parts in {30, 0x5B00}
HDR_58 = 0x58            # u32 == 0x620 (1568) on all products + standard parts
ASMPRODUCT = b"ASMPRODUCT"   # root-product token, then u8 len + name


def asmproduct_root_ok(data):
    """Observed idiom: 'ASMPRODUCT' + 1-byte length (strlen+1) + name."""
    i = data.find(ASMPRODUCT)
    if i < 0:
        return False
    L = data[i + len(ASMPRODUCT)]
    if not 1 <= L <= 40:
        return False
    name = data[i + len(ASMPRODUCT) + 1: i + len(ASMPRODUCT) + 1 + L]
    printable = sum(1 for x in name if 0x20 <= x < 0x7f)
    return len(name) == L and printable >= L - 1


def verify_catpart(path):
    data = open(path, "rb").read()
    base = os.path.basename(path)
    ok = True

    def v(name, cond, detail=""):
        nonlocal ok
        if not check("  %s [%s]" % (name, base), cond, detail):
            ok = False
        return cond

    v("magic V5_CFV2", data[:8] == V5_MAGIC, data[:8].hex())
    v("class MechanicalPart present", data.count(b"MechanicalPart") >= 1,
      "count=%d" % data.count(b"MechanicalPart"))
    v("class CATMechProdCont absent", b"CATMechProdCont" not in data)
    v("no CATProduct class token", b"CATProduct" not in data)
    m = WRITER.search(data)
    v("writer marker V5R..SP..HF..", bool(m),
      m.group(0).decode() if m else "none")
    v("u32@0x54 in {30, 0x5B00}", struct.unpack_from("<I", data, HDR_54)[0]
      in (30, 0x5B00), "got %d" % struct.unpack_from("<I", data, HDR_54)[0])
    v("u32@0x48 in {240, 0x464A1000}",
      struct.unpack_from("<I", data, 0x48)[0] in (240, 0x464A1000),
      "got %d" % struct.unpack_from("<I", data, 0x48)[0])
    v("root token ASMPRODUCT present", ASMPRODUCT in data)
    v("root token ASMPRODUCT has length-prefixed name",
      asmproduct_root_ok(data))
    v("catalogLinks registry present", b"catalogLinks" in data)
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

    v("magic V5_CFV2", data[:8] == V5_MAGIC, data[:8].hex())
    v("class CATMechProdCont present", data.count(b"CATMechProdCont") >= 1,
      "count=%d" % data.count(b"CATMechProdCont"))
    v("class MechanicalPart absent", b"MechanicalPart" not in data)
    v("no CATPart class token", b"CATPart" not in data)
    m = WRITER.search(data)
    v("writer marker V5R..SP..HF..", bool(m),
      m.group(0).decode() if m else "none")
    v("u32@0x54 == 3", struct.unpack_from("<I", data, HDR_54)[0] == 3,
      "got %d" % struct.unpack_from("<I", data, HDR_54)[0])
    v("u32@0x48 == 240", struct.unpack_from("<I", data, 0x48)[0] == 240,
      "got %d" % struct.unpack_from("<I", data, 0x48)[0])
    v("root token ASMPRODUCT present", ASMPRODUCT in data)
    v("root token ASMPRODUCT has length-prefixed name",
      asmproduct_root_ok(data))
    v("catalogLinks registry present", b"catalogLinks" in data)
    v("instance-name key _InstanceName present", b"_InstanceName" in data)
    v("rep edges CtorToProd/ProdToRep present",
      b"CtorToProd" in data and b"ProdToRep" in data)
    v("constraint edges Coincidence present", b"Coincidence." in data)
    v("no literal child filename .CATPart", b".CATPart" not in data,
      "found at %#x" % data.find(b".CATPart") if b".CATPart" in data else "")
    v("no drive-letter path C:\\..", not re.search(rb"[A-Za-z]:[\\\\/]", data))
    return ok


def test_notebook_exists():
    txt = notebook_text()
    check("notebook exists: docs/re-notes/catpart-catproduct-container-notebook.md",
          bool(txt), NOTEBOOK if not txt else "")
    if not txt:
        return
    for needle in ("V5_CFV2", "MechanicalPart", "CATMechProdCont",
                   "ASMPRODUCT", "_InstanceName", "0x54", "catalogLinks",
                   "reference", "path"):
        check("notebook mentions %r" % needle, needle in txt)


def test_real_material():
    parts, prods = find_samples()
    if not parts and not prods:
        print("[SKIP] no real CATPart/CATProduct samples available "
              "(set OEP_CATIA_SAMPLES) - nothing to verify")
        return
    if not (parts and prods):
        print("[SKIP] need at least one .CATPart AND one .CATProduct; "
              "have %d parts, %d products" % (len(parts), len(prods)))
        return
    print("\nverifying %d CATPart + %d CATProduct real sample(s):"
          % (len(parts), len(prods)))
    allok = True
    for s in parts:
        allok = verify_catpart(s) and allok
    for s in prods:
        allok = verify_catproduct(s) and allok
    check("all real-sample facts hold across %d files" % (len(parts) + len(prods)),
          allok, "one bad file fails the whole run")


def main():
    print("P1-M2-T03: CATPart vs CATProduct container notebook acceptance test")
    print("=" * 70)
    test_notebook_exists()
    test_real_material()
    print("=" * 70)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
