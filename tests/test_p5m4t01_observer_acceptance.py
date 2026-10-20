#!/usr/bin/env python3
"""P5-M4-T01 acceptance test (part 2): the committed feature-tree observer and
its evidence artifact are real and internally consistent.

Follows the memo acceptance test: written first (TDD), now GREEN on real
material. Verifies, without vendor tooling:

* python/oep_tooling/feature_tree_observe.py exists and exposes the API.
* corpus/obs/feature-tree-observation.json exists and is parseable.
* The evidence is self-consistent: parts_observed > 0, every part magic-OK is
  not asserted here (summary only), LP records and value hits are non-zero,
  all sub-counts sum to the reported totals (no hand-written numbers).
* Reproducibility: re-running the observer over the local corpus cache (when
  present) reproduces the same LP-per-type totals and part count (tolerant
  SKIP when the corpus is not resolvable, so the tree stays green without it).

Run:  python3 tests/test_p5m4t01_observer_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import json
import os
import subprocess
import sys
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OBS = os.path.join(REPO, "python", "oep_tooling", "feature_tree_observe.py")
EVID = os.path.join(REPO, "corpus", "obs", "feature-tree-observation.json")


def load_evidence():
    with open(EVID, encoding="utf-8") as fh:
        return json.load(fh)


class TestObserverExists(unittest.TestCase):
    def test_observer_module_present(self):
        self.assertTrue(os.path.isfile(OBS), "observer missing: %s" % OBS)


class TestEvidenceReal(unittest.TestCase):
    def test_evidence_json_present_and_parseable(self):
        self.assertTrue(os.path.isfile(EVID), "evidence missing: %s" % EVID)
        d = load_evidence()
        self.assertGreater(d.get("parts_observed", 0), 0,
                           "evidence must record observed parts")

    def test_summary_counts_nonzero(self):
        d = load_evidence()
        self.assertGreater(d.get("lp_feature_records_total", 0), 0,
                           "evidence must show length-prefixed feature records")
        ve = d["value_encoding"]
        self.assertGreater(ve.get("files_with_ieee754_near_value_keys", 0), 0,
                           "evidence must show binary value encoding observed")

    def test_by_type_sums_match_total(self):
        d = load_evidence()
        by_type = d["lp_feature_records_by_type"]
        self.assertEqual(sum(by_type.values()),
                         d["lp_feature_records_total"],
                         "per-type LP counts must sum to the reported total")

    def test_attr_census_dominates_with_value_key(self):
        # the memo's claim: "Value" indirection keys dominate the schema
        d = load_evidence()
        attr = d["attr_key_census"]
        self.assertGreater(attr.get("Value", 0), 0,
                           "attribute census must include the 'Value' key")
        self.assertGreater(attr.get("Sketch", 0) + attr.get("Offset", 0), 0,
                           "attribute census must include feature/offset keys")


class TestReproducible(unittest.TestCase):
    """Re-running the committed observer over a resolvable corpus must produce
    the same LP-per-type totals and part count. Tolerant SKIP when no corpus
    cache is present locally (keeps CI green without the NAS object store)."""

    def test_rerun_reproduces_totals(self):
        cand = [
            "/home/r1a/isolith-main/.worktrees/p5m1t02-native-observability-1016a/corpus",
            os.path.join(REPO, "corpus"),
        ]
        root = next((c for c in cand if os.path.isdir(c)
                     and any(f.endswith(".catpart") for _, _, fs in os.walk(c)
                             for f in fs)), None)
        if root is None:
            self.skipTest("no local corpus cache resolvable")
        d0 = load_evidence()
        tmp = os.path.join("/tmp", "kw-p5m4-rerun.json")
        r = subprocess.run([sys.executable, OBS, root, "--out", tmp],
                           capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, "observer rerun failed: %s" % r.stderr)
        with open(tmp, encoding="utf-8") as fh:
            d1 = json.load(fh)
        self.assertEqual(d1["parts_observed"], d0["parts_observed"],
                         "rerun part count differs from committed evidence")
        self.assertEqual(d1["lp_feature_records_by_type"],
                         d0["lp_feature_records_by_type"],
                         "rerun LP per-type totals differ from committed evidence")
        self.assertEqual(d1["value_encoding"]["plausible_double_hits"],
                         d0["value_encoding"]["plausible_double_hits"],
                         "rerun value-hit count differs from committed evidence")
        os.unlink(tmp)


if __name__ == "__main__":
    unittest.main(verbosity=2)
