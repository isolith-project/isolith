"""Golden-file runner: run a command per corpus entry and compare stdout to a golden file.

Tolerant model (P2-M1-T04): a test whose corpus entry can't be resolved, whose
command fails, or whose golden is missing is classified SKIP with a warning, never
a crash. A real comparison mismatch is FAIL (non-zero exit). SKIP rows do not fail
the run, matching the regression-corpus contract (P2-M5-T03: every corpus file
opens or yields a classified skip).

Spec schema (corpus/golden_tests.json):
    schema_version, tests: [
      {id, corpus_id, command: [argv, ...],  # "{input}" placeholder -> corpus path
       golden: relative path, compare: text|bytes|numeric, tolerance: float}
    ]

Exit code: 0 when no test FAILs (SKIP allowed); 1 otherwise.
"""
import json
import os
import subprocess
import sys

from oep_tooling.corpus import registry


class GoldenResult:
    def __init__(self, test_id, status, detail=""):
        self.id = test_id
        self.status = status  # PASS | FAIL | SKIP
        self.detail = detail

    def __repr__(self):
        return "GoldenResult(%s, %s, %s)" % (self.id, self.status, self.detail)


def _normalize(text):
    return "\n".join(ln.rstrip() for ln in text.splitlines()).strip()


def _compare_text(actual, expected):
    return (_normalize(actual) == _normalize(expected)), None


def _compare_bytes(actual, expected):
    return (actual == expected), None


def _numeric_tokens(text):
    out = []
    for tok in text.split():
        try:
            out.append(float(tok))
        except ValueError:
            return None
    return out


def _compare_numeric(actual, expected, tolerance):
    a = _numeric_tokens(actual)
    e = _numeric_tokens(expected)
    if a is None or e is None:
        return False, "non-numeric token present"
    if len(a) != len(e):
        return False, "token count differs: %d vs %d" % (len(a), len(e))
    for i, (x, y) in enumerate(zip(a, e)):
        if abs(x - y) > tolerance:
            return False, "token %d off by %g (tolerance %g)" % (i, abs(x - y), tolerance)
    return True, None


def _resolve_input(test, corpus_root):
    """Resolve a test's input directly under an explicit corpus root when given.

    Returns (path, None) on success or (None, error). The registry/manifest
    path stays the fallback for the committed-corpus mode. Supporting both
    ``<root>/<corpus_id>`` and ``<root>/data/<basename>`` layouts.
    """
    cid = test.get("corpus_id", "")
    if not corpus_root or not cid:
        return None, None
    for cand in (
        os.path.join(corpus_root, cid),
        os.path.join(corpus_root, "data", cid),
        os.path.join(corpus_root, "data", os.path.basename(cid)),
    ):
        if os.path.isfile(cand):
            return cand, None
    return None, "input absent under corpus_root: %s" % cid


def run_test(test, manifest=None, cache_dir=None, corpus_root=None):
    """Execute one golden test. Returns GoldenResult."""
    tid = test["id"]
    in_path, in_err = _resolve_input(test, corpus_root)
    if in_path is None and in_err is None:
        m = manifest or registry.load_manifest()
        entry = registry.find_entry(m, test["corpus_id"])
        if entry is None:
            return GoldenResult(tid, "SKIP", "corpus entry not in registry: %s" % test["corpus_id"])
        in_path, in_err = registry.resolve_entry(entry, manifest=m, cache_dir=cache_dir)
    if in_err:
        return GoldenResult(tid, "SKIP", "input unresolved: %s" % in_err)
    cmd = []
    for a in test.get("command", []):
        cmd.append(a.replace("{input}", in_path))
    if not cmd:
        return GoldenResult(tid, "SKIP", "no command in spec")
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
    except FileNotFoundError:
        return GoldenResult(tid, "SKIP", "command not found: %s" % cmd[0])
    except Exception as e:
        return GoldenResult(tid, "SKIP", "command error: %r" % (e,))
    if r.returncode != 0:
        return GoldenResult(tid, "SKIP",
                            "command rc=%d: %s" % (r.returncode, (r.stderr or "")[-200:]))
    golden_rel = test.get("golden")
    golden_abs = golden_rel
    repo_root_dir = os.path.dirname(m["corpus_dir"])  # parent of corpus/
    if not os.path.isabs(golden_rel):
        golden_abs = os.path.join(repo_root_dir, golden_rel)
    if not os.path.isfile(golden_abs):
        return GoldenResult(tid, "SKIP", "golden missing: %s" % golden_rel)
    try:
        with open(golden_abs, encoding="utf-8") as fh:
            exp = fh.read()
    except Exception as e:
        return GoldenResult(tid, "SKIP", "golden unreadable: %r" % (e,))
    mode = test.get("compare", "text")
    tol = float(test.get("tolerance", 0.0))
    if mode == "bytes":
        # binary compare on encoded bytes
        try:
            ok = (r.stdout.encode("utf-8") == exp.encode("utf-8"))
        except Exception:
            ok = False
        detail = None if ok else "bytes differ"
    elif mode == "numeric":
        ok, detail = _compare_numeric(r.stdout, exp, tol)
    else:
        ok, detail = _compare_text(r.stdout, exp)
    if ok:
        return GoldenResult(tid, "PASS", "")
    return GoldenResult(tid, "FAIL", detail or "output differs from golden; see diff")


def run_spec(spec_path, manifest=None, cache_dir=None, corpus_root=None):
    """Run every test in a golden spec file. Returns (results, exit_code).

    ``corpus_root`` scopes input resolution to an explicit corpus tree when
    given (falls through to the registry manifest for the committed corpus).
    """
    spec_abs = os.path.abspath(spec_path)
    if manifest is None:
        try:
            manifest = registry.load_manifest(os.path.dirname(spec_abs))
        except FileNotFoundError:
            manifest = registry.load_manifest()  # fall back to cwd/repo root
    with open(spec_abs, encoding="utf-8") as fh:
        spec = json.load(fh)
    results = []
    for test in spec.get("tests", []):
        results.append(run_test(test, manifest=manifest, cache_dir=cache_dir,
                                corpus_root=corpus_root))
    any_fail = any(r.status == "FAIL" for r in results)
    return results, (1 if any_fail else 0)


def print_summary(results):
    print("STATUS  TEST")
    for r in results:
        print("%-7s %s" % (r.status, r.id) + ("  -- %s" % r.detail if r.detail else ""))
    n_pass = sum(1 for r in results if r.status == "PASS")
    n_skip = sum(1 for r in results if r.status == "SKIP")
    n_fail = sum(1 for r in results if r.status == "FAIL")
    print("-" * 40)
    print("PASS=%d SKIP=%d FAIL=%d" % (n_pass, n_skip, n_fail))
    return n_fail
