#!/usr/bin/env python3
"""P2-M5-T03 acceptance test: regression corpus runner + report matrix.

TDD: this test documents the acceptance criteria BEFORE the runner exists, so it
MUST FAIL (RED) when run against a tree with no `oep_tooling.regression`, then
PASS (GREEN) once the implementation lands on real material.

'Done' for P2-M5-T03 means:
  - every corpus file yields exactly one classified outcome: status OPEN, or a
    SKIP carrying a machine code from the closed taxonomy
      {unsupported-format, not-a-cgr-container, truncated,
       object-store-unresolved, missing-on-disk, no-mesh-found, unknown-format,
       io-error};
  - no file is dropped and no phantom row is added (coverage == truth set);
  - the report matrix (JSON + markdown) is well-formed and self-consistent
    (summary rollup totals match row counts);
  - real material: every CGR sample supplied via OEP_CGR_SAMPLES is classified
    (OPEN or a taxonomy SKIP) with no crash, and the probe path is exercised;
  - tolerant: hostile/empty inputs never crash the sweep (per P2-M1-T04
    tolerant failure model) and exit non-zero iff a row lands in the fail bucket.

Clean-room rule: no proprietary bytes are committed. Every CGR fixture below is
synthesized from constants already published under docs/re-notes (magic
01 00 00 00, 96-byte zero preamble, landscape literal, 16-byte invariant
template) — the same construction used by golden_header.rs upstream.

Pure stdlib. Runs under `python3 tests/corpus_regression_test.py` and pytest.
Exit code 1 on any failure.
"""
import json
import os
import sys
import tempfile
from pathlib import Path

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))
sys.path.insert(0, os.path.join(REPO, "python", "oep_tooling"))

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


TAXONOMY = {
    "unsupported-format", "not-a-cgr-container", "truncated",
    "object-store-unresolved", "missing-on-disk", "no-mesh-found",
    "unknown-format", "io-error",
}

# Observed CGR header constants (published, clean-room).
CGR_MAGIC = bytes([0x01, 0x00, 0x00, 0x00])
CGR_LANDSCAPE = 0x10CD_3600
CGR_TEMPLATE = bytes([0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00,
                      0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F])


def cgr_header_shape(landscape=CGR_LANDSCAPE, template=CGR_TEMPLATE, badmagic=False):
    b = bytearray(0x78 + 16)
    b[0:4] = bytes([0x99, 0x00, 0x00, 0x00]) if badmagic else CGR_MAGIC
    b[0x64:0x68] = landscape.to_bytes(4, "little")
    b[0x68:0x78] = template
    return bytes(b)


def make_fake_entries():
    return [
        {"id": "box.stl", "path": "data/box.stl", "format": "stl-ascii",
         "geometry_class": "prism", "backend": "repo"},
        {"id": "FIXTURE.cgr", "path": "data/FIXTURE.cgr", "format": "cgr",
         "geometry_class": "prism", "backend": "repo"},
        {"id": "legacy.cgr", "path": "data/legacy.cgr", "format": "cgr",
         "geometry_class": "prism", "backend": "repo"},
        {"id": "part.CATPart", "path": "data/part.CATPart", "format": "catpart-v5",
         "geometry_class": "prism", "backend": "repo"},
        {"id": "assembly.CATProduct", "path": "data/assembly.CATProduct",
         "format": "catproduct-v5", "geometry_class": "assembly", "backend": "repo"},
        {"id": "miss.CATPart", "path": "data/miss.CATPart", "format": "catpart-v5",
         "geometry_class": "prism", "backend": "repo"},
        {"id": "few.3dxml", "path": "data/few.3dxml", "format": "3dxml-zip",
         "geometry_class": "prism", "backend": "repo"},
        {"id": "blob.CATPart", "path": "objects/blob.CATPart", "format": "catpart-v5",
         "geometry_class": "prism", "backend": "object-store"},
    ]


