#!/usr/bin/env python3
"""P5-M3-T01 acceptance: clean-room B-Rep extraction attempt on real CATParts.

TDD contract (written before the tool; RED first, then GREEN on real
material):
  * python/oep_tooling/catpart_brep_extract.py exists and exposes the core
    observation API (extract_brep_chain / classify_parity /
    to_oep_geo_document).
  * On at least one real corpus CATPart that resolves from corpus/manifest.json
    (P1-M1-T04), the tool extracts a byte-attributable B-Rep chain: the
    rep-graph (MechanicalPart / Shape 1 / BagRep), at least one B-Rep
    record-class marker (BrpRsur surface record), and at least one observed
    analytic surface-family reference, all with pinned byte offsets.
  * The extracted chain maps into the documented oep-geo native model: a
    Document with a Part -> Body (solid) -> Feature records, plus a parity
    report that classifies extraction honestly:
      - records present in the bytes are marked exact with offsets;
      - anything not byte-attributable is marked [uncertain] or
        blocked_on_encoded_payload, never "exact";
      - the tool never fabricates NURBS/analytic coefficients it did not gate.
  * Honesty gate: NO record in the emitted document claims parity "exact"
    without a pinned offset, and no analytic surface is emitted as "exact"
    (the strict structural gate emits candidates as [uncertain] only).
  * Tolerant when the object-store mirror is not reachable (SKIP, exit 0);
    when the corpus IS present the facts must hold exactly (FAIL loud).

Run:  python3 -m pytest -q tests/test_catpart_brep_extract_acceptance.py
      python3 tests/test_catpart_brep_extract_acceptance.py
"""
import json
import os
import sys
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

MANIFEST = os.path.join(REPO, "corpus", "manifest.json")
NATIVE_FORMATS = ("catpart-v5", "catproduct-v5")

V5_MAGIC = b"V5_CFV2\x00"
RENOTE = os.path.join(REPO, "docs", "re-notes",
                      "p5-m3-t01-brep-extraction-attempt.md")


def resolve_native(manifest=None):
    """Resolve native entries via the shared registry (tolerant)."""
    try:
        import oep_tooling.catpart_brep_extract as B
        from oep_tooling.corpus import registry
    except Exception as exc:  # RED until the module exists
        return None, "catpart_brep_extract missing: %s" % exc
    try:
        m = registry.load_manifest(repo=REPO)
    except Exception as exc:
        return None, "manifest unreadable: %s" % exc
    entries = [e for e in m["data"].get("entries", [])
               if e.get("format") in NATIVE_FORMATS]
    resolved = []
    miss = []
    for e in entries:
        p, err = registry.resolve_entry(e, manifest=m)
        if err or not p or not os.path.isfile(p):
            miss.append(e["id"])
            continue
        resolved.append((e, p))
    return (resolved, miss), None


class TestToolExists(unittest.TestCase):
    def test_module_importable_with_expected_api(self):
        try:
            import oep_tooling.catpart_brep_extract as m
        except Exception as exc:
            self.fail("python/oep_tooling/catpart_brep_extract.py missing: %s"
                      % exc)
        for need in ("extract_brep_chain", "classify_parity",
                     "to_oep_geo_document", "observe_one"):
            self.assertTrue(hasattr(m, need), "tool missing API: %s" % need)


class TestNotebookHasNotes(unittest.TestCase):
    def test_notebook_present(self):
        self.assertTrue(os.path.isfile(RENOTE),
                        "RE note missing: %s" % RENOTE)

    def test_notebook_marks_uncertainty(self):
        txt = open(RENOTE, encoding="utf-8").read()
        self.assertIn("P5-M3-T01", txt)
        for tag in ("[uncertain]", "blocked_on_encoded_payload",
                    "no fabricated PASS"):
            self.assertIn(tag, txt,
                          "RE note must carry confidence tag %r" % tag)


