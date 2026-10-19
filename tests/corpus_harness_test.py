#!/usr/bin/env python3
"""P0-M4-T02 acceptance test: Keelworks shared test-data registry + golden-file runner.

TDD: this test documents the acceptance criteria BEFORE the harness exists, so it
must FAIL (RED) when run against a repo with no corpus harness, then PASS (GREEN)
once the implementation lands.

'Done' for P0-M4-T02 means this test passes against the real tree (real fixture
files in the corpus, real golden outputs, real command execution) AND the
object-store backend round-trips a real blob through the NAS Gitea package
registry with a verified sha256.

Pure stdlib. Runs under `python3 tests/corpus_harness_test.py` and pytest.
Exit code 1 on any failure.
"""
import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


def check_file(path, label=None):
    p = os.path.join(REPO, path)
    check(label or ("exists: " + path), os.path.isfile(p), p)


def sha256_of(path):
    import hashlib
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


# --- 1. Shared test-data registry (manifest) --------------------------------
def test_manifest_exists_and_schema():
    mpath = os.path.join(REPO, "corpus", "manifest.json")
    check("corpus/manifest.json exists", os.path.isfile(mpath), mpath)
    if not os.path.isfile(mpath):
        return
    with open(mpath, encoding="utf-8") as fh:
        data = json.load(fh)
    check("manifest has schema_version", data.get("schema_version") == 1,
          repr(data.get("schema_version")))
    entries = data.get("entries")
    check("manifest.entries is a non-empty list",
          isinstance(entries, list) and len(entries) > 0,
          repr(len(entries) if isinstance(entries, list) else None))
    if not isinstance(entries, list):
        return
    REQUIRED = {"id", "format", "geometry_class", "source", "license",
                "size", "sha256", "backend"}
    ids = set()
    for e in entries:
        ids.add(e.get("id"))
        missing = REQUIRED - set(e.keys())
        check("entry fields complete: %s" % e.get("id", "?"),
              not missing, "missing " + repr(sorted(missing)))
        # sha256 must be a valid hex digest (64 chars)
        s = e.get("sha256", "")
        check("entry sha256 is 64-hex: %s" % e.get("id", "?"),
              len(s) == 64 and all(c in "0123456789abcdef" for c in s), s)
        # backend must be one of the supported transports
        check("entry backend known: %s" % e.get("id", "?"),
              e.get("backend") in ("repo", "object-store"), repr(e.get("backend")))
    check("manifest entry ids are unique",
          len(ids) == len(entries), "dups: %d ids, %d entries" % (len(ids), len(entries)))


def test_repo_backed_entries_verify_integrity():
    """Every backend=repo entry must exist relative to the corpus dir and its
    real on-disk sha256 must match the manifest (real-material integrity)."""
    mpath = os.path.join(REPO, "corpus", "manifest.json")
    if not os.path.isfile(mpath):
        check("repo entries verify (manifest missing)", False, "manifest absent")
        return
    with open(mpath, encoding="utf-8") as fh:
        data = json.load(fh)
    corpus_dir = os.path.join(REPO, "corpus")
    seen = 0
    for e in data.get("entries", []):
        if e.get("backend") != "repo":
            continue
        seen += 1
        rel = e.get("path") or e.get("id")
        fpath = os.path.join(corpus_dir, rel.lstrip("/"))
        check("repo file exists: %s" % rel, os.path.isfile(fpath), fpath)
        if os.path.isfile(fpath):
            real = sha256_of(fpath)
            check("repo sha256 matches manifest: %s" % rel,
                  real == e.get("sha256"),
                  "expected %s got %s" % (e.get("sha256"), real))
        else:
            check("repo sha256 matches manifest: %s" % rel, False, "file absent")
    check("manifest declares >=1 repo-backed entry", seen >= 1, "seen=%d" % seen)


# --- 2. Corpus harness python package ---------------------------------------
def test_registry_module_importable():
    try:
        import oep_tooling.corpus.registry as R
        check("import oep_tooling.corpus.registry",
              hasattr(R, "load_manifest") and hasattr(R, "resolve_entry"),
              "load_manifest/resolve_entry present")
    except Exception as ex:
        check("import oep_tooling.corpus.registry", False, repr(ex))


def test_store_module_importable():
    try:
        import oep_tooling.corpus.store as S
        check("import oep_tooling.corpus.store",
              hasattr(S, "fetch_entry") and hasattr(S, "object_store_url"),
              "fetch_entry/object_store_url present")
    except Exception as ex:
        check("import oep_tooling.corpus.store", False, repr(ex))


def test_cli_entrypoint():
    r = subprocess.run([sys.executable, "-m", "oep_tooling.corpus", "--help"],
                       cwd=REPO, capture_output=True, text=True,
                       env=dict(os.environ, PYTHONPATH=os.path.join(REPO, "python")))
    check("python -m oep_tooling.corpus --help exits 0",
          r.returncode == 0, (r.stdout + r.stderr)[-400:])


