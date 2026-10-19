#!/usr/bin/env python3
"""P5-M1-T01: CATPart/CATProduct schema-drift runner (full-corpus re-run).

Re-runs the P1-M2-T03 CATPart-vs-CATProduct container observation over the FULL
131-entry native corpus (manifest: 123 catpart-v5 + 8 catproduct-v5) and
measures schema drift ACROSS WRITER ERAS (R2026 vs older).

Byte-observed, clean-room (program decision 5): every value in the report is
read back from a real file resolved via the corpus registry; no Dassault
library and no vendor binary is executed.

Honesty doctrine (carried over from P1-M2-T02 cgr_drift.py):
  * Era labels come ONLY from observed writer/provenance markers inside the
    file (V5R..SP..HF.., CATBuildLevel, MinimalVersionToRead), never from
    marketing release names.
  * An era with ZERO real samples is reported status='blocked_on_sample' --
    never silently treated as 'no drift'.
  * Corpus blobs live on the NAS object store; object-store entries are
    resolved through the registry cache and fetched only when absent.

Usage:
  python3 -m oep_tooling.catpart_drift [corpus-root] [--json out.json]
Exit code: 0 (report produced). Claim mismatches are not hard failures here;
the acceptance test (tests/test_catpart_drift_acceptance.py) enforces them.
"""
import argparse
import json
import os
import re
import struct
import sys

try:
    from oep_tooling.corpus import registry
except ImportError:  # allow direct file execution next to oep_tooling/
    sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    from oep_tooling.corpus import registry

MAGIC_V5 = b"V5_CFV2\x00"
WRITER_MARKER_RE = re.compile(rb"V5R\d\d(?:SP\d+)*HF\d*")
CATIAV5R_RE = re.compile(rb"CATIAV5R\d+")
BUILD_DATE_RE = re.compile(rb"\d\d-\d\d-\d{4}\.\d\d\.\d\d")
R2026_RE = re.compile(rb"R2026|V5-6R2026|3DEXPERIENCE")
DRIVE_PATH_RE = re.compile(rb"[A-Za-z]:[\\/]")

NATIVE_FORMATS = ("catpart-v5", "catproduct-v5")


def fingerprint(path):
    """Probe one real CATPart/CATProduct file. Returns plain-dict probe."""
    with open(path, "rb") as fh:
        data = fh.read()
    header = {}
    if len(data) >= 0x6c:
        for off in (0x40, 0x44, 0x48, 0x4c, 0x50, 0x54, 0x58, 0x5c, 0x60, 0x64):
            header[hex(off)] = hex(struct.unpack_from("<I", data, off)[0])
    return {
        "file": os.path.basename(path),
        "size": len(data),
        "magic_ok": data[:8] == MAGIC_V5,
        "header": header,
        "writer_markers": sorted({m.group().decode() for m in WRITER_MARKER_RE.finditer(data)}),
        "catiav5r": sorted({m.group().decode() for m in CATIAV5R_RE.finditer(data)}),
        "build_dates": sorted({m.group().decode() for m in BUILD_DATE_RE.finditer(data)}),
        "r2026_markers": [m.group().decode() for m in R2026_RE.finditer(data)],
        "count_mech_part": data.count(b"MechanicalPart"),
        "count_mech_prod_cont": data.count(b"CATMechProdCont"),
        "count_cls_catpart": data.count(b"CATPart"),
        "count_cls_catproduct": data.count(b"CATProduct"),
        "count_asmproduct": data.count(b"ASMPRODUCT"),
        "count_cataloglinks": data.count(b"catalogLinks"),
        "count_instancename": data.count(b"_InstanceName"),
        "count_c2p": data.count(b"CtorToProd"),
        "count_p2r": data.count(b"ProdToRep"),
        "count_coincidence": len(re.findall(rb"Coincidence\.\d+", data)),
        "count_constraintasm": data.count(b"ConstraintASM"),
        "count_asmgeometry": data.count(b"ASMGEOMETRY"),
        "has_child_catpart": b".CATPart" in data,
        "drive_path_hits": [m.start() for m in DRIVE_PATH_RE.finditer(data)],
        "jpeg_at_header": data[0x40:0x4c].find(b"\xff\xd8\xff\xe0") >= 0,
    }


def era_of(probe):
    """Classify era by OBSERVED writer markers (never marketing names)."""
    joined = " ".join(probe["writer_markers"] + probe["catiav5r"]
                      + probe.get("r2026_markers", []))
    if re.search(r"R2026|V5-6R2026|3DEXPERIENCE", joined):
        return "R2026"
    m = re.search(r"V5R(\d\d)", joined)
    if m:
        return "V5R%s" % m.group(1)
    return "unrecognized"


def native_entries(manifest):
    return [e for e in manifest["data"].get("entries", [])
            if e.get("format") in NATIVE_FORMATS]


