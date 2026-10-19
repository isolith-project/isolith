#!/usr/bin/env python3
"""P10-M3-T02 acceptance test: the committed export-all archive proof is
internally consistent, re-imports into oep-geo, and carries the documented
retention policy defaults.

Pure stdlib + hashlib (mirrors tests/test_p9_m3_export_workers.py). Verifies
the committed proof under docs/P10-M3/proof WITHOUT re-running Rust:
  1. manifest.json  -- oep-archive v1, tool identity, summary rollups,
     every written geometry/model artifact carries a matching sha-256 whose
     on-disk file re-hashes to it; every skip carries a reason.
  2. vault-metadata.json -- oep-vault-export v1; blob content addresses agree
     with the archive's raw/ and geometry/ bytes.
  3. retention-policy.json -- oep-retention-policy v1: defaults on, released
     retained forever, no automatic deletion, purge requires an archive.
  4. checksums.sha256 -- every listed member re-hashes (integrity manifest).
  5. import-report.json -- oep-import-report v1: the vault metadata rebuilt
     (3 items / 3 revisions / 3 files), the faceted STEP re-imported into
     oep-geo (1 part / bodies >= 1), the geometry-less STEP classified as a
     skip, zero failures.
  6. Geometry artifacts are actually there and well-formed (.glb / .stl /
     .3dxml magics and structural size).

Run: python3 tests/test_p10_m3_export_all.py   (or pytest).
"""
import hashlib
import json
import os
import struct
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROOF = os.path.join(REPO, "docs", "P10-M3", "proof")
ARCHIVE = os.path.join(PROOF, "archive")

failures = []


