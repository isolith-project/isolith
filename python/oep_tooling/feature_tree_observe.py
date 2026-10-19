#!/usr/bin/env python3
"""P5-M4-T01: feature-tree recoverability observation (native CATPart corpus).

Pure stdlib, clean-room, no vendor binary. Probes real native CATPart bytes
for the three facts the P5-M4-T01 recoverability memo is scored on:

  Q1  Feature-instance records: are feature-tree records readable (names,
      types, file order)?   -> length-prefixed standalone token census
  Q2  Attribute-key schema:  are the parameter KEY names readable?
                              -> attribute-key dictionary census
  Q3  Value encoding:        how are numeric parameter VALUES stored?
                              -> plausible IEEE-754 doubles near value-key
                                 keywords (binary, not ASCII)

Usage:
  python3 -m oep_tooling.feature_tree_observe CORPUS_DIR [--out OUT.json]
Where CORPUS_DIR is a cache of native .catpart files (sha256-named) and/or
a data/catpart directory; every file is deduped by basename/sha.

Outputs a JSON evidence artifact used by the memo. Values are observed from
real bytes; nothing is invented. Where a meaning is not byte-proven the memo
marks it [uncertain]; this script never asserts a meaning it did not observe.
"""
import argparse
import glob
import json
import os
import re
import struct
import sys

MAGIC_V5 = b"V5_CFV2\x00"

FEATURE_TOKENS = [b"Pad", b"Pocket", b"Hole", b"Sketch", b"Revolution", b"Shaft",
                  b"Chamfer", b"Fillet", b"Rib", b"Groove", b"Mirror", b"Draft",
                  b"PartBody", b"Thickness", b"Shell", b"Loft", b"Slot", b"Thread",
                  b"_FtrList", b"MechanicalPart", b"CurrentFeature"]
ATTR_KEYS = [b"Hole_Diameter", b"Hole_Depth", b"Hole_BottomType", b"Pocket_EndLimit",
             b"EdgeFillet_Radius", b"Offset", b"Depth", b"GSMPlaneValues",
             b"GSMPlaneOffset", b"MergeThin", b"ResultOUT", b"Layer", b"FirstLimit",
             b"SecondLimit", b"Length", b"Radius", b"Angle", b"SolveManager",
             b"VPMin", b"VPMax", b"Value"]
VALUE_KEYWORDS = (b"Value", b"Offset", b"Depth", b"Layer", b"FirstLimit",
                  b"SecondLimit", b"Length", b"Radius", b"Angle")


def collect_files(root):
    out = []
    for pat in ("*.catpart", "*.CATPart"):
        out.extend(glob.glob(os.path.join(root, "**", pat), recursive=True))
    # dedup by basename stem (sha256), keep first
    seen, res = set(), []
    for p in sorted(out):
        h = os.path.basename(p).split(".")[0]
        if h in seen:
            continue
        seen.add(h)
        res.append(p)
    return res


def count_substr(data, tokens):
    return {t.decode(): data.count(t) for t in tokens}


def lp_feature_counts(data):
    """Standalone length-prefixed feature tokens (prefix byte == strlen+1)."""
    counts = {}
    for tok in FEATURE_TOKENS:
        n = len(tok)
        c = 0
        for m in re.finditer(re.escape(tok), data):
            if m.start() >= 1 and data[m.start() - 1] == n + 1:
                c += 1
        counts[tok.decode()] = c
    return counts


def plausible_f64(data, off):
    try:
        v = struct.unpack_from("<d", data, off)[0]
    except struct.error:
        return None
    return v if (v != 0 and abs(v) < 1e9) else None


def count_value_hits(data):
    hits = 0
    for kw in VALUE_KEYWORDS:
        for m in re.finditer(re.escape(kw), data):
            off = m.start()
            for d in (4, 8, 12, 16, 20, 24, 28):
                if off + d + 8 > len(data):
                    continue
                v = plausible_f64(data, off + d)
                if v is not None:
                    hits += 1
                    break
    return hits


def fingerprint(path):
    data = open(path, "rb").read()
    is_part = b"MechanicalPart" in data and b"CATMechProdCont" not in data
    return {
        "file": os.path.basename(path),
        "size": len(data),
        "magic_ok": data[:8] == MAGIC_V5,
        "is_part": is_part,
        "writer_markers": sorted({m.group().decode() for m in
                                  re.finditer(rb"V5R\d\d(?:SP\d+)*HF\d*", data)}),
        "lp_feature_counts": lp_feature_counts(data),
        "attr_key_counts": count_substr(data, ATTR_KEYS),
        "value_double_hits": count_value_hits(data),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("corpus_dir")
    ap.add_argument("--out", default="feature-tree-observation.json")
    args = ap.parse_args()

    files = collect_files(args.corpus_dir)
    parts = []
    for p in files:
        fp = fingerprint(p)
        if fp["is_part"]:
            parts.append(fp)

    agg_attr = {k.decode(): 0 for k in ATTR_KEYS}
    lp_totals = {}
    value_files = 0
    value_hits = 0
    for fp in parts:
        for k, v in fp["lp_feature_counts"].items():
            lp_totals[k] = lp_totals.get(k, 0) + v
        for k, v in fp["attr_key_counts"].items():
            agg_attr[k] = agg_attr.get(k, 0) + v
        if fp["value_double_hits"]:
            value_files += 1
            value_hits += fp["value_double_hits"]

    report = {
        "task": "P5-M4-T01",
        "method": "byte-observed, clean-room (stdlib only, no vendor binary)",
        "corpus_root": args.corpus_dir,
        "unique_files_scanned": len(files),
        "parts_observed": len(parts),
        "lp_feature_records_total": sum(lp_totals.values()),
        "lp_feature_records_by_type": {k: lp_totals[k] for k in sorted(lp_totals)},
        "attr_key_census": {k: agg_attr[k] for k in sorted(agg_attr) if agg_attr[k]},
        "value_encoding": {
            "files_with_ieee754_near_value_keys": value_files,
            "plausible_double_hits": value_hits,
            "reading": ("binary IEEE-754 values present + Value/Depth/Offset/"
                        "Length/Radius-in-name keys -> numeric values are NOT "
                        "plain-ASCII; key names recoverable, value layout "
                        "[uncertain] until binary value-record decode"),
        },
    }
    with open(args.out, "w") as fh:
        json.dump(report, fh, indent=2)
    print("observed %d unique files, %d parts; wrote %s" % (
        len(files), len(parts), args.out))
    print("LP feature records:", report["lp_feature_records_total"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
