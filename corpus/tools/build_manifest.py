#!/usr/bin/env python3
"""P1-M1-T04: deterministic corpus-manifest builder for the Keelworks corpus.

Consumes the sibling registries and the real on-disk corpus bytes, and emits:

  corpus/manifest.json   schema v1 (P0-M4-T02 corpus harness) - the SHARED
                         corpus: every redistributable sample with source,
                         license, format, size, sha256, geometry class.
  corpus/inventory.json  complete program inventory: shared + reference-only
                         (non-redistributable) samples, role-tagged.
  corpus/inventory.md    human-readable summary of the above.

Inputs (single source of truth; derived data is never trusted as-is):
  corpus/catpart-samples.json            (P1-M1-T03)   10 repo-backed MIT parts
  corpus/catpart-source-full-registry.json (P1-M1-T03) 121 object-store MIT parts
  corpus/tools/3dxml-catalog.json        (P1-M1-T02)   8 samples (6 committed)
  workstreams/format-corpus/samples/cgr-sample-registry.json         (P1-M1-T01)   11 reference-only CGRs
  corpus/data/* real bytes               size + sha256 computed HERE

Invariant: a repo-backed entry whose real bytes disagree with its registry hash
is a hard build failure (no silent drift). Reference-only entries carry the
registry hash; when a local cache of the real CGR binaries is present they are
re-verified too (set CGR_CACHE_DIR to the dir containing the .cgr files).

Determinism (program doctrine #3): `build_manifest.py --check` regenerates all
outputs to temp and diffs against the committed files; exit 0 only if
byte-identical. CI and the P1-M1-T04 acceptance test run this check.

Usage:
  python3 corpus/tools/build_manifest.py            # regenerate in place
  python3 corpus/tools/build_manifest.py --check    # verify committed outputs
Pure stdlib.
"""
import hashlib
import json
import os
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
CORPUS = os.path.join(ROOT, "corpus")
MANIFEST = os.path.join(CORPUS, "manifest.json")
INVENTORY_JSON = os.path.join(CORPUS, "inventory.json")
INVENTORY_MD = os.path.join(CORPUS, "inventory.md")

GEOMETRY_CLASSES = ("prism", "surface", "assembly", "dataset", "text")

# Keelworks-authored fixture entries inherited from P0-M4-T02 (PR #14) so the
# manifest is the union of the whole corpus program. Kept byte-identical to the
# harness PR to make later merges trivially conflict-free.
BOX_STL_SHA = "37323bbac8e341aad5ef46c2a2c85d230dab35d72353bed3af7665fa604957b0"


# Generation date: a CONSTANT, not today's date, so `--check` stays green on
# any machine / any day. Bump this whenever the manifest is regenerated with new
# inputs (the committed outputs embed it; drift detection is byte-identical).
GENERATED = "2026-10-15"


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def norm_geometry_class(raw, cid):
    """Normalize a free-text geometry label to the controlled vocabulary."""
    t = (raw or "").lower()
    if t.startswith("prism") or t.startswith("mechanical-part") or "solid" in t:
        return "prism"
    if t.startswith("assembly"):
        return "assembly"
    if t.startswith("surface") or t in ("dataset", "text"):
        return t
    raise ValueError("cannot map geometry_class %r (entry %s)" % (raw, cid))