def build_fixture_corpus(root):
    """Materialize a deterministic corpus dir from documented constants."""
    corpus = Path(root) / "corpus"
    data = corpus / "data"
    obj = corpus / "objects"
    data.mkdir(parents=True, exist_ok=True)
    obj.mkdir(parents=True, exist_ok=True)
    # stl-ascii
    (data / "box.stl").write_text(
        "solid box\nfacet normal 0 0 1\n  outer loop\n"
        "    vertex 0 0 0\n    vertex 1 0 0\n    vertex 0 1 0\n"
        "  endloop\nendfacet\nendsolid box\n")
    # valid CGR header shape -> OPEN via probe
    (data / "FIXTURE.cgr").write_bytes(cgr_header_shape())
    # legacy CGR: template drift -> classified not-a-cgr-container (or family
    # variant); must be SKIP, never crash.
    drifted_tmpl = bytearray(CGR_TEMPLATE)
    drifted_tmpl[0] ^= 0xFF
    (data / "legacy.cgr").write_bytes(cgr_header_shape(template=bytes(drifted_tmpl)))
    # CATPart magic V5_CFV2 prefix (observed marker), body is dummy
    (data / "part.CATPart").write_bytes(b"V5_CFV2" + b"\x00" * 64)
    (data / "assembly.CATProduct").write_bytes(b"V5_CFV2" + b"\x00" * 64)
    # miss.CATPart intentionally NOT created (missing-on-disk class)
    # 3dxml: zip-magic PK.. + minimal xml (classified unsupported-format, P3 scope)
    (data / "few.3dxml").write_bytes(b"PK\x03\x04dummyzip")
    # object-store entry: blob not materialized locally
    return corpus


# --- 1. module exists and sweeps a corpus ----------------------------------
def test_runner_module_import_and_sweep():
    try:
        import oep_tooling.regression as R
    except Exception as e:  # RED: module absent until implementation
        check("import oep_tooling.regression", False, repr(e))
        return
    check("import oep_tooling.regression", True)
    for name in ("classify_file", "sweep", "build_matrix", "write_matrix",
                 "exit_code", "CLOSED_TAXONOMY"):
        check("runner has %s" % name, hasattr(R, name), name)

    with tempfile.TemporaryDirectory() as td:
        corpus = build_fixture_corpus(td)
        rows = R.sweep(entries=make_fake_entries(), corpus_root=str(corpus),
                       sample_dirs=[], cache_dir=None)
        check("sweep returns rows", isinstance(rows, list) and len(rows) > 0,
              "rows=%d" % len(rows))
        for row in rows:
            st = getattr(row, "status", None)
            tag = getattr(row, "cls", None)
            check("row classified: %s" % getattr(row, "id", "?"),
                  st in ("OPEN", "SKIP") and (st == "OPEN" or tag in TAXONOMY),
                  "status=%r cls=%r" % (st, tag))


# --- 2. coverage: every tested file yields exactly one row -----------------
def test_coverage_no_drop_no_phantom():
    import oep_tooling.regression as R
    with tempfile.TemporaryDirectory() as td:
        corpus = build_fixture_corpus(td)
        rows = R.sweep(entries=make_fake_entries(), corpus_root=str(corpus),
                       sample_dirs=[], cache_dir=None)
        ids = sorted(r.id for r in rows)
        expected = sorted(e["id"] for e in make_fake_entries())
        check("coverage: every entry produces a row",
              ids == expected, "expect %s got %s" % (expected, ids))
        check("coverage: no phantom rows", len(ids) == len(set(ids)))


# --- 3. CGR open probe: deterministic synthetic header ---------------------
def test_cgr_probe_opens_observed_shape():
    import oep_tooling.regression as R
    out = R.probe_cgr_header(cgr_header_shape())
    check("observed CGR header shape OPEN",
          out is True or (isinstance(out, dict) and out.get("ok") is True),
          repr(out))


def test_cgr_probe_rejects_truncated_and_badmagic():
    import oep_tooling.regression as R
    r_trunc = R.probe_cgr_header(b"\x01\x00\x00\x00")
    check("truncated -> not OPEN", r_trunc is not True, repr(r_trunc))
    r_bad = R.probe_cgr_header(cgr_header_shape(badmagic=True))
    check("bad magic -> not OPEN", r_bad is not True, repr(r_bad))


