"""P7-M3-T02 acceptance: mates/kinematics demo (own DMU-like semantics).

Independent (stdlib-only) verification of the recorded demo produced by the
`oep-kinematics-demo` binary (crank-slider fixture):

  - the recorded artifacts exist: trace.json, manifest.json, GIF/MP4, PPM frames
  - the trace numerics are physically correct and tight:
      piston stroke == 2 * CRANK_R == 60 mm (x range over the 90-frame turn),
      piston stays on the prismatic groove line (y/z constant),
      the crank pin-to-piston-distance stays == ROD_L == 70 mm (closed loop),
      solver closed-loop residual and analytic error are machine-tight
  - the mechanism has exactly 1 free DOF when the crank is not driven
  - the clean-room invariant holds: oep-kinematics declares only oep-geo and
    contains no vendor references / prebuilt artifacts / vendored trees
  - real-material reuse: the demo embeds the oep-geo corpus golden (bracket)
    byte-identical to crates/oep-geo/tests/golden/example-bracket.oepgeo.json

Written after the Rust gates (this file is the independent second voice).
"""
import hashlib
import json
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = os.path.join(REPO, "crates", "oep-kinematics")
OUT = os.path.join(REPO, "demos", "p7-m3-t02")
TRACE = os.path.join(OUT, "trace.json")
MANIFEST = os.path.join(OUT, "manifest.json")
FRAMES = os.path.join(OUT, "frames")

CRANK_R = 30.0
ROD_L = 70.0
EXPECTED_FRAMES = 90

FORBIDDEN_TOKENS = [
    "opencascade", "occ", "cascade", "occt", "freecad", "cadcraft",
    "cgal", "nalgebra", "parry", "kiss3d", "cgmath", "ultraviolet", "vek", "glam",
]
FORBIDDEN_EXT = {".dll", ".so", ".dylib", ".lib", ".obj", ".a", ".o", ".pdb", ".rlib"}


def _run_demo():
    """Build + run the demo binary into OUT (idempotent, ~seconds on repeat)."""
    if (os.path.isfile(TRACE) and os.path.isdir(FRAMES)
            and len(os.listdir(FRAMES)) >= EXPECTED_FRAMES):
        return  # already recorded (frames are gitignored, so CI must re-run)
    os.makedirs(FRAMES, exist_ok=True)
    env = dict(os.environ)
    r = subprocess.run(
        ["cargo", "run", "--quiet", "-p", "oep-kinematics",
         "--bin", "oep-kinematics-demo", "--", OUT],
        cwd=REPO, env=env, capture_output=True, text=True, timeout=1800,
    )
    assert r.returncode == 0, f"demo failed:\n{r.stdout}\n{r.stderr}"


def _load_trace():
    with open(TRACE, "r", encoding="utf-8") as fh:
        return json.load(fh)


def _xor_sum_sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        h.update(fh.read())
    return h.hexdigest()


def test_recorded_artifacts_exist():
    _run_demo()
    for p in [TRACE, MANIFEST, os.path.join(OUT, "crank-slider-demo.gif"),
              os.path.join(OUT, "crank-slider-demo.mp4")]:
        assert os.path.isfile(p), f"missing recorded artifact {p}"
    frames = sorted(f for f in os.listdir(FRAMES) if f.endswith(".ppm"))
    assert len(frames) == EXPECTED_FRAMES, f"expected {EXPECTED_FRAMES} PPM frames, got {len(frames)}"
    # PPM header sanity
    with open(os.path.join(FRAMES, frames[0]), "rb") as fh:
        assert fh.read(15) == b"P6\n640 480\n255\n", "PPM P6 header"


