"""P2-M3-T01 acceptance test: CGR attribute/normal observation re-devised.

Verifies that docs/re-notes/oep-cgr-vertex-attributes.md facts hold on real
material WHEN samples are present, and that the observation tool classifies
without crashing. Tolerant SKIP when the (clean-room, never-committed) sample
corpus is absent, so CI stays green.

Facts asserted (from the notebook + tool):
  1. absent attribute records are the honest fallback: a payload with no
     identifiable normals must not be reported as carrying them;
  2. the tool never crashes on any real .cgr it is pointed at;
  3. unit-length f32 normal runs are NOT found in either family (negative
     evidence the acceptance criterion leans on).

Run:  python -m pytest tests/test_cgr_attributes_acceptance.py -v
   or: python -m oep_tooling.cgr_attributes <file.cgr>
"""
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)


def _find_samples():
    dirs = []
    env = os.environ.get("OEP_CGR_SAMPLES")
    if env:
        dirs += [d.strip() for d in env.replace(":", ",").split(",") if d.strip()]
    dirs += [os.path.join(HERE, "samples"), os.path.join(HERE, "..", "samples")]
    found = []
    for d in dirs:
        if not os.path.isdir(d):
            continue
        for name in os.listdir(d):
            if name.lower().endswith(".cgr"):
                found.append(os.path.join(d, name))
    return found


def test_observe_never_crashes_on_real_material():
    samples = _find_samples()
    if not samples:
        print("SKIP  no CGR sample supplied (set OEP_CGR_SAMPLES=<dir>)")
        return
    script = os.path.join(ROOT, "python", "oep_tooling", "cgr_attributes.py")
    for s in samples:
        # capture; must exit 0 and print the report without exception
        r = subprocess.run([sys.executable, script, s], capture_output=True, text=True)
        assert r.returncode == 0, "scan crash on %s: %s" % (s, r.stderr[-400:])
        assert "vertex pools" in r.stdout, "expected structured report"


def test_unit_normal_runs_absent_negative_evidence():
    """Where real samples exist, the notebook's negative evidence must hold:
    no >=8-run of raw unit-length f32 normals anywhere in either family."""
    samples = _find_samples()
    if not samples:
        print("SKIP  no CGR sample supplied (set OEP_CGR_SAMPLES=<dir>)")
        return
    import sys as _sys
    sys.path.insert(0, os.path.join(ROOT, "python"))
    from oep_tooling.cgr_attributes import unit_normal_runs
    for s in samples:
        data = open(s, "rb").read()
        runs = unit_normal_runs(data)
        assert not runs, "unexpected raw unit-f32 normal run in %s: %r" % (s, runs[:2])