# --- 4. matrix output ------------------------------------------------------
def test_matrix_output_schema_and_rollup():
    import oep_tooling.regression as R
    with tempfile.TemporaryDirectory() as td:
        corpus = build_fixture_corpus(td)
        rows = R.sweep(entries=make_fake_entries(), corpus_root=str(corpus),
                       sample_dirs=[], cache_dir=None)
        matrix = R.build_matrix(rows)
        check("matrix.schema_version == 1", matrix.get("schema_version") == 1)
        check("matrix.rows length == sweep rows",
              len(matrix.get("rows", [])) == len(rows))
        summary = matrix.get("summary", {})
        n_open = sum(1 for r in rows if r.status == "OPEN")
        n_skip = sum(1 for r in rows if r.status == "SKIP")
        check("summary rollup OPEN/SKIP",
              summary.get("open") == n_open and summary.get("skip") == n_skip,
              "summary=%s rows open=%d skip=%d" % (summary, n_open, n_skip))
        check("summary totals match", summary.get("total") == len(rows))
        # writer round-trip: JSON parseable + markdown has a row per file
        outdir = Path(td) / "out"
        outdir.mkdir(parents=True, exist_ok=True)
        R.write_matrix(matrix, str(outdir))
        with open(outdir / "regression-matrix.json", encoding="utf-8") as fh:
            reloaded = json.load(fh)
        check("matrix JSON round-trips",
              len(reloaded.get("rows", [])) == len(rows))
        md = (outdir / "regression-matrix.md").read_text(encoding="utf-8")
        check("matrix markdown written", len(md) > 0 and "|" in md)


# --- 5. exit code: non-zero iff fail bucket -------------------------------
def test_exit_code_zero_with_skips_only():
    import oep_tooling.regression as R
    with tempfile.TemporaryDirectory() as td:
        corpus = build_fixture_corpus(td)
        rows = R.sweep(entries=make_fake_entries(), corpus_root=str(corpus),
                       sample_dirs=[], cache_dir=None)
        has_fail = any(getattr(r, "status", "") == "FAIL" for r in rows)
        check("skips-only run implies exit 0",
              R.exit_code(rows) == 0 and not has_fail)


# --- 6. real material (CI-safe tolerant when samples absent) --------------
def test_real_material_every_cgr_classified():
    import oep_tooling.regression as R
    dirs = []
    env = os.environ.get("OEP_CGR_SAMPLES", "")
    for chunk in env.split(":"):
        if chunk and os.path.isdir(chunk):
            dirs.append(chunk)
    for probe in ("/home/r1a/.hermes/cache/scratch/cfysa",
                  "/home/r1a/.hermes/cache/scratch/piston"):
        if os.path.isdir(probe):
            dirs.append(probe)
    dirs = sorted(set(dirs))
    if not dirs:
        check("real material leg (samples absent, tolerant SKIP)", True,
              "OEP_CGR_SAMPLES unset and no local sample dirs found")
        return
    with tempfile.TemporaryDirectory() as td:
        corpus = build_fixture_corpus(td)
        rows = R.sweep(entries=[], corpus_root=str(corpus),
                       sample_dirs=dirs, cache_dir=None)
        check("real samples sweep produced rows", len(rows) > 0,
              "rows=%d" % len(rows))
        n_open = n_skip = 0
        for r in rows:
            st, tag = r.status, getattr(r, "cls", None)
            if st == "OPEN":
                n_open += 1
            elif st == "SKIP" and tag in TAXONOMY:
                n_skip += 1
            else:
                check("real sample classified: %s" % r.id,
                      False, "status=%r cls=%r" % (st, tag))
        check("all real samples classified (OPEN or taxonomy SKIP)",
              n_open + n_skip == len(rows),
              "open=%d skip=%d rows=%d" % (n_open, n_skip, len(rows)))
        check("at least one real sample OPEN via probe", n_open >= 1,
              "open=%d" % n_open)


# --- 7. hostile input never crashes ---------------------------------------
def test_no_crash_on_hostile_inputs():
    import oep_tooling.regression as R
    with tempfile.TemporaryDirectory() as td:
        corpus = Path(td) / "corpus"
        data = corpus / "data"
        data.mkdir(parents=True)
        (data / "empty.bin").write_bytes(b"")
        (data / "rand.bin").write_bytes(os.urandom(512))
        entries = [
            {"id": "empty.bin", "path": "data/empty.bin", "format": "unknown",
             "geometry_class": "prism", "backend": "repo"},
            {"id": "rand.bin", "path": "data/rand.bin", "format": "unknown",
             "geometry_class": "prism", "backend": "repo"},
        ]
        rows = R.sweep(entries=entries, corpus_root=str(corpus),
                       sample_dirs=[], cache_dir=None)
        check("hostile inputs classified (no crash)",
              all(getattr(r, "status") in ("OPEN", "SKIP") for r in rows),
              "rows=%d" % len(rows))