class TestRealMaterialExtraction(unittest.TestCase):
    """Real-material leg: at least one native CATPart yields a byte-attributed
    B-Rep chain into the oep-geo model; skip-tolerant without the corpus."""

    @classmethod
    def setUpClass(cls):
        data, cls.err = resolve_native()
        if data is None:
            raise unittest.SkipTest("corpus not resolvable: %s" % cls.err)
        cls.resolved, cls.miss = data
        # pick the first real CATPart (not product) if any
        catparts = [(e, p) for e, p in cls.resolved
                    if e.get("format") == "catpart-v5"]
        if not catparts:
            raise unittest.SkipTest("no real catpart-v5 resolvable")
        cls.entry, cls.path = catparts[0]

    def test_real_catpart_leads_to_brep_chain(self):
        import oep_tooling.catpart_brep_extract as B
        with open(self.path, "rb") as fh:
            data = fh.read()
        self.assertEqual(data[:8], V5_MAGIC, "not a V5_CFV2 file")
        chain = B.extract_brep_chain(data)
        # rep-graph observable
        self.assertGreaterEqual(chain["rep_graph"].get("mechanical_part", {}).get("count", 0), 1)
        self.assertGreaterEqual(chain["rep_graph"].get("shape_1", {}).get("count", 0), 1)
        self.assertGreaterEqual(chain["rep_graph"].get("bag_rep", {}).get("count", 0), 1)
        # at least one B-Rep record-class marker (surface record)
        self.assertIn("surface_record", chain["brep_records"],
                      "no BrpRsur surface-record marker observed")
        # every record is offset-pinned
        for rec in chain["brep_records"].values():
            self.assertIsNotNone(rec.get("offset"), "record without offset")
            self.assertGreaterEqual(rec["offset"], 0)
        # at least one body container anchors the B-Rep payload
        self.assertTrue(chain["body_containers"],
                        "no B-Rep body container observed")

    def test_extraction_maps_into_oep_geo(self):
        import oep_tooling.catpart_brep_extract as B
        with open(self.path, "rb") as fh:
            data = fh.read()
        chain = B.extract_brep_chain(data)
        gated = B.gated_analytic_surfaces(data, chain)
        doc = B.to_oep_geo_document(chain, gated, self.path)
        # documented oep-geo header
        self.assertEqual(doc["format"], "oep-geo")
        self.assertEqual(doc["version"], 1)
        self.assertEqual(doc["units"], "mm")
        # part -> body -> features chain present and referencing correctly
        self.assertEqual(len(doc["parts"]), 1)
        part = doc["parts"][0]
        self.assertEqual(part["body_ids"], [doc["bodies"][0]["id"]])
        self.assertEqual(doc["bodies"][0]["kind"], "solid")
        self.assertTrue(doc["features"],
                        "no surface-family features mapped into oep-geo")
        # feature references resolve
        body_ids = {b["id"] for b in doc["bodies"]}
        for f in doc["features"]:
            self.assertTrue(all(bid in body_ids for bid in f["body_ids"]),
                            "feature body reference unresolved")

    def test_no_fabricated_pass(self):
        import oep_tooling.catpart_brep_extract as B
        with open(self.path, "rb") as fh:
            data = fh.read()
        chain = B.extract_brep_chain(data)
        gated = B.gated_analytic_surfaces(data, chain)
        parity = B.classify_parity(chain, gated)
        doc = B.to_oep_geo_document(chain, gated, self.path)
        # Every feature with parity "exact" must carry a pinned offset and an
        # honest evidence string; anything without evidence cannot say "exact".
        for f in doc["features"]:
            p = f.get("parameters", {})
            if p.get("parity") == "exact":
                self.assertTrue(p.get("offsets") or p.get("count"),
                                "exact feature without byte evidence")
                self.assertIn("byte-observed", p.get("evidence", ""))
        # The tool NEVER claims exact NURBS coefficients it did not decode.
        self.assertEqual(parity["exact_geometry_coefficients"],
                         "blocked_on_encoded_payload")
        conf = parity["confidence"]["nurbs_control_points"]
        self.assertIn("[uncertain]", conf)
        # Gated analytic candidates must be marked [uncertain], never exact.
        for s in gated:
            self.assertIn("uncertain", s.get("parity", "").lower())

    def test_report_written_consistently(self, tmpdir=None):
        import tempfile
        import oep_tooling.catpart_brep_extract as B
        with tempfile.TemporaryDirectory() as td:
            out = os.path.join(td, "report.json")
            rc = B.main([self.path, "--json", out])
            self.assertEqual(rc, 0)
            with open(out, encoding="utf-8") as fh:
                report = json.load(fh)
            self.assertEqual(report["single"]["file"],
                             os.path.basename(self.path))


if __name__ == "__main__":
    unittest.main(verbosity=2)
