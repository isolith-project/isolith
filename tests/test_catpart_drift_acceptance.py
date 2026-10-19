#!/usr/bin/env python3
"""P5-M1-T01 acceptance test: CATPart full-corpus re-run + schema-drift notes.

TDD contract (written before the implementation; RED first, then GREEN on real
material):
  * python/oep_tooling/catpart_drift.py exists and fingerprints real
    CATPart/CATProduct files, resolving the full native corpus from
    corpus/manifest.json (P1-M1-T04) through the object-store-backed registry.
  * docs/re-notes/catpart-catproduct-container-notebook.md has a drift-notes
    section (P5-M1-T01) carrying confidence tags ([high] / [uncertain] /
    blocked_on_sample) and byte-observed counts.
  * On the full corpus every v1.0 claim is re-verified at 131/131 and the era
    census is reported honestly: all files are V5R29 (older era) and the
    R2026 leg is blocked_on_sample because the corpus contains no real
    R2026-era sample (P1-M2-T02 honesty rule).
  * Tolerant when the object-store mirror is not resolvable (SKIP, exit 0) so
    the tree stays green without the corpus; when the corpus IS present the
    facts must hold exactly (FAIL loud otherwise).

Run:  python3 -m pytest -q tests/test_catpart_drift_acceptance.py
      python3 tests/test_catpart_drift_acceptance.py
"""
import os
import re
import struct
import sys
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

NOTEBOOK = os.path.join(REPO, "docs", "re-notes",
                        "catpart-catproduct-container-notebook.md")
MANIFEST = os.path.join(REPO, "corpus", "manifest.json")
NATIVE_FORMATS = ("catpart-v5", "catproduct-v5")

V5_MAGIC = b"V5_CFV2\x00"
WRITER_RE = re.compile(rb"V5R\d\d(?:SP\d+)*HF\d*")


def native_manifest_entries():
    import json
    with open(MANIFEST, encoding="utf-8") as fh:
        return [e for e in json.load(fh).get("entries", [])
                if e.get("format") in NATIVE_FORMATS]


def resolve_native(manifest=None):
    """Resolve every native entry via the drift tool (returns its scan).
    Tolerant: (None, detail) when tool or corpus is unavailable."""
    try:
        import oep_tooling.catpart_drift as D
    except Exception as exc:  # RED until the module exists
        return None, "catpart_drift missing: %s" % exc
    try:
        m = D.registry.load_manifest(repo=REPO)
    except Exception as exc:
        return None, "manifest unreadable: %s" % exc
    probes, ok_ids, miss_ids = D.scan_corpus(m)
    return (probes, ok_ids, miss_ids), None


class TestDriftToolExists(unittest.TestCase):
    def test_module_importable_with_expected_api(self):
        try:
            import oep_tooling.catpart_drift as m
        except Exception as exc:
            self.fail("python/oep_tooling/catpart_drift.py missing: %s" % exc)
        for need in ("fingerprint", "scan_corpus", "run", "era_of"):
            self.assertTrue(hasattr(m, need), "tool missing API: %s" % need)
        self.assertTrue(hasattr(m, "MAGIC_V5"), "tool missing MAGIC_V5")


class TestNotebookHasDriftNotes(unittest.TestCase):
    def test_notebook_present(self):
        self.assertTrue(os.path.isfile(NOTEBOOK),
                        "notebook missing: %s" % NOTEBOOK)

    def test_drift_section_present_with_confidence_tags(self):
        txt = open(NOTEBOOK, encoding="utf-8").read()
        self.assertIn("P5-M1-T01", txt,
                      "notebook has no P5-M1-T01 drift-notes section")
        for tag in ("[high]", "[uncertain]", "blocked_on_sample"):
            self.assertIn(tag, txt,
                          "drift notes must carry confidence tag %r" % tag)
        # byte-observed counts must literally be in the notebook
        self.assertIn("131/131", txt,
                      "notebook must state the full-corpus observation count")


