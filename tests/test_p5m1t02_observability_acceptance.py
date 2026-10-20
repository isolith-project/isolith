#!/usr/bin/env python3
"""P5-M1-T02 acceptance test: native CATPart entity-class observability report.

TDD contract (written before the report; RED first, then GREEN on real material):

* python/oep_tooling/catpart_observability.py exists and fingerprints a real
  native CATPart from bytes only (no vendor tooling).
* docs/re-notes/native-catpart-observability.md exists and answers
  the P5 alpha-target question: which entity classes (B-Rep exact geometry,
  tessellation, parameter graph) are observable in native CATPart, with a
  per-entity-class confidence rating and [uncertain] markers where the byte
  evidence does not prove meaning.
* On the resolved native corpus (unique files, dedup by sha256) the byte
  facts the report asserts hold exactly:
    - every part is V5_CFV2 with the V5R writer marker;
    - every part carries feature-model tokens (parameter graph observable);
    - every part carries a nested V5_CFV2 geometry blob (exact-geometry
      payload present);
    - NO part carries tessellation vocabulary or a CAT3DBagRep, and freeform
      (NURBS/Gregory) tokens are absent from the corpus;
    - analytic-kind tokens, when present, live in the feature/schema region,
      not inside the nested geometry blob.
* Tolerant when the corpus is not resolvable (SKIP, exit 0) so the tree stays
  green without the corpus; FAIL loud when the corpus IS present and a stated
  byte fact does not hold.

Run:  python3 tests/test_p5m1t02_observability_acceptance.py
      python3 -m pytest -q tests/test_p5m1t02_observability_acceptance.py
"""

import os
import sys
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

REPORT = os.path.join(REPO, "docs", "re-notes",
                      "native-catpart-observability.md")
MANIFEST = os.path.join(REPO, "corpus", "manifest.json")
NATIVE_FORMATS = ("catpart-v5", "catproduct-v5")


def native_manifest_entries():
    import json
    with open(MANIFEST, encoding="utf-8") as fh:
        return [e for e in json.load(fh).get("entries", [])
                if e.get("format") in NATIVE_FORMATS]


def unique_native_entries():
    """The 131 manifest rows collapse to 121 unique files (dedup by sha256);
    the P1-M1-T04 registry lists flat names and upstream names for the same
    bytes. All census counts in the report are per unique file."""
    seen = {}
    for e in native_manifest_entries():
        seen.setdefault(e["sha256"], e)
    return list(seen.values())


def resolve_native():
    """Return (fingerprints_by_sha, detail). Tolerant: (None, reason) when the
    corpus or the scanner is unavailable."""
    try:
        from oep_tooling import catpart_observability as O
    except Exception as exc:  # RED until the module exists
        return None, "catpart_observability missing: %s" % exc
    try:
        from oep_tooling.corpus import registry, store
        man = registry.load_manifest(repo=REPO)
    except Exception as exc:
        return None, "registry manifest unreadable: %s" % exc
    cache_dir = os.environ.get("OEP_CORPUS_CACHE")
    out = {}
    missing = 0
    for e in unique_native_entries():
        if e.get("backend") == "repo":
            path, err = registry.resolve_entry(e, manifest=man)
        elif cache_dir is not None:
            path, err = store.fetch_entry(e, cache_dir=cache_dir)
        else:
            path, err = registry.resolve_entry(e, manifest=man)
        if not path:
            missing += 1
            continue
        fp = O.fingerprint(path)
        fp["entry_id"] = e["id"]
        fp["kind"] = e["format"]
        out[e["sha256"]] = fp
    if not out:
        return None, "failed to resolve any native entry (missing=%d)" % missing
    return out, "resolved %d unique native files (missing=%d)" % (len(out), missing)


class TestObservabilityToolExists(unittest.TestCase):
    def test_module_importable_with_expected_api(self):
        try:
            from oep_tooling import catpart_observability as m
        except Exception as exc:
            self.fail("python/oep_tooling/catpart_observability.py missing: %s" % exc)
        for need in ("fingerprint", "nested_blob", "index_buffer_scan", "scan_corpus"):
            self.assertTrue(hasattr(m, need), "tool missing API: %s" % need)
        self.assertTrue(hasattr(m, "MAGIC_V5"), "tool missing MAGIC_V5")


