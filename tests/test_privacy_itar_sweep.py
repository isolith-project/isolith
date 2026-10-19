#!/usr/bin/env python3
"""P1-M1-T06 acceptance test: Privacy/ITAR sweep gates all corpus samples.

Written FIRST (TDD RED). Defines 'done' for P1-M1-T06: a reusable privacy/ITAR
sweep tool exists under python/oep_tooling/, every sample that is destined for
the shared keelworks repo has been swept on its real bytes (sha256-anchored),
the sweep detects planted ITAR/PII markers on synthetic fixtures (positive
control) while ignoring binary float noise (negative control), and a committed
evidence report (P1-M1/privacy-itar-sweep-report.json) documents the verdict
per sample with no unexplained HIGH finding.

Run: python3 -m pytest -q tests/test_privacy_itar_sweep.py
     python3 tests/test_privacy_itar_sweep.py
"""
import json
import os
import struct
import sys
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

REPORT = os.path.join(REPO, "P1-M1", "privacy-itar-sweep-report.json")
CORPUS = os.path.join(REPO, "corpus", "data")

# Ground truth of the repo-destined shared-repo corpus (sha256 of the exact
# bytes committed by P1-M1-T02/T03/T04, recomputed 2026-10-15).
REPO_DESTINED = {
    "box.stl": "37323bbac8e341aad5ef46c2a2c85d230dab35d72353bed3af7665fa604957b0",
    "catpart/3DM016.CATPart": "03f5acd78dc32be43d077a84f991c5edfe98cd8a1ad6e8ed6ca9d61cc602cef1",
    "catpart/hex-nut.CATPart": "28cd2250ab0417585e090c2383e8c8a88c332c92ad6c361508f153a979f8637b",
    "catpart/kj-collar.CATPart": "e51ba83bcc866533091e3eb6b29dc2571e81b9980085bbc75e1554a8dfe0eaa3",
    "catpart/kj-eye-end.CATPart": "58e7ae6a8494d4713c7467d69e4d4a36eb1ee2135e3659a98b1aa3a62ef24e47",
    "catpart/kj-fork-end.CATPart": "b66e7e049611e25111b3ccf5a90acd1e90f93ae4b810cbb12bb8d2e9d721a763",
    "catpart/kj-pin.CATPart": "cbdaf0289f5d30472cff86b5c962296d917fc4e2416db212ca53dd3923d67849",
    "catpart/kj-taper-pin.CATPart": "0573ec6059a4122a49ad4b89fc1314ee0d1a11ee9a8f84d46c7185875b83649d",
    "catpart/knuckle-joint.CATProduct": "c44761ed4d31daf71e43c42f93aa6410c091c02951a23d83964d1e5f018496f3",
    "catpart/square-thread.CATPart": "69d8f1a272648fa0ea732234466144434f1f78344e41185a1b8469027b8cd720",
    "catpart/star.CATPart": "45de7652e4dcdabb3ff6eae52564965aeb2cf90ee64e42a2bd2567250fc5bd71",
    "3dxml/FM1_satellite.3DXML": "7ff05256666d63199ac16aa041b2869acde06df3ee03d839b5c0436eee2663ee",
    "3dxml/README.md": "38894e146cb3802c4d13cced871dcc356a37f5287e1533fa1390812e6c5a382d",
    "3dxml/blocks.3dxml": "14ec76309f176fd5f70428d9f8b766640446b61cab8530c8708069dad4c1ead9",
    "3dxml/cube1.3dxml": "8a4d4daef69baf11f674f41d9e022724f0b5a510bf36f26847479e6df6f04e6b",
    "3dxml/cycloidal.3DXML": "007751ab8639097ca146431b0d0c605a2523a269a8dbdc81facd966ddbc0a2de",
    "3dxml/machinist.3DXML": "2203d90f73d47cbb088a9e3b5f4e61780c0f65e575f0f27182c10be3f57c6753",
    "3dxml/quad.3dxml": "24459eb1e786451d614c21f6a5cbca3b88eb226479e931b51673481613319d79",
}


def sha256_of(path):
    import hashlib
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def make_binary_noise(n=4096):
    """Deterministic float-packed bytes that must NOT trip the detector."""
    import random
    rnd = random.Random(1234)
    return struct.pack("<%df" % n, *[rnd.uniform(-1e4, 1e4) for _ in range(n)])


class TestSweepToolExists(unittest.TestCase):
    def test_module_importable(self):
        try:
            import oep_tooling.privacy_itar_sweep as m
        except Exception as exc:  # RED until the module is written
            self.fail("python/oep_tooling/privacy_itar_sweep.py missing: %s" % exc)
        for need in ("scan_bytes", "sweep_file", "verdict_for", "main"):
            self.assertTrue(hasattr(m, need), "tool missing API: %s" % need)