# ==========================================================================
# P3-M5-T01 — 3DXML container open/classified-skip path
# (same shared runner: python/oep_tooling/regression.py)
# ==========================================================================

# Observed 3DXML container facts (P3-M1-T01 + P1-M2-T04 hex notebook):
#  - ZIP-archive 3DXML: first 4 bytes PK\x03\x04; must contain a member named
#    exactly "Manifest.xml" whose <Root> declares the primary model doc; the
#    declared root must name a member present in the archive (BOM coherence).
#  - Standalone ASCII model document: first bytes "<?xm".
#  - All corpus members are DEFLATE (method 8).
ZIP_MAGIC_PK34 = b"PK\x03\x04"
STANDALONE_XML_PREFIX = b"<?xm"
MANIFEST_NAME = "Manifest.xml"


def make_synthetic_3dxml_zip(manifest_root="Abaqus_Model.3dxml",
                             include_manifest=True, root_present=True,
                             manifest_body=None, gzip_member=False):
    """Build a synthetic ZIP-archive 3DXML from public ZIP/XML shape facts.

    - include_manifest=False -> a zip with NO Manifest.xml member.
    - root_present=False  -> Manifest declares a <Root> that is NOT an archive
      member (BOM incoherence).
    - manifest_body       -> raw bytes to use as Manifest.xml (test escapes).
    - gzip_member         -> a member compressed with method 8 (DEFLATE).
    """
    import io
    import struct
    import zlib

    buf = io.BytesIO()
    members = {}

    if include_manifest:
        if manifest_body is not None:
            man_bytes = manifest_body
        else:
            man_bytes = ('<?xml version="1.0" encoding="utf-8" ?>\n'
                         '<Manifest>\n<Root>%s</Root>\n</Manifest>\n'
                         % manifest_root).encode("utf-8")
        members[MANIFEST_NAME] = man_bytes
    if root_present and manifest_root:
        members[manifest_root] = b"<Model_3dxml/>" + b"\x00" * 16

    # Build a minimal ZIP local-file + central directory by hand (public
    # PKWARE APPNOTE): each member stored (method 0) or DEFLATE (method 8).
    local_offsets = {}
    for name, data in members.items():
        raw = data
        method = 0
        if gzip_member:
            co = zlib.compressobj(9, zlib.DEFLATED, -15)
            raw = co.compress(data) + co.flush()
            method = 8
        name_b = name.encode("utf-8")
        crc = zlib.crc32(data) & 0xFFFFFFFF
        local_offsets[name] = buf.tell()
        buf.write(struct.pack("<IHHHHHIIIHH",
                              0x04034B50, 20, 0x0800, method,
                              0, 0, crc, len(raw), len(data),
                              len(name_b), 0))
        buf.write(name_b)
        buf.write(raw)

    central_start = buf.tell()
    for name, data in members.items():
        raw = data
        method = 0
        if gzip_member:
            co = zlib.compressobj(9, zlib.DEFLATED, -15)
            raw = co.compress(data) + co.flush()
            method = 8
        name_b = name.encode("utf-8")
        crc = zlib.crc32(data) & 0xFFFFFFFF
        buf.write(struct.pack("<IHHHHHHIIIHHHHHII",
                              0x02014B50, 20, 20, 0x0800, method,
                              0, 0, crc, len(raw), len(data),
                              len(name_b), 0, 0, 0, 0, 0,
                              local_offsets[name]))
        buf.write(name_b)

    end = buf.tell()
    n = len(members)
    buf.write(struct.pack("<IHHHHIIH",
                          0x06054B50, 0, 0, n, n, end - central_start,
                          central_start, 0))
    return buf.getvalue()


# --- 8. probe opens synthetic + real 3DXML containers ---------------------
def test_3dxml_probe_opens_synthetic_zip():
    import oep_tooling.regression as R
    blob = make_synthetic_3dxml_zip()
    verdict = R.probe_3dxml_container(blob)
    check("synthetic zip 3DXML OPEN", verdict is True, repr(verdict))
    row = R.classify_file_blob(blob, "3dxml-zip")
    check("classify 3dxml-zip OPEN via probe",
          row.status == "OPEN" and row.reader == "probe" and row.cls == "",
          "row=%s" % (row.as_dict(),))


