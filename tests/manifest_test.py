#!/usr/bin/env python3
"""P1-M1-T04 acceptance test: Isolith corpus manifest.

TDD: this file documents the acceptance criteria BEFORE the manifest exists, so
it MUST FAIL (RED) when run against a tree with no corpus/manifest.json, then
PASS (GREEN) once the manifest is built on real material.

'Done' for P1-M1-T04 means:
  - corpus/manifest.json (schema v1, inherited from P0-M4-T02) lists every
    redistributable shared-corpus sample with source, license, format, size,
    sha256 and geometry class;
  - every backend=repo entry proves its integrity on the real on-disk file
    (size + sha256 match);
  - cross-format coverage is complete: all CATPart/CATProduct files, all
    redistributable 3DXML files and the box.stl fixture are registered;
  - the non-redistributable reference samples (11 CGR + 2 reference-only
    3DXML) are catalogued in corpus/inventory.json as role=reference with
    geometry class, and their CGR hashes are re-verified on real local copies
    when present (no silent drift);
  - the committed manifest is deterministic: regenerating it from the sibling
    registries + on-disk bytes yields a byte-identical file.

Pure stdlib. Runs under `python3 tests/manifest_test.py` and pytest.
Exit code non-zero on any failure.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "corpus", "manifest.json")
INVENTORY = os.path.join(ROOT, "corpus", "inventory.json")
BUILDER = os.path.join(ROOT, "corpus", "tools", "build_manifest.py")
CATPART_DIR = os.path.join(ROOT, "corpus", "data", "catpart")
THREEDXML_DIR = os.path.join(ROOT, "corpus", "data", "3dxml")
CGR_REGISTRY = os.path.join(ROOT, "workstreams/format-corpus/samples", "cgr-sample-registry.json")

# Real CGR copies verified by P1-M1-T01 live on the U1 dev disk (they are NOT
# redistributable, so they live outside the repo). When present, the test
# re-verifies the manifest/inventory hash against the real bytes.
CGR_CACHE_DIRS = [
    "/home/r1a/.hermes/cache/scratch/cfysa",
    "/home/r1a/.hermes/cache/scratch/piston",
]

GEOMETRY_CLASSES = ("prism", "surface", "assembly", "dataset", "text")
BACKENDS = ("repo", "object-store")
OPEN_LICENSES = ("MIT", "Apache-2.0", "CC0-1.0", "BSD-3-Clause", "BSD-2-Clause")


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def repo_entry_paths(entries):
    """Resolved absolute paths of every backend=repo entry."""
    for e in entries:
        if e.get("backend") == "repo":
            yield os.path.join(ROOT, "corpus", e["path"].lstrip("/"))


class TestManifestSchema(unittest.TestCase):
    def setUp(self):
        self.assertTrue(os.path.isfile(MANIFEST),
                        "corpus/manifest.json missing (acceptance is RED)")
        self.m = load(MANIFEST)

    def test_schema_version(self):
        self.assertEqual(self.m.get("schema_version"), 1)

    def test_meta_describes_formats_and_clean_room(self):
        desc = (self.m.get("description", "") + " "
                + json.dumps(self.m.get("meta", {}))).lower()
        for anchor in ("source", "license", "geometry", "clean-room"):
            self.assertIn(anchor, desc, "manifest missing anchor: %s" % anchor)
        self.assertEqual(self.m.get("task"), "P1-M1-T04")

    def test_entries_present(self):
        es = self.m.get("entries", [])
        self.assertGreaterEqual(len(es), 139,
                                "expected >=139 entries, have %d" % len(es))

    def test_entry_shape_and_vocab(self):
        es = self.m["entries"]
        ids = set()
        paths = set()
        required = ("id", "path", "format", "geometry_class",
                    "source", "license", "size", "sha256", "backend")
        for e in es:
            for k in required:
                self.assertIn(k, e, "entry %s missing field %s" % (e.get("id"), k))
            self.assertIn(e["geometry_class"], GEOMETRY_CLASSES,
                          "bad geometry_class on %s: %s" % (e["id"], e["geometry_class"]))
            self.assertIn(e["backend"], BACKENDS,
                          "bad backend on %s: %s" % (e["id"], e["backend"]))
            self.assertIsInstance(e["size"], int)
            self.assertGreater(e["size"], 0)
            self.assertRegex(e["sha256"], r"^[0-9a-f]{64}$",
                             "sha256 not 64-hex on %s" % e["id"])
            self.assertTrue(e["source"], "empty source on %s" % e["id"])
            self.assertTrue(e["license"], "empty license on %s" % e["id"])
            self.assertTrue(e["path"], "empty path on %s" % e["id"])
            ids.add(e["id"])
            paths.add(e["path"])
        self.assertEqual(len(ids), len(es), "duplicate entry ids")
        self.assertEqual(len(paths), len(es), "duplicate entry paths")

    def test_repo_entries_real_material(self):
        """Integrity on real bytes: size + sha256 of every repo entry == file."""
        es = [e for e in self.m["entries"] if e["backend"] == "repo"]
        self.assertEqual(len(es), 17, "expected 17 repo entries, have %d" % len(es))
        for e in es:
            p = os.path.join(ROOT, "corpus", e["path"].lstrip("/"))
            self.assertTrue(os.path.isfile(p), "repo file absent: %s" % p)
            self.assertEqual(os.path.getsize(p), e["size"],
                             "size mismatch on %s" % e["id"])
            self.assertEqual(sha256_file(p), e["sha256"],
                             "sha256 mismatch on %s" % e["id"])

    def test_cross_format_coverage(self):
        ids = {e["id"] for e in self.m["entries"]}
        catpart = [f for f in os.listdir(CATPART_DIR)
                   if f.lower().endswith((".catpart", ".catproduct"))]
        self.assertEqual(len(catpart), 10, "CATPart/CATProduct files on disk")
        for f in catpart:
            self.assertIn(f, ids, "CATPart file not registered: %s" % f)
        threed = [f for f in os.listdir(THREEDXML_DIR)
                  if f.lower().endswith((".3dxml", ".3drep")) and f.lower() != "readme.md"]
        self.assertEqual(len(threed), 6, "committed 3DXML files on disk")
        for f in threed:
            self.assertIn(f, ids, "3DXML file not registered: %s" % f)
        self.assertIn("box.stl", ids, "box.stl fixture not registered")

    def test_object_store_entries(self):
        es = [e for e in self.m["entries"] if e["backend"] == "object-store"]
        self.assertEqual(len(es), 122, "expected 122 object-store entries")
        for e in es:
            ptr = e.get("object_store")
            self.assertTrue(ptr, "object-store entry missing pointer: %s" % e["id"])
            for k in ("owner", "package", "version", "object"):
                self.assertIn(k, ptr, "pointer missing %s on %s" % (k, e["id"]))
            # content-addressed object names: <sha256>.<ext>
            self.assertEqual(ptr["object"], "%s.%s" % (e["sha256"], ptr["object"].split(".")[-1]),
                             "object name not content-addressed on %s" % e["id"])

    def test_redistribution_discipline(self):
        es = [e for e in self.m["entries"] if e["backend"] == "repo"]
        for e in es:
            self.assertIn(e["license"], OPEN_LICENSES,
                          "repo entry must carry an open license: %s [%s]"
                          % (e["id"], e["license"]))
        # reference-only (non-redistributable) samples are NOT manifest entries
        manifest_ids = {e["id"] for e in self.m["entries"]}
        cgr = load(CGR_REGISTRY)
        for s in cgr.get("samples", []):
            self.assertNotIn(s["id"], manifest_ids,
                             "non-redistributable CGR leaked into manifest: %s" % s["id"])
        for ref in ("press_rink_axe_fta.3dxml", "landinggear.3dxml"):
            self.assertNotIn(ref, manifest_ids,
                             "reference-only 3DXML leaked into manifest: %s" % ref)


class TestReferenceInventory(unittest.TestCase):
    def setUp(self):
        self.assertTrue(os.path.isfile(INVENTORY),
                        "corpus/inventory.json missing (acceptance is RED)")
        self.inv = load(INVENTORY)

    def test_inventory_schema(self):
        self.assertEqual(self.inv.get("schema_version"), 1)
        self.assertIn("samples", self.inv)

    def test_cgr_reference_coverage(self):
        cgr = load(CGR_REGISTRY)
        cgr_ids = {s["id"] for s in cgr.get("samples", [])}
        self.assertEqual(len(cgr_ids), 11)
        ref = [s for s in self.inv["samples"] if s.get("corpus_role") == "reference"]
        inv_ids = {s["id"] for s in ref}
        self.assertTrue(cgr_ids <= inv_ids,
                        "CGR samples missing from inventory: %s"
                        % (cgr_ids - inv_ids))
        for r in ref:
            self.assertIn(r["geometry_class"], GEOMETRY_CLASSES)
        # the two reference-only 3DXML samples
        for rid in ("press_rink_axe_fta.3dxml", "landinggear.3dxml"):
            self.assertIn(rid, inv_ids, "reference-only 3DXML missing: %s" % rid)

    def test_cgr_metadata_matches_t01_registry(self):
        cgr = load(CGR_REGISTRY)
        by_id = {s["id"]: s for s in cgr.get("samples", [])}
        ref = {s["id"]: s for s in self.inv["samples"]
               if s.get("source_registry") == "cgr-registry"}
        for cid, reg in by_id.items():
            self.assertIn(cid, ref, "inventory missing cgr sample %s" % cid)
            self.assertEqual(ref[cid]["size"], reg["size_bytes"])
            self.assertEqual(ref[cid]["sha256"], reg["sha256"])
            self.assertEqual(ref[cid]["file"], reg["file"])
            self.assertEqual(ref[cid]["redistribution_allowed"],
                             reg.get("redistribution", {}).get("allowed"))

    def test_cgr_real_material_hash(self):
        """If real CGR copies are on the U1 dev disk, their sha256 must match
        the inventory (no drift vs P1-M1-T01 evidence). Skipped when absent."""
        cgr = load(CGR_REGISTRY)
        files = {}
        for d in CGR_CACHE_DIRS:
            if not os.path.isdir(d):
                continue
            for f in os.listdir(d):
                if f.lower().endswith(".cgr"):
                    files.setdefault(f.lower(), os.path.join(d, f))
        if not files:
            self.skipTest("no local CGR cache dir present on this machine")
        for s in cgr.get("samples", []):
            key = s["file"].lower()
            if key not in files:
                continue
            real = sha256_file(files[key])
            self.assertEqual(real, s["sha256"],
                             "CGR drift on real material: %s" % s["file"])


class TestDeterminism(unittest.TestCase):
    def test_builder_check_is_byte_identical(self):
        self.assertTrue(os.path.isfile(BUILDER), "builder missing")
        r = subprocess.run([sys.executable, BUILDER, "--check"],
                           cwd=ROOT, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0,
                         "build_manifest --check failed:\n%s\n%s"
                         % (r.stdout, r.stderr))


if __name__ == "__main__":
    unittest.main(verbosity=2)