def build():
    repo_entries = []   # backend=repo
    obj_entries = []    # backend=object-store

    today = GENERATED

    # --- fixtures from the P0-M4-T02 corpus harness ---------------------
    box_path = os.path.join(CORPUS, "data", "box.stl")
    box_size = os.path.getsize(box_path)
    box_sha = sha256_file(box_path)
    if box_sha != BOX_STL_SHA:
        raise ValueError("box.stl drifted: expected %s got %s" % (BOX_STL_SHA, box_sha))
    repo_entries.append({
        "id": "box.stl",
        "path": "data/box.stl",
        "format": "stl-ascii",
        "geometry_class": "prism",
        "source": "keelworks-authored test fixture (self-written, P0-M4-T02)",
        "license": "CC0-1.0",
        "size": box_size,
        "sha256": box_sha,
        "backend": "repo",
    })
    obj_entries.append({
        "id": "box.stl.object",
        "path": "object/%s.stl" % box_sha,
        "format": "stl-ascii",
        "geometry_class": "prism",
        "source": "keelworks-authored test fixture mirrored to NAS object store (P0-M4-T02)",
        "license": "CC0-1.0",
        "size": box_size,
        "sha256": box_sha,
        "backend": "object-store",
        "object_store": {
            "owner": "don",
            "package": "keelworks-corpus",
            "version": "v1",
            "object": "%s.stl" % box_sha,
        },
    })

    # --- CATPart/CATProduct: repo-backed subset (P1-M1-T03) ------------
    catpart = load(os.path.join(CORPUS, "catpart-samples.json"))["entries"]
    for e in catpart:
        p = os.path.join(CORPUS, "data", "catpart", e["id"])
        real_size = os.path.getsize(p)
        real_sha = sha256_file(p)
        if e["size"] != real_size or e["sha256"] != real_sha:
            raise ValueError("CATPart drift: %s (registry %s/%s vs disk %s/%s)"
                             % (e["id"], e["size"], e["sha256"][:16],
                                real_size, real_sha[:16]))
        repo_entries.append({
            "id": e["id"],
            "path": "data/catpart/%s" % e["id"],
            "format": e["format"],
            "geometry_class": e["geometry_class"],
            "source": e["source"],
            "license": e["license"],
            "size": real_size,
            "sha256": real_sha,
            "backend": "repo",
        })

    # --- CATPart/CATProduct: full object-store set (P1-M1-T03, 121) ----
    full = load(os.path.join(CORPUS, "catpart-source-full-registry.json"))["entries"]
    for e in full:
        ext = e["id"].rsplit(".", 1)[-1].lower() if "." in e["id"] else "bin"
        obj_entries.append({
            "id": e["id"],
            "path": "object/%s.%s" % (e["sha256"], ext),
            "format": e["format"],
            "geometry_class": e["geometry_class"],
            "source": e["source"],
            "license": e["license"],
            "size": e["size"],
            "sha256": e["sha256"],
            "backend": "object-store",
            "object_store": {
                "owner": "don",
                "package": "keelworks-corpus",
                "version": "v1",
                "object": "%s.%s" % (e["sha256"], ext),
            },
        })

    # --- 3DXML (P1-M1-T02): committed subset ---------------------------
    xcat = load(os.path.join(CORPUS, "tools", "3dxml-catalog.json"))["samples"]
    for s in xcat:
        if not s["redistributable"]:
            continue
        p = os.path.join(CORPUS, "data", "3dxml", s["id"])
        real_size = os.path.getsize(p)
        real_sha = sha256_file(p)
        repo_entries.append({
            "id": s["id"],
            "path": "data/3dxml/%s" % s["id"],
            "format": s["format"],
            "geometry_class": s["geometry_class"],
            "source": s["source"],
            "license": s["license"],
            "size": real_size,
            "sha256": real_sha,
            "backend": "repo",
        })

    entries = sorted(repo_entries, key=lambda x: x["id"]) + \
              sorted(obj_entries, key=lambda x: x["id"])

    manifest = {
        "schema_version": 1,
        "description": ("Keelworks shared test-data registry + corpus manifest "
                        "(P1-M1-T04). Every redistributable sample: source, "
                        "license, format, size, sha256, geometry class "
                        "(prism|surface|assembly|dataset|text). Clean-room: "
                        "only redistributable, license-clean samples are "
                        "entries; non-redistributable/reference-only samples "
                        "(11 CGR + 2 3DXML) are catalogued in "
                        "corpus/inventory.json with role=reference and are "
                        "never committed or synced."),
        "task": "P1-M1-T04",
        "generated": today,
        "generated_by": "keelworks worker (default profile)",
        "policy": ("Redistribution granted only under explicit open licenses "
                   "(MIT/Apache-2.0/CC0-1.0). backend=repo files are committed "
                   "under corpus/data/. The 121 MIT CATPart blobs + fixture "
                   "mirror use backend=object-store (NAS Gitea generic package "
                   "registry, content-addressed by sha256) and are uploaded by "
                   "P1-M1-T05."),
        "geometry_classes": "|".join(GEOMETRY_CLASSES),
        "inputs": [
            "workstreams/format-corpus/samples/cgr-sample-registry.json (P1-M1-T01)",
            "corpus/catpart-samples.json + corpus/catpart-source-full-registry.json (P1-M1-T03)",
            "corpus/tools/3dxml-catalog.json (P1-M1-T02)",
            "corpus/data/ real bytes (size+sha256 computed here)",
        ],
        "counts": {
            "total_entries": len(entries),
            "backend_repo": len(repo_entries),
            "backend_object_store": len(obj_entries),
        },
    }
    manifest["entries"] = entries

    # --- reference-only inventory --------------------------------------
    cgr = load(os.path.join(ROOT, "workstreams/format-corpus/samples", "cgr-sample-registry.json"))
    env_cache = os.environ.get("CGR_CACHE_DIR")
    if env_cache:
        cgr_cache_dirs = [env_cache]
    else:
        cgr_cache_dirs = ["/home/r1a/.hermes/cache/scratch/cfysa",
                          "/home/r1a/.hermes/cache/scratch/piston"]
    cgr_cache_dirs = [d for d in cgr_cache_dirs if os.path.isdir(d)]
    cgr_verified = 0
    inv_samples = []
    for s in cgr.get("samples", []):
        real_sha = None
        for d in cgr_cache_dirs:
            fp = os.path.join(d, s["file"].lower())
            if os.path.isfile(fp):
                real_sha = sha256_file(fp)
                if real_sha != s["sha256"]:
                    raise ValueError("CGR drift on real material: %s (registry %s vs disk %s)"
                                     % (s["file"], s["sha256"][:16], real_sha[:16]))
                cgr_verified += 1
                break
        inv_samples.append({
            "id": s["id"],
            "file": s["file"],
            "format": "cgr-v5",
            "geometry_class": norm_geometry_class(s["geometry_class"], s["id"]),
            "geometry_notes": s["identity"]["note"],
            "source": s["source_url"],
            "source_registry": "cgr-registry",
            "license": s["license"],
            "license_verdict": s.get("redistribution", {}).get("license_verdict"),
            "redistribution_allowed": s.get("redistribution", {}).get("allowed"),
            "size": s["size_bytes"],
            "sha256": s["sha256"],
            "corpus_role": "reference",
            "manifest_ref": False,
        })
    for s in xcat:
        if s["redistributable"]:
            continue
        inv_samples.append({
            "id": s["id"],
            "file": s["id"],
            "format": s["format"],
            "geometry_class": s["geometry_class"],
            "geometry_notes": s["geometry_notes"],
            "source": s["source"].split("(")[0].strip() or s["id"],
            "source_registry": "3dxml-catalog",
            "license": s["license"],
            "license_verdict": "reference-only-no-redistribution",
            "redistribution_allowed": False,
            "size": None,
            "sha256": None,
            "corpus_role": "reference",
            "manifest_ref": False,
            "real_material_verified": False,
        })
    # shared samples (manifest entries) are also inventoried, with a manifest_ref
    for e in entries:
        inv_samples.append({
            "id": e["id"],
            "file": e["id"].rsplit("/", 1)[-1],
            "format": e["format"],
            "geometry_class": e["geometry_class"],
            "source": e["source"],
            "source_registry": ("catpart-samples" if e["format"] in ("catpart-v5", "catproduct-v5") and e["backend"] == "repo"
                                else "catpart-full-registry" if e["backend"] == "object-store" and e["id"] != "box.stl.object"
                                else "corpus-harness" if e["id"].startswith("box.")
                                else "3dxml-catalog"),
            "license": e["license"],
            "redistribution_allowed": True,
            "size": e["size"],
            "sha256": e["sha256"],
            "corpus_role": "shared",
            "manifest_ref": True,
            "real_material_verified": e["backend"] == "repo",
        })
    inv_samples.sort(key=lambda x: (x["corpus_role"] != "reference", x["id"]))
    inventory = {
        "schema_version": 1,
        "description": ("Complete Keelworks corpus inventory (shared + "
                        "reference-only). corpus_role=shared samples are "
                        "manifest entries (redistributable); "
                        "corpus_role=reference samples are located + identity-"
                        "verified but NOT redistributable, so they are never "
                        "committed or synced. CGR identity was established on "
                        "real material by P1-M1-T01 (2026-10-15); hash "
                        "re-verification runs in tests/manifest_test.py "
                        "whenever local copies are present."),
        "task": "P1-M1-T04",
        "generated": today,
        "samples": inv_samples,
    }

    inventory_md = render_inventory_md(manifest, inventory, cgr_verified)
    return manifest, inventory, inventory_md


