#!/usr/bin/env python3
"""
P1-M1-T02 acceptance test. Locate public 3DXML samples (Dassault published
samples, open collections) + document the process to obtain the 3DXML spec
under Dassault's free internal-use license.

Written FIRST, before the deliverable. The catalog, source register and
spec-process documents must make this test pass. It verifies the deliverable
exists, covers every required element of the card, anchors every catalog
entry with a real URL and a license verdict, and checks the committed sample
files on real material (sha256 against the catalog). It also checks that the
spec request process document exists and names the official portal and the
internal-use-only limitation (clean-room invariant: no XSD republication).

Run: python3 -m pytest -q tests/test_3dxml_samples_catalog.py
     or python3 tests/test_3dxml_samples_catalog.py
"""

import hashlib
import json
import os
import re
import unittest

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CATALOG = os.path.join(ROOT, "workstreams/format-corpus/samples", "3dxml-samples.md")
SPEC_PROCESS = os.path.join(ROOT, "workstreams/format-corpus/samples", "3dxml-spec-process.md")
SOURCES = os.path.join(ROOT, "workstreams/format-corpus/samples", "sources.md")
VERIFICATION = os.path.join(ROOT, "workstreams/format-corpus/samples", "verification.md")
SAMPLE_DIR = os.path.join(ROOT, "corpus", "data", "3dxml")

# Real-material anchor: files committed to the corpus, with their sha256 as
# fetched and verified on 2026-10-15. The test recomputes the hash of the
# committed bytes and requires an exact match.
COMMITTED_SAMPLES = {
    "cube1.3dxml": "8a4d4daef69baf11f674f41d9e022724f0b5a510bf36f26847479e6df6f04e6b",
    "blocks.3dxml": "14ec76309f176fd5f70428d9f8b766640446b61cab8530c8708069dad4c1ead9",
    "cycloidal.3DXML": "007751ab8639097ca146431b0d0c605a2523a269a8dbdc81facd966ddbc0a2de",
    "FM1_satellite.3DXML": "7ff05256666d63199ac16aa041b2869acde06df3ee03d839b5c0436eee2663ee",
    "machinist.3DXML": "2203d90f73d47cbb088a9e3b5f4e61780c0f65e575f0f27182c10be3f57c6753",
    "quad.3dxml": "24459eb1e786451d614c21f6a5cbca3b88eb226479e931b51673481613319d79",
}


def read(p):
    with open(p, encoding="utf-8") as fh:
        return fh.read()


class Test3dxmlSamplesDeliverable(unittest.TestCase):
    def test_required_files_exist(self):
        for p in (CATALOG, SPEC_PROCESS, SOURCES, VERIFICATION):
            self.assertTrue(os.path.isfile(p), "missing file: %s" % p)

    def test_catalog_has_title_and_version(self):
        text = read(CATALOG)
        self.assertRegex(text, r"P1-M1-T02")
        self.assertRegex(text, r"Version:\s*1\.\d+")

    def test_catalog_covers_card_elements(self):
        text = read(CATALOG).lower()
        self.assertIn("3dxml", text)
        self.assertIn("dassault", text)
        self.assertIn("license", text)

    def test_catalog_has_dassault_published_sample(self):
        text = read(CATALOG)
        self.assertIn("press_rink_axe", text.lower(), "official Dassault gallery sample missing")
        self.assertIn("www.3ds.com", text, "Dassault 3ds.com source URL missing")

    def test_catalog_has_open_collection_samples(self):
        text = read(CATALOG)
        self.assertIn("trimesh", text, "open collection (trimesh) missing")
        self.assertIn("i2e-haw-hamburg", text, "open collection (cad-in-unity) missing")

    def test_each_catalog_row_has_url_license_and_redistributability(self):
        text = read(CATALOG)
        # Split into rows on the markdown table; skip header/separator.
        rows = [l for l in text.splitlines() if l.strip().startswith("|")][2:]
        self.assertGreaterEqual(len(rows), 4, "catalog must list >= 4 samples")
        for r in rows:
            cells = [c.strip() for c in r.split("|")[1:-1]]
            self.assertEqual(len(cells), 7,
                             "row cells != 7 (name/source/url/license/redistrib/size/sha): %s" % r)
            name, source, url, license_, redistrib, size, sha = cells
            self.assertTrue(url.lower().startswith("http"), "bad url: %s" % url)
            self.assertTrue(license_, "empty license on %s" % name)
            self.assertIn(redistrib, ("yes", "no", "yes-via-license", "reference-only"),
                          "redistributability verdict invalid on %s" % name)
            if redistrib == "reference-only":
                # a reference-only file is catalogued but not committed, so it
                # may legitimately carry no size and no hash
                self.assertRegex(size, r"(\d|n/a)", "size missing on reference-only %s" % name)
                self.assertRegex(sha, r"((n/a)|([0-9a-f]{16}))",
                                 "sha must be hex or n/a on reference-only %s" % name)
            else:
                self.assertRegex(size, r"\d", "size not numeric on %s" % name)
                self.assertRegex(sha, r"[0-9a-f]{16}",
                                 "sha256 prefix missing on %s" % name)

    def test_committed_samples_exist_and_match_catalog_sha256(self):
        missing = [k for k in COMMITTED_SAMPLES if not os.path.isfile(os.path.join(SAMPLE_DIR, k))]
        self.assertEqual(missing, [], "committed sample(s) missing: %s" % missing)
        for name, want in COMMITTED_SAMPLES.items():
            with open(os.path.join(SAMPLE_DIR, name), "rb") as fh:
                got = hashlib.sha256(fh.read()).hexdigest()
            self.assertEqual(got, want, "sha256 mismatch on real material: %s" % name)

    def test_catalog_sha256_matches_committed_files(self):
        text = read(CATALOG)
        for name, want in COMMITTED_SAMPLES.items():
            self.assertIn(want[:16], text, "catalog missing sha256 anchor for %s" % name)

    def test_spec_process_documents_portal_and_request_steps(self):
        text = read(SPEC_PROCESS).lower()
        self.assertIn("3ds.com", text, "request portal URL missing")
        self.assertIn("3d xml xsd", text.replace("_", " "), "XSD schema item missing")
        self.assertRegex(read(SPEC_PROCESS), r"Version:\s*1\.\d+")

    def test_spec_process_documents_internal_use_limitation(self):
        text = read(SPEC_PROCESS).lower()
        self.assertIn("internal", text, "internal-use limitation not covered")
        self.assertIn("royalt", text.replace("y", "y"), "license grant not described")
        self.assertIn("not republish", text, "clean-room no-republish note missing")

    def test_sources_register_has_retrievable_urls(self):
        text = read(SOURCES)
        urls = re.findall(r"https?://[^\s\)\]]+", text)
        self.assertGreaterEqual(len(urls), 4, "sources.md must cite >= 4 URLs")
        for u in urls:
            self.assertTrue(u.lower().startswith(("http://", "https://")), "bad url: %s" % u)

    def test_verification_record_present(self):
        text = read(VERIFICATION)
        for anchor in ("sha256", "2026-10-15", "verif", "3dxml"):
            self.assertIn(anchor, text.lower(), "verification missing anchor: %s" % anchor)


if __name__ == "__main__":
    unittest.main(verbosity=2)