def scan_corpus(manifest):
    """Fingerprint every native entry via the registry (tolerant on misses).

    Returns (probes, ok_ids, miss_ids). ok_ids keep manifest order.
    """
    probes = {}
    ok_ids, miss_ids = [], []
    for e in native_entries(manifest):
        path, err = registry.resolve_entry(e, manifest=manifest)
        if err:
            miss_ids.append((e["id"], err))
            continue
        if not registry.sha256_file(path) == e.get("sha256"):
            miss_ids.append((e["id"], "sha256 mismatch"))
            continue
        p = fingerprint(path)
        p["id"] = e["id"]
        p["backend"] = e.get("backend")
        p["is_part"] = e.get("format") == "catpart-v5"
        probes[e["id"]] = p
        ok_ids.append(e["id"])
    return probes, ok_ids, miss_ids


def header_histograms(probes):
    """Per-family value histograms for the header words the notebook tracks."""
    fields = ("0x40", "0x44", "0x48", "0x4c", "0x50", "0x54", "0x58", "0x5c", "0x60", "0x64")
    out = {}
    for family in ("parts", "products"):
        fam = {k: v for k, v in probes.items() if (v["is_part"]) == (family == "parts")}
        hist = {}
        for f in fields:
            hist[f] = {}
            for v in fam.values():
                off = v["header"].get(f, "NA")
                hist[f][off] = hist[f].get(off, 0) + 1
        out[family] = hist
    return out


def run(repo=None):
    """Run the full-corpus drift observation; return the JSON report dict."""
    m = registry.load_manifest(repo=repo)
    probes, ok_ids, miss_ids = scan_corpus(m)
    n_parts = sum(1 for i in ok_ids if probes[i]["is_part"])
    n_prods = len(ok_ids) - n_parts

    eras = {}
    for i in ok_ids:
        e = era_of(probes[i])
        eras.setdefault(e, []).append(i)

    parts = [probes[i] for i in ok_ids if probes[i]["is_part"]]
    prods = [probes[i] for i in ok_ids if not probes[i]["is_part"]]

    # notebook-claim confirmation table (P1-M2-T03 + full-corpus extension)
    claims = {
        "magic V5_CFV2": sum(1 for p in parts + prods if p["magic_ok"]) == len(ok_ids),
        "writer marker V5R..SP..HF..": all(bool(p["writer_markers"]) for p in parts + prods),
        "MechanicalPart in every part": all(p["count_mech_part"] >= 1 for p in parts),
        "CATMechProdCont in every product": all(p["count_mech_prod_cont"] >= 1 for p in prods),
        "part never labels CATProduct": all(p["count_cls_catproduct"] == 0 for p in parts),
        "product never labels CATPart": all(p["count_cls_catpart"] == 0 for p in prods),
        "product u32@0x54 == 3": all(p["header"].get("0x54") == "0x3" for p in prods),
        "part u32@0x54 in {30, 0x5B00}": all(
            p["header"].get("0x54") in ("0x1e", "0x5b00") for p in parts),
        "ASMPRODUCT present": all(p["count_asmproduct"] >= 1 for p in parts + prods),
        "catalogLinks present": all(p["count_cataloglinks"] >= 1 for p in parts + prods),
        "product carries _InstanceName": all(p["count_instancename"] >= 1 for p in prods),
        "product carries CtorToProd/ProdToRep": all(
            p["count_c2p"] >= 1 and p["count_p2r"] >= 1 for p in prods),
        "product never contains child .CATPart filename": all(
            not p["has_child_catpart"] for p in prods),
        "product no drive-letter path": all(len(p["drive_path_hits"]) == 0 for p in prods),
    }

    # photo/preview batch: 5 motor-blower parts carry a JPEG header at 0x44+
    jpeg_parts = [probes[i]["id"] for i in ok_ids
                  if probes[i]["is_part"] and probes[i]["jpeg_at_header"]]

    report = {
        "task": "P5-M1-T01",
        "method": "byte-observed, clean-room; era labels from in-file writer markers",
        "corpus": {
            "manifest_entries_native": len(native_entries(m)),
            "resolved": len(ok_ids),
            "parts": n_parts,
            "products": n_prods,
            "unresolved": len(miss_ids),
            "unresolved_detail": miss_ids,
            "unique_sha256": len({m_.get("sha256") for m_ in native_entries(m)}),
        },
        "era_census": {e: len(ids) for e, ids in sorted(eras.items())},
        "era_files": {e: ids for e, ids in sorted(eras.items())},
        "r2026_leg": {
            "status": "blocked_on_sample" if not eras.get("R2026") else "observed",
            "samples": len(eras.get("R2026", [])),
        },
        "writer_provenance": {
            "writer_markers_all": sorted({m for p in parts + prods for m in p["writer_markers"]}),
            "catiav5r_all": sorted({m for p in parts + prods for m in p["catiav5r"]}),
            "build_dates_all": sorted({m for p in parts + prods for m in p["build_dates"]}),
        },
        "notebook_claims_confirmed": claims,
        "header_histograms": header_histograms(probes),
        "jpeg_preview_batch": jpeg_parts,
        "jpeg_preview_n": len(jpeg_parts),
    }
    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("repo", nargs="?", default=None, help="corpus root (default: cwd/repo root)")
    ap.add_argument("--json", default=None, help="write the report JSON here")
    args = ap.parse_args(argv)
    report = run(repo=args.repo)
    text = json.dumps(report, indent=2, default=str)
    if args.json:
        with open(args.json, "w") as fh:
            fh.write(text + "\n")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