def test_3dxml_probe_opens_standalone_xml():
    import oep_tooling.regression as R
    blob = b'<?xml version="1.0" encoding="utf-8" ?>\n' \
           b'<Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML"/>'
    verdict = R.probe_3dxml_container(blob)
    check("standalone ascii 3DXML OPEN", verdict is True, repr(verdict))
    row = R.classify_file_blob(blob, "3dxml-ascii")
    check("classify 3dxml-ascii OPEN via probe",
          row.status == "OPEN" and row.reader == "probe",
          "row=%s" % (row.as_dict(),))


def test_3dxml_probe_rejects_zip_without_manifest():
    import oep_tooling.regression as R
    blob = make_synthetic_3dxml_zip(include_manifest=False)
    verdict = R.probe_3dxml_container(blob)
    check("zip without Manifest.xml not OPEN", verdict is not True,
          repr(verdict))
    row = R.classify_file_blob(blob, "3dxml-zip")
    check("zip w/o Manifest.xml -> classified skip",
          row.status == "SKIP" and row.cls in TAXONOMY,
          "row=%s" % (row.as_dict(),))


def test_3dxml_probe_rejects_bom_incoherence():
    import oep_tooling.regression as R
    blob = make_synthetic_3dxml_zip(manifest_root="Ghost.3dxml",
                                    root_present=False)
    verdict = R.probe_3dxml_container(blob)
    check("BOM-incoherent zip not OPEN", verdict is not True, repr(verdict))
    row = R.classify_file_blob(blob, "3dxml-zip")
    check("BOM-incoherent zip -> classified skip",
          row.status == "SKIP" and row.cls in TAXONOMY,
          "row=%s" % (row.as_dict(),))


def test_3dxml_probe_hostile_inputs_no_crash():
    import oep_tooling.regression as R
    cases = {
        "trunc.zip": b"PK\x03\x04",
        "trunc2.3dxml": b"PK\x03\x04\x14\x00",
        "garbage.3dxml": os.urandom(256),
        "empty.3dxml": b"",
    }
    for name, blob in cases.items():
        try:
            verdict = R.probe_3dxml_container(blob)
            row = R.classify_file_blob(blob, "3dxml-zip")
            check("hostile %s classified (no crash)" % name,
                  verdict is not True
                  and row.status in ("OPEN", "SKIP")
                  and (row.status == "OPEN" or row.cls in TAXONOMY),
                  "verdict=%r row=%s" % (verdict, row.as_dict()))
        except Exception as e:  # never crash
            check("hostile %s never raises" % name, False, repr(e))


# --- real material (CI-safe tolerant when samples absent) -----------------
def _write_temp_exe(script_body):
    """Write an executable helper into a temp dir and return its path."""
    import stat as _stat
    import tempfile as _tempfile
    d = _tempfile.mkdtemp()
    p = os.path.join(d, "fake_read_3dxml.sh")
    with open(p, "w") as fh:
        fh.write("#!/bin/sh\n" + script_body)
    os.chmod(p, os.stat(p).st_mode | _stat.S_IXUSR)
    return p


def test_3dxml_cli_escalation_reader_open():
    """OEP_3DXML_READER pointing at a reader that exits 0 -> OPEN, reader=cli."""
    import oep_tooling.regression as R
    script = _write_temp_exe("exit 0\n")
    old = os.environ.get("OEP_3DXML_READER")
    os.environ["OEP_3DXML_READER"] = script
    try:
        blob = make_synthetic_3dxml_zip()
        row = R.classify_file_blob(blob, "3dxml-zip")
        check("OEP_3DXML_READER exit 0 -> OPEN reader=cli",
              row.status == "OPEN" and row.reader.startswith("cli"),
              "row=%s" % (row.as_dict(),))
    finally:
        if old is None:
            os.environ.pop("OEP_3DXML_READER", None)
        else:
            os.environ["OEP_3DXML_READER"] = old