# --- 3. Golden-file runner --------------------------------------------------
def _run_golden(golden_spec_path):
    """Run the golden runner CLI against a spec file; return (rc, stdout, stderr)."""
    return subprocess.run(
        [sys.executable, "-m", "oep_tooling.corpus", "golden", "--spec",
         os.path.join(REPO, golden_spec_path)],
        cwd=REPO, capture_output=True, text=True,
        env=dict(os.environ, PYTHONPATH=os.path.join(REPO, "python")))


def test_golden_spec_exists():
    check_file("corpus/golden_tests.json", "golden spec exists")


def test_golden_runner_passes_on_real_material():
    """The shipped golden spec runs against the ASSERTED test=no-mismatch command
    (real fixture + real golden output) and every test should PASS (rc 0)."""
    r = _run_golden("corpus/golden_tests.json")
    check("golden runner rc 0 on shipping spec",
          r.returncode == 0, (r.stdout + r.stderr)[-600:])
    out = r.stdout or ""
    check("golden matrix shows PASS for each test",
          "STATUS" in out and out.count("PASS") >= 1, "")
    check("golden matrix does not show FAIL on clean spec",
          "FAIL" not in out.splitlines(),
          ", ".join(ln for ln in out.splitlines() if "FAIL" in ln))  # rows with FAIL


def test_golden_runner_detects_mismatch():
    """A spec whose command output differs from the golden file MUST fail the run
    (rc != 0 and a FAIL row emitted) -- proving the runner really compares."""
    spec = {
        "schema_version": 1,
        "tests": [{
            "id": "probe-mismatch",
            "corpus_id": "box.stl",
            "command": ["python3", "-c",
                        "import sys; print('triangle_count 999')", "{input}"],
            "golden": "corpus/golden/box-triangles.golden",
            "compare": "text",
            "tolerance": 0.0,
        }],
    }
    import tempfile
    spec_dir = tempfile.mkdtemp(prefix="keel-golden-", dir="/tmp")
    spec_path = os.path.join(spec_dir, "golden_tests.json")
    with open(spec_path, "w", encoding="utf-8") as fh:
        json.dump(spec, fh)
    r = subprocess.run(
        [sys.executable, "-m", "oep_tooling.corpus", "golden", "--spec", spec_path],
        cwd=REPO, capture_output=True, text=True,
        env=dict(os.environ, PYTHONPATH=os.path.join(REPO, "python")))
    out = (r.stdout or "") + (r.stderr or "")
    check("mismatching spec exits non-zero", r.returncode != 0, "rc=%d" % r.returncode)
    check("mismatching spec emits a FAIL row", "FAIL" in out, out[-500:])


def test_golden_runner_classified_skip_is_tolerant():
    """A test referencing a corpus entry that does not exist must be classified as
    SKIP (a warning), must NOT crash the runner, and must NOT fail the run."""
    spec = {
        "schema_version": 1,
        "tests": [{
            "id": "probe-skip",
            "corpus_id": "nope-nothing.cgr",
            "command": ["python3", "-c", "print(1)", "{input}"],
            "golden": "corpus/golden/box-triangles.golden",
            "compare": "text",
            "tolerance": 0.0,
        }],
    }
    import tempfile
    spec_dir = tempfile.mkdtemp(prefix="keel-golden-", dir="/tmp")
    spec_path = os.path.join(spec_dir, "golden_tests.json")
    with open(spec_path, "w", encoding="utf-8") as fh:
        json.dump(spec, fh)
    r = subprocess.run(
        [sys.executable, "-m", "oep_tooling.corpus", "golden", "--spec", spec_path],
        cwd=REPO, capture_output=True, text=True,
        env=dict(os.environ, PYTHONPATH=os.path.join(REPO, "python")))
    out = (r.stdout or "") + (r.stderr or "")
    check("skip test rc 0", r.returncode == 0, "rc=%d" % r.returncode)
    check("skip test classified SKIP", "SKIP" in out, out[-500:])


# --- 4. Object-store backend (NAS Gitea package registry) -------------------
def test_manifest_object_store_entries_wellformed():
    """object-store entries must declare their NAS package pointer so a later
    sync (P1-M1-T05) can materialize them; declare >=1 to prove schema support."""
    mpath = os.path.join(REPO, "corpus", "manifest.json")
    if not os.path.isfile(mpath):
        check("object-store entries wellformed (manifest absent)", False)
        return
    with open(mpath, encoding="utf-8") as fh:
        data = json.load(fh)
    obj = [e for e in data.get("entries", []) if e.get("backend") == "object-store"]
    check("manifest declares >=1 object-store entry (schema exercised)",
          len(obj) >= 1, "count=%d" % len(obj))
    for e in obj:
        o = e.get("object_store") or {}
        check("object-store ptr fields: %s" % e.get("id"),
              {"owner", "package", "version", "object"} <= set(o.keys()),
              repr(o))


def main():
    print("Keelworks test corpus harness acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_manifest_exists_and_schema()
    test_repo_backed_entries_verify_integrity()
    test_registry_module_importable()
    test_store_module_importable()
    test_cli_entrypoint()
    test_golden_spec_exists()
    test_golden_runner_passes_on_real_material()
    test_golden_runner_detects_mismatch()
    test_golden_runner_classified_skip_is_tolerant()
    test_manifest_object_store_entries_wellformed()
    print("=" * 60)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