class TestFullCorpusObservation(unittest.TestCase):
    """Real-material leg: facts must hold exactly on the resolved corpus;
    skips when the mirror is not reachable (tolerant, like the golden runner)."""

    @classmethod
    def setUpClass(cls):
        cls.scan, cls.err = resolve_native()
        from oep_tooling.corpus import store
        if store.object_store_unavailable():
            raise unittest.SkipTest(
                "object-store mirror not provisioned: %s" % store.object_store_unavailable())
        if cls.scan is None or not cls.scan[1]:
            raise unittest.SkipTest("corpus not resolvable: %s" % cls.err)
        cls.probes, cls.ok_ids, cls.miss_ids = cls.scan

    @property
    def parts(self):
        return [self.probes[i] for i in self.ok_ids if self.probes[i]["is_part"]]

    @property
    def prods(self):
        return [self.probes[i] for i in self.ok_ids
                if not self.probes[i]["is_part"]]

    def _all(self, pred):
        return [self.probes[i] for i in self.ok_ids if pred(self.probes[i])]

    def test_every_resolved_file_magic(self):
        for p in self._all(lambda p: True):
            self.assertTrue(p["magic_ok"], "%s magic != V5_CFV2" % p["id"])

    def test_every_resolved_file_writer_marker(self):
        for p in self._all(lambda p: True):
            self.assertTrue(p["writer_markers"],
                            "%s has no V5R writer marker" % p["id"])

    def test_class_exclusivity(self):
        for p in self.parts:
            self.assertGreaterEqual(p["count_mech_part"], 1, p["id"])
            self.assertEqual(p["count_mech_prod_cont"], 0, p["id"])
        for p in self.prods:
            self.assertGreaterEqual(p["count_mech_prod_cont"], 1, p["id"])
            self.assertEqual(p["count_mech_part"], 0, p["id"])

    def test_header_discriminator(self):
        for p in self.prods:
            self.assertEqual(p["header"].get("0x54"), "0x3", p["id"])
        for p in self.parts:
            self.assertIn(p["header"].get("0x54"), ("0x1e", "0x5b00"), p["id"])

    def test_products_carry_edges_and_no_paths(self):
        for p in self.prods:
            self.assertGreaterEqual(p["count_instancename"], 1, p["id"])
            self.assertGreaterEqual(p["count_c2p"], 1, p["id"])
            self.assertGreaterEqual(p["count_p2r"], 1, p["id"])
            self.assertFalse(p["has_child_catpart"], p["id"])
            self.assertEqual(p["drive_path_hits"], [], p["id"])

    def test_full_corpus_era_census_and_r2026_leg(self):
        """When the WHOLE native corpus resolves (131 rows), the era census and
        R2026-leg status are exact."""
        if self.miss_ids:
            self.skipTest("partial corpus only (%d unresolved): %s"
                          % (len(self.miss_ids), self.miss_ids[:3]))
        n = len(self.ok_ids)
        self.assertEqual(n, 131, "expected 131 native rows, resolved %d" % n)
        import oep_tooling.catpart_drift as D
        eras = {}
        for i in self.ok_ids:
            eras.setdefault(D.era_of(self.probes[i]), 0)
            eras[D.era_of(self.probes[i])] += 1
        self.assertEqual(eras.get("V5R29"), 131,
                         "every native row must be V5R29-era: %s" % eras)
        # tiny preview-batch (5 motor-blower parts) with JPEG at header
        jpeg = [self.probes[i]["id"] for i in self.ok_ids
                if self.probes[i]["is_part"] and self.probes[i]["jpeg_at_header"]]
        self.assertEqual(len(jpeg), 5,
                         "expected the 5-file JPEG-preview batch, got %s" % jpeg)
        self.assertEqual(len(set(self.ok_ids)), 131)

    def test_report_runs_and_flags_blocked_r2026(self):
        import oep_tooling.catpart_drift as D
        report = D.run(repo=REPO)
        self.assertEqual(report["r2026_leg"]["status"], "blocked_on_sample")
        self.assertEqual(report["r2026_leg"]["samples"], 0)
        self.assertEqual(report["corpus"]["resolved"], 131)
        self.assertTrue(all(report["notebook_claims_confirmed"].values()),
                        report["notebook_claims_confirmed"])


if __name__ == "__main__":
    unittest.main(verbosity=2)