def test_3dxml_cli_escalation_reader_skip():
    """OEP_3DXML_READER exit 3 (not-a-3dxml-container) -> classified skip."""
    import oep_tooling.regression as R
    script = _write_temp_exe("echo nope >&2\nexit 3\n")
    old = os.environ.get("OEP_3DXML_READER")
    os.environ["OEP_3DXML_READER"] = script
    try:
        blob = make_synthetic_3dxml_zip()
        row = R.classify_file_blob(blob, "3dxml-zip")
        check("OEP_3DXML_READER exit 3 -> SKIP reader=cli",
              row.status == "SKIP" and row.cls in ("unknown-format",)
              and row.reader.startswith("cli"),
              "row=%s" % (row.as_dict(),))
    finally:
        if old is None:
            os.environ.pop("OEP_3DXML_READER", None)
        else:
            os.environ["OEP_3DXML_READER"] = old


def test_3dxml_real_material_classified():
    import oep_tooling.regression as R
    dirs = []
    env = os.environ.get("OEP_3DXML_SAMPLES", "")
    for chunk in env.split(":"):
        if chunk and os.path.isdir(chunk):
            dirs.append(chunk)
    for probe in (
        "/home/r1a/.hermes/cache/scratch/t05-stage/data/3dxml",
        "/home/r1a/isolith-main/.worktrees/3dxml-samples-1015a/corpus/data/3dxml",
        "/home/r1a/.hermes/cache/scratch/3dxml-binary-branch",
    ):
        if os.path.isdir(probe):
            dirs.append(probe)
    dirs = sorted(set(dirs))
    if not dirs:
        check("3dxml real-material leg (samples absent, tolerant SKIP)", True,
              "OEP_3DXML_SAMPLES unset and no local sample dirs found")
        return
    with tempfile.TemporaryDirectory() as td:
        # entries for on-disk 3DXML files, resolved under their containing dir
        entries, roots = [], []
        for d in dirs:
            for name in sorted(os.listdir(d)):
                p = os.path.join(d, name)
                if not os.path.isfile(p):
                    continue
                low = name.lower()
                if not (low.endswith(".3dxml") or low.endswith(".3drep")):
                    continue
                fmt = ("3dxml-ascii" if name.lower().endswith("quad.3dxml")
                       else "3dxml-zip")
                entries.append({"id": name,
                                "path": name,
                                "format": fmt,
                                "backend": "repo"})
                roots.append(d)
        # sweep per containing dir (one dir at a time => correct resolution)
        rows = []
        for d in set(roots):
            sub = [e for e, rd in zip(entries, roots) if rd == d]
            rows += R.sweep(entries=sub, corpus_root=d,
                            sample_dirs=[], cache_dir=None)
        check("real 3DXML samples produced rows", len(rows) > 0,
              "rows=%d" % len(rows))
        n_open = sum(1 for r in rows if r.status == "OPEN")
        n_skip = sum(1 for r in rows if r.status == "SKIP"
                     and r.cls in TAXONOMY)
        check("all real 3DXML classified (OPEN or taxonomy SKIP)",
              n_open + n_skip == len(rows),
              "open=%d skip=%d rows=%d" % (n_open, n_skip, len(rows)))
        # the 5 committed ZIP-archive samples must OPEN with BOM coherence
        zip_opens = [r.id for r in rows if r.status == "OPEN"
                     and r.format == "3dxml-zip"]
        check("5 committed ZIP-archive 3DXML OPEN",
              any("blocks" in i for i in zip_opens) and
              any("cube1" in i for i in zip_opens) and
              any("machinist" in i for i in zip_opens) and
              any("cycloidal" in i for i in zip_opens) and
              any("FM1" in i for i in zip_opens),
              "zip opens=%r" % zip_opens)
        # standalone quad.3dxml must OPEN
        ascii_opens = [r.id for r in rows if r.status == "OPEN"
                       and r.format == "3dxml-ascii"]
        check("standalone quad.3dxml OPEN",
              any("quad" in i for i in ascii_opens),
              "ascii opens=%r" % ascii_opens)


if __name__ == "__main__":
    for fn_name in sorted(n for n in dir(sys.modules[__name__]) if n.startswith("test_")):
        fn = getattr(sys.modules[__name__], fn_name)
        fn()
    print("-" * 50)
    if _failures:
        print("RESULT: FAIL (%d)" % len(_failures))
        sys.exit(1)
    print("RESULT: PASS")
    sys.exit(0)