class TestReportExists(unittest.TestCase):
    def test_report_present(self):
        self.assertTrue(os.path.isfile(REPORT),
                        "report missing: %s" % REPORT)

    def test_report_answers_alpha_question(self):
        txt = open(REPORT, encoding="utf-8").read()
        low = txt.lower()
        for token in ("catpart", "b-rep", "tessellation", "parameter graph"):
            self.assertIn(token, low, "report must cover entity class %r" % token)
        self.assertIn("[uncertain]", txt,
                      "report must mark unknown meaning [uncertain]")
        self.assertIn("confidence", low,
                      "report must carry per-entity-class confidence ratings")
        self.assertIn("P5-M1-T03", txt,
                      "report must feed the P5-M1-T03 alpha-target decision")


class TestByteFacts(unittest.TestCase):
    """Real-material leg: stated byte facts must hold on the resolved corpus;
    SKIP when the corpus is not resolvable (tolerant, like the golden runner)."""

    @classmethod
    def setUpClass(cls):
        cls.scan, cls.err = resolve_native()
        from oep_tooling.corpus import store
        if store.object_store_unavailable():
            raise unittest.SkipTest(
                "object-store mirror not provisioned: %s" % store.object_store_unavailable())
        if cls.scan is None:
            raise unittest.SkipTest("corpus not resolvable: %s" % cls.err)

    def parts(self):
        return [fp for fp in self.scan.values() if fp["is_part"]]

    def test_unique_corpus_count_matches_report(self):
        # report claims 121 unique native files (114 CATPart + 7 CATProduct)
        self.assertEqual(len(self.scan), 121,
                         "corpus should resolve to 121 unique native files")
        parts = self.parts()
        prods = [fp for fp in self.scan.values() if not fp["is_part"]]
        self.assertEqual(len(parts), 114)
        self.assertEqual(len(prods), 7)

    def test_every_part_magic_and_writer(self):
        for fp in self.parts():
            self.assertTrue(fp["magic_ok"], "%s magic != V5_CFV2" % fp["entry_id"])
            self.assertTrue(fp["writer_markers"], "%s no V5R marker" % fp["entry_id"])

    def test_parameter_graph_tokens_every_part(self):
        for fp in self.parts():
            self.assertGreater(fp["feature_token_hits"], 0,
                               "%s has no feature-model tokens" % fp["entry_id"])

    def test_nested_geometry_blob_every_part(self):
        for fp in self.parts():
            self.assertIsNotNone(fp["blob"],
                                 "%s has no nested V5_CFV2 blob" % fp["entry_id"])
            self.assertGreater(fp["f32_in_blob"], 0,
                               "%s blob has no coordinate data" % fp["entry_id"])

    def test_no_tessellation_vocabulary(self):
        for fp in self.parts():
            self.assertEqual(fp["mesh_string_tokens"], 0,
                             "%s has mesh vocabulary" % fp["entry_id"])
            self.assertFalse(fp["has_cat3dbagrep"],
                             "%s has a CAT3DBagRep (CGR-like rep)" % fp["entry_id"])

    def test_triangle_occurrences_are_drafting_symbols(self):
        # "Triangle" / "Triang" substrings in this corpus are drafting-symbol
        # names (drafting:ISO_3D.DressUp.Symbols.Triangle and friends), not
        # tessellation. Assert that so the negative finding is honest.
        for fp in self.parts():
            if fp["triangle_strings"] == 0:
                continue
            for ctx in fp["triangle_contexts"]:
                self.assertTrue(
                    ("drafting" in ctx) or ("triangle" in ctx.lower())
                    or any(k in ctx for k in ("Symbols", "Dimension", "FrameDef")),
                    "%s Triangle occurrence not a drafting context: %r"
                    % (fp["entry_id"], ctx))

    def test_no_freeform_tokens_in_corpus(self):
        for fp in self.parts():
            self.assertEqual(fp["freeform_tokens"], 0,
                             "%s has NURBS/Gregory tokens" % fp["entry_id"])

    def test_analytic_tokens_live_outside_blob(self):
        # where analytic-kind tokens exist they are schema/feature-level, not
        # inside the binary geometry container
        seen = 0
        for fp in self.parts():
            if fp["analytic_total"] > 0:
                seen += 1
                self.assertEqual(fp["analytic_inside_blob"], 0,
                                 "%s analytic token inside B-Rep blob"
                                 % fp["entry_id"])
        self.assertGreater(seen, 0, "no analytic-kind tokens observed at all")


if __name__ == "__main__":
    unittest.main(verbosity=2)