def check(name, ok, detail=""):
    marker = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (marker, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        failures.append(name)


def load(rel, root=ARCHIVE):
    with open(os.path.join(root, rel), encoding="utf-8") as fh:
        return json.load(fh)


def sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def read_bytes(rel, root=ARCHIVE):
    with open(os.path.join(root, rel), "rb") as fh:
        return fh.read()


def main():
    return _run()


def _run():
    # 1) manifest.
    manifest = load("manifest.json")
    check("manifest.format", manifest.get("format") == "oep-archive")
    check("manifest.version", manifest.get("version") == 1)
    tool = manifest.get("tool", {})
    check("manifest.tool", tool.get("name") == "oep-archive")
    check("manifest.source_vault", manifest.get("source_vault", {}).get("owner") == "keelworks-proof")

    summary = manifest.get("summary", {})
    check("summary.items", summary.get("items") == 3)
    check("summary.revisions", summary.get("revisions") == 3)
    check("summary.files", summary.get("files") == 3)
    check("summary.raw_artifacts", summary.get("raw_artifacts") == 3)
    check("summary.geometry_artifacts", summary.get("geometry_artifacts") == 5)
    check("summary.model_artifacts", summary.get("model_artifacts") == 1)
    check("summary.skipped", summary.get("skipped") == 7)

    # Retention snapshot embedded in the manifest.
    ret = manifest.get("retention", {})
    check("manifest.retention", ret.get("format") == "oep-retention-policy" and ret.get("version") == 1)

    # Every written artifact re-hashes to its manifest sha-256; skips carry a reason.
    artifacts = manifest.get("artifacts", [])
    written_geometry = ["box-part-a/1/box-part." + ext for ext in ("step", "glb", "stl", "3dxml")]
    check("artifacts list has all box-part formats",
          all(any(a["path"] == "geometry/" + p for a in artifacts) for p in written_geometry))
    n_skip = 0
    n_written_ok = 0
    for a in artifacts:
        p = a["path"]
        if a.get("skipped"):
            n_skip += 1
            check("skip carries reason: " + p, bool(a.get("reason")))
        else:
            # integrity: every written geometry/model/raw file re-hashes
            if a["role"] in ("geometry", "model", "raw"):
                check("artifact sha ok: " + p,
                      sha256_file(os.path.join(ARCHIVE, p)) == a["sha256"])
            if a["role"] in ("geometry", "model"):
                n_written_ok += 1
    check("written geometry+model count", n_written_ok == 6, str(n_written_ok))  # 5 geometry + 1 model
    check("skipped rows recorded", n_skip == summary.get("skipped"), str(n_skip))

    # 2) vault metadata.
    meta = load("vault-metadata.json")
    check("vault-metadata.format", meta.get("format") == "oep-vault-export")
    check("vault-metadata.version", meta.get("version") == 1)
    items = meta.get("items", [])
    check("vault-metadata.items", len(items) == 3)
    name_to_item = {it["name"]: it for it in items}
    box_part = name_to_item.get("Box Part A")
    check("vault-metadata has Box Part A", box_part is not None)
    if box_part:
        revs = box_part.get("revisions", [])
        check("box-part released rev", len(revs) == 1 and revs[0]["state"] == "released")
        f = revs[0]["files"][0]
        blob = f["blob_sha256"]
        step_on_disk = os.path.join(ARCHIVE, "geometry/box-part-a/1/box-part.step")
        mini_raw = os.path.join(ARCHIVE, "geometry/box-mini-b/1/box-mini.step")
        check("blob sha agrees with geometry step", sha256_file(step_on_disk) == blob)
        check("blob sha agrees with raw step",
              sha256_file(os.path.join(ARCHIVE, "raw/box-part-a/1/box-part.step")) == blob)

    # 3) retention defaults.
    policy = load("retention-policy.json")
    check("retention.format", policy.get("format") == "oep-retention-policy")
    check("retention.version", policy.get("version") == 1)
    check("retention.defaults", policy.get("defaults") is True)
    deletion = policy.get("deletion", {})
    check("retention.no autodelete", deletion.get("automatic_deletion_enabled") is False)
    check("retention.purge needs archive", deletion.get("purge_requires_archive_export") is True)
    released = [r for r in policy.get("rules", []) if r["state"] == "released"]
    check("retention.released kept forever", released and released[0]["rule"]["keep_forever"] is True)

    # 4) checksums.
    sums_text = read_bytes("checksums.sha256").decode()
    lines = [ln for ln in sums_text.splitlines() if ln.strip()]
    check("checksums non-empty", len(lines) >= 10)
    for ln in lines:
        digest, path = ln.split("  ", 1)
        check("checksum ok: " + path,
              sha256_file(os.path.join(ARCHIVE, path)) == digest)

    # 5) import report (the re-import acceptance).
    report = load("import-report.json", root=PROOF)
    check("import-report.format", report.get("format") == "oep-import-report")
    check("import-report.version", report.get("version") == 1)
    vs = report.get("vault", {})
    check("vault rebuilt", vs.get("items_rebuilt") == 3 and vs.get("revisions_rebuilt") == 3
          and vs.get("files") == 3 and vs.get("ok") is True)
    rs = report.get("summary", {})
    check("reimport summary", rs.get("total_step") == 2 and rs.get("reimported") == 1
          and rs.get("skipped") == 1 and rs.get("failed") == 0 and rs.get("ok") is True)
    rows = report.get("documents", [])
    box_part_row = [d for d in rows if d["source"].endswith("box-part.step")]
    check("box-part reimport row", len(box_part_row) == 1
          and box_part_row[0]["status"] == "reimported"
          and box_part_row[0]["valid"] is True and box_part_row[0]["parts"] >= 1)

    # 6) geometry bytes well-formed.
    glb = read_bytes("geometry/box-part-a/1/box-part.glb")
    check("glb magic", glb[:4] == b"glTF")
    stl = read_bytes("geometry/box-part-a/1/box-part.stl")
    tri_count = struct.unpack("<I", stl[80:84])[0]
    check("stl structural size", tri_count > 0 and len(stl) == 84 + tri_count * 50)
    arc = read_bytes("geometry/box-part-a/1/box-part.3dxml")
    check("3dxml zip magic", arc[:2] == b"PK")

    sys.exit(1 if failures else 0)


if __name__ == "__main__":
    main()