def render_inventory_md(manifest, inventory, cgr_verified):
    es = manifest["entries"]
    repo = [e for e in es if e["backend"] == "repo"]
    obj = [e for e in es if e["backend"] == "object-store"]
    ref = [s for s in inventory["samples"] if s["corpus_role"] == "reference"]

    from collections import Counter
    gc_shared = Counter(e["geometry_class"] for e in es)
    gc_ref = Counter(s["geometry_class"] for s in ref)
    lic_shared = Counter(e["license"] for e in es)

    L = []
    L.append("# Keelworks Corpus Inventory (P1-M1-T04)\n")
    L.append("Version 1.0, %s" % manifest["generated"])
    L.append("\nComplete corpus manifest and inventory: for every located sample, this")
    L.append("records source, license, format, size and geometry class")
    L.append("(`prism | surface | assembly | dataset | text`, vocabulary inherited from")
    L.append("P0-M4-T02). Regenerate with `python3 corpus/tools/build_manifest.py`;\n")
    L.append("`--check` proves the committed files are deterministic.\n")

    L.append("## Coverage\n")
    L.append("- Shared corpus (manifest entries): **%d** (repo %d, object-store %d)"
             % (len(es), len(repo), len(obj)))
    L.append("- Reference-only (not redistributable): **%d** (11 CGR + 2 3DXML)"
             % len(ref))
    L.append("- CGR identity: established on real material by P1-M1-T01 "
             "(2026-10-15); re-verified here whenever local copies are present "
             "(tests/manifest_test.py)")
    L.append("")
    L.append("Geometry-class rollup (shared): " + ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(gc_shared.items())))
    L.append("Geometry-class rollup (reference): " + ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(gc_ref.items())))
    L.append("License rollup (shared): " + ", ".join(
        "%s=%d" % (k, v) for k, v in sorted(lic_shared.items())))
    L.append("")

    L.append("## Shared corpus (corpus/manifest.json)\n")
    L.append("| id | format | geometry_class | license | size | backend |")
    L.append("|---|--------|----------------|---------|------|---------|")
    for e in es:
        L.append("| %s | %s | %s | %s | %d | %s |"
                 % (e["id"], e["format"], e["geometry_class"], e["license"],
                    e["size"], e["backend"]))
    L.append("")

    L.append("## Reference-only (corpus/inventory.json, role=reference)\n")
    L.append("Never committed or synced (license has no redistribution grant). "
             "Obtain from the source URL and verify against the sha256.\n")
    L.append("| id | format | geometry_class | source | license verdict |")
    L.append("|---|--------|----------------|--------|-----------------|")
    for s in ref:
        L.append("| %s | %s | %s | %s | %s |"
                 % (s["id"], s["format"], s["geometry_class"],
                    (s["source"] or "").replace("|", "/")[:70],
                    s.get("license_verdict", "")))
    L.append("")

    L.append("## Clean-room posture\n")
    L.append("Only redistributable, license-clean samples are part of the shared "
             "corpus. The non-redistributable CGR/3DXML reference set stays as "
             "locate+observe evidence (P1-M1-T01/T02). P1-M1-T06 performs the "
             "privacy/ITAR sweep before anything new enters a shared repo.\n")
    return "\n".join(L)