def test_trace_numerics_are_physically_correct():
    _run_demo()
    d = _load_trace()
    assert d["program"] == "P7-M3-T02"
    assert d["fixture"] == "crank-slider"
    frames = d["frames"]
    assert len(frames) == EXPECTED_FRAMES
    x_pos = []
    for f in frames:
        piston = f["bodies"]["inst-piston"]
        x_pos.append(piston[12])          # column-major translation x
        assert abs(piston[13] - 50.0) < 1e-6, "piston y must stay on the groove"
        assert abs(piston[14] - 8.0) < 1e-6, "piston z must stay on the groove"
    stroke = max(x_pos) - min(x_pos)
    assert abs(stroke - 2 * CRANK_R) < 1e-3, f"stroke {stroke} != 60"
    assert d["free_dof_undriven"] == 1, "crank-slider is a 1-DOF mechanism"
    assert float(d["worst_residual_max"]) < 1e-6, "closed-loop residual must be tight"
    assert float(d["worst_translation_error"]) < 1e-4, "analytic match must be mm-tight"
    assert float(d["worst_orientation_error"]) < 1e-6


def test_closed_loop_rod_length_is_preserved():
    _run_demo()
    d = _load_trace()
    for f in d["frames"]:
        crank = f["bodies"]["inst-crank"]
        piston = f["bodies"]["inst-piston"]
        # world crank pin: crank pose origin + R * (rotated local +X col)
        pin = (crank[12] + CRANK_R * crank[0],
               crank[13] + CRANK_R * crank[4],
               crank[14] + CRANK_R * crank[8])
        crank_pivot = (crank[12], crank[13], crank[14])
        piston_origin = (piston[12], piston[13], piston[14])
        # the crank pin rides a circle of radius R around the fixed pivot
        dist_pin_pivot = sum((a - b) ** 2 for a, b in zip(pin, crank_pivot)) ** 0.5
        assert abs(dist_pin_pivot - CRANK_R) < 1e-3, f"pin radius {dist_pin_pivot}"
        # the closed loop closes: piston origin stays ROD_L from the crank pin
        dist_loop = sum((a - b) ** 2 for a, b in zip(piston_origin, pin)) ** 0.5
        assert abs(dist_loop - ROD_L) < 1e-3, f"loop length {dist_loop}"


def test_cleanroom_invariant_holds():
    # 1) runtime deps exactly oep-geo
    toml = open(os.path.join(CRATE, "Cargo.toml"), encoding="utf-8").read()
    sec = toml.split("[dependencies]")[1].split("\n[")[0]
    keys = [ln.split("=")[0].strip() for ln in sec.splitlines()
            if "=" in ln and not ln.strip().startswith("#")]
    assert keys == ["oep-geo"], f"runtime deps must be exactly [oep-geo], got {keys}"
    # 2) no vendor references in crate source code
    for root, _dirs, files in os.walk(os.path.join(CRATE, "src")):
        for name in files:
            if not name.endswith(".rs"):
                continue
            for i, line in enumerate(open(os.path.join(root, name), encoding="utf-8"), 1):
                code = line.split("//")[0].lower()
                for tok in FORBIDDEN_TOKENS:
                    assert tok not in code, f"{name}:{i} references {tok}"
    # 3) no prebuilt artifacts / vendored trees in the crate
    for root, dirs, files in os.walk(CRATE):
        rel = os.path.relpath(root, CRATE)
        if rel.split(os.sep)[0] == "target":
            continue
        for d in dirs:
            assert d.lower() not in {"vendor", "third_party", "third-party", "3rdparty"}, rel
        for name in files:
            _, ext = os.path.splitext(name)
            if ext.lower() in FORBIDDEN_EXT:
                raise AssertionError(f"prebuilt artifact in crate: {os.path.join(rel, name)}")


def test_real_material_golden_reuse_is_byte_identical():
    _run_demo()
    ours = os.path.join(CRATE, "tests", "golden", "example-bracket.oepgeo.json")
    real = os.path.join(REPO, "crates", "oep-geo", "tests", "golden",
                        "example-bracket.oepgeo.json")
    assert _xor_sum_sha(ours) == _xor_sum_sha(real), "golden copies drifted apart"


if __name__ == "__main__":
    for name, fn in sorted({k: v for k, v in list(globals().items())
                            if k.startswith("test_")}.items()):
        try:
            fn()
            print("[PASS] %s" % name)
        except AssertionError as e:
            print("[FAIL] %s -- %s" % (name, e))
            sys.exit(1)
    print("ALL P7-M3-T02 ACCEPTANCE CHECKS PASSED")