class TestDetectorSensitivity(unittest.TestCase):
    """Positive controls: planted ITAR/classified/PII markers must be flagged."""

    def _run(self, data, name="fixture.bin"):
        import oep_tooling.privacy_itar_sweep as m
        return m.scan_bytes(name, data)

    def test_planted_itar_text(self):
        findings = self._run(b"the leading edge is ITAR controlled data\n", "a.txt")
        self.assertTrue(any(f.severity == "HIGH" and "ITAR" in f.match
                           for f in findings), "ITAR in ASCII text not flagged")

    def test_planted_cui_text(self):
        findings = self._run(b"// CUI/FOUO MANDATORY\n", "b.txt")
        self.assertTrue(any(f.severity == "HIGH" and "CUI" in f.match.upper()
                           for f in findings), "CUI not flagged")

    def test_planted_email_text(self):
        findings = self._run(b"contact john.smith@example.com now\n", "c.txt")
        self.assertTrue(any(f.severity == "HIGH" for f in findings),
                        "email not flagged")

    def test_planted_itar_in_utf16_binary(self):
        blob = make_binary_noise() + "ITAR RESTRICTED".encode("utf-16-le")
        findings = self._run(blob, "d.cgr")
        self.assertTrue(any(f.match and "ITAR" in f.match.upper()
                            for f in findings), "ITAR in UTF-16 binary not flagged")

    def test_planted_email_in_utf16_binary(self):
        blob = make_binary_noise() + "owner r.doe@example.org".encode("utf-16-le")
        findings = self._run(blob, "e.cgr")
        self.assertTrue(any(f.severity == "HIGH" for f in findings),
                        "email in UTF-16 binary not flagged")


class TestDetectorSpecificity(unittest.TestCase):
    """Negative controls: binary float noise and benign strings must be clean."""

    def _run(self, data, name="noise.bin"):
        import oep_tooling.privacy_itar_sweep as m
        return m.scan_bytes(name, data)

    def test_float_noise_is_clean(self):
        findings = self._run(make_binary_noise(), "n1.bin")
        self.assertEqual([f for f in findings if f.severity == "HIGH"], [])

    def test_version_string_is_not_email(self):
        findings = self._run(b"version 1@1.0 and 6@s.0\n", "v.txt")
        self.assertEqual([f for f in findings if f.severity == "HIGH"], [])

    def test_coordinate_noise_not_flagged(self):
        findings = self._run(b"vertex 32.0755 -65.7144 0.0\n", "coords.txt")
        self.assertEqual([f for f in findings if f.severity == "HIGH"], [])

    def test_benign_cad_strings_clean(self):
        text = (b"CATStorageProperty CATFeatCont SurfacicReps SceneGraph "
                b"Part1 Revol_ThickThin1 V5USERID\n")
        findings = self._run(text, "part.txt")
        self.assertEqual([f for f in findings if f.severity == "HIGH"], [])


class TestRealMaterialReport(unittest.TestCase):
    """Committed evidence report: schema-valid, covers every repo-destined
    sample by sha256, and carries no unexplained HIGH finding."""

    def test_report_exists_and_is_valid_json(self):
        self.assertTrue(os.path.isfile(REPORT), "missing %s" % REPORT)
        with open(REPORT, encoding="utf-8") as fh:
            self.data = json.load(fh)
        self.assertIn("schema_version", self.data)
        self.assertIn("files", self.data)

    def test_every_repo_destined_sample_covered_with_matching_sha(self):
        with open(REPORT, encoding="utf-8") as fh:
            data = json.load(fh)
        by_rel = {os.path.normpath(f["relative_path"]): f for f in data["files"]}
        for rel, expected_sha in REPO_DESTINED.items():
            self.assertIn(os.path.normpath(rel), by_rel,
                          "sample missing from sweep report: %s" % rel)
            got = by_rel[os.path.normpath(rel)]["sha256"]
            self.assertEqual(got, expected_sha,
                             "sha256 drift on %s" % rel)

    def test_no_unexplained_high_finding(self):
        with open(REPORT, encoding="utf-8") as fh:
            data = json.load(fh)
        for f in data["files"]:
            for hit in f.get("findings", []):
                if hit.get("severity") == "HIGH":
                    self.assertTrue(hit.get("disposition"),
                                    "HIGH finding without disposition: %s %s"
                                    % (f["relative_path"], hit.get("match")))


class TestLiveCorpusSweep(unittest.TestCase):
    """When corpus/data is present in-tree, re-run the sweep over every sample
    and require a verdict per file with a matching sha256. Skips on main until
    the corpus PRs merge (tolerant, like the golden-file runner)."""

    def test_live_sweep_verdicts_every_present_sample(self):
        import oep_tooling.privacy_itar_sweep as m
        if not os.path.isdir(CORPUS):
            self.skipTest("corpus/data not merged into this tree yet")
        present = []
        for root, _dirs, files in os.walk(CORPUS):
            for fn in files:
                if fn.startswith("."):
                    continue
                p = os.path.join(root, fn)
                rel = os.path.relpath(p, os.path.dirname(CORPUS))
                verdict = m.verdict_for(p)
                self.assertEqual(verdict["sha256"], sha256_of(p),
                                 "tool sha256 mismatch on %s" % p)
                present.append(rel)
        self.assertTrue(present, "corpus dir present but empty")


if __name__ == "__main__":
    unittest.main(verbosity=2)