def write_outputs(manifest, inventory, inventory_md):
    payloads = {
        MANIFEST: json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        INVENTORY_JSON: json.dumps(inventory, indent=2, ensure_ascii=False) + "\n",
        INVENTORY_MD: inventory_md + "\n",
    }
    for path, data in payloads.items():
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(data)


def check_outputs():
    manifest, inventory, inventory_md = build()
    expected = {
        MANIFEST: json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
        INVENTORY_JSON: json.dumps(inventory, indent=2, ensure_ascii=False) + "\n",
        INVENTORY_MD: inventory_md + "\n",
    }
    bad = []
    for path, want in expected.items():
        if not os.path.isfile(path):
            bad.append("missing: %s" % path)
            continue
        with open(path, encoding="utf-8") as fh:
            got = fh.read()
        if got != want:
            bad.append("drift: %s" % os.path.relpath(path, ROOT))
    return bad


def main():
    if "--check" in sys.argv:
        bad = check_outputs()
        if bad:
            for b in bad:
                print("[FAIL] " + b)
            print("ACCEPTANCE: FAIL - committed corpus outputs are stale or missing;")
            print("run: python3 corpus/tools/build_manifest.py")
            return 1
        print("ACCEPTANCE: PASS - corpus outputs byte-identical to a fresh build")
        return 0
    write_outputs(*build())
    print("wrote corpus/manifest.json, corpus/inventory.json, corpus/inventory.md")
    return 0


if __name__ == "__main__":
    sys.exit(main())
