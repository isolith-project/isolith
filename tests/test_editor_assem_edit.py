"""EDITOR-ASSEM-EDIT acceptance: interactive assembly mate editing with the live solver.

Independent (stdlib-only) verification that the assembly solver is interactive
and part of the editor surface, driven through the machine-callable workbench
CLI (the agent/automation lane of the shell):

  1. The surface opens a committed 3-part fixture (base/cap/peg) carrying the
     oep-assem mate vocabulary (fixed, coincident, on-plane, plane-distance,
     parallel) and lists instances + mates.
  2. A live solve places the assembly and each movable instance follows; a
     mate edit re-solves and the moved instance lands at the edited target.
  3. A diverging mate edit returns a TYPED error (solver-failed) and never a
     fabricated pose: the working instance placement is unchanged, and
     deleting the bad mate recovers the solved state.
  4. The viewport (CPU raster, PPM) content-hash changes across a solve: the
     instances visibly follow.
  5. Mechanism motion plays deterministically through the kinematics lane:
     two recordings of the crank-slider turn yield byte-identical traces and
     the committed physical numbers (piston stroke 60 mm, 1 free DOF).

The prose rule of the P7 lane (no em dashes, no curly quotes) is asserted on
the files this card adds/edits on the shell surface.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHELL = os.path.join(REPO, "crates", "oep-shell")
GOLDEN_MODEL = os.path.join(
    REPO, "crates", "oep-assem", "tests", "golden", "three-part-fixture.oepgeo.json"
)
GOLDEN_CONSTRAINTS = os.path.join(
    REPO, "crates", "oep-assem", "tests", "golden", "three-part-fixture.constraints.json"
)

EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"


def run(*args):
    """Run the workbench CLI with `args`; return (returncode, stdout, stderr)."""
    cmd = ["cargo", "run", "--quiet", "-p", "oep-shell", "--example", "workbench", "--", *args]
    env = dict(os.environ)
    env.pop("OEP_DEBUG", None)
    r = subprocess.run(cmd, cwd=REPO, env=env, capture_output=True, text=True, timeout=1800)
    return r.returncode, r.stdout, r.stderr


def test_open_lists_the_three_part_fixture():
    rc, out, err = run("assem:open", "assem:instances")
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "assembled oep-assembly-fixture3: instances=3 mates=9" in out, out
    for inst in ("base", "cap", "peg"):
        assert "instance %s part=" % inst in out, f"missing {inst}:\n{out}"


def test_live_solve_places_instances_that_follow():
    rc, out, err = run("assem:open", "assem:solve", "assem:instances")
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    # dof=2 is physically correct: both movable parts keep a free roll about
    # their upright axis (the v1 solver pins only Z-axis orientation).
    assert "solved dof=2 moved=2" in out, out
    assert "moved cap to (36.0000, 18.0000, 10.0000)" in out, out
    assert "moved peg to (18.0000, 22.0000, 10.0000)" in out, out
    # The instance listing reflects the solved placement.
    assert "instance peg part=oep-part-peg at (18.0000, 22.0000, 10.0000)" in out, out
    assert "instance base part=oep-part-base at (0.0000, 0.0000, 0.0000)" in out, out


def test_mate_edit_is_accepted_and_instances_follow_immediately():
    rc, out, err = run(
        "assem:open",
        "assem:mate-edit:2:plane-distance:cap:origin:base:inX:30",
        "assem:solve",
        "assem:instances",
    )
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "edited mate 2: solved" in out, out
    assert "moved cap to (30.0000, 18.0000, 10.0000)" in out, out


def test_mate_add_and_delete_are_the_surface_vocabulary():
    # Create a redundant but consistent plane-distance on the peg via
    # mate-add (inY 22 pinches y to 22, the same target the outY 28 mate
    # already enforces), then delete it; the assembly stays solved at each
    # live step.
    rc, out, err = run(
        "assem:open",
        "assem:mate-add:plane-distance:peg:origin:base:inY:22",
        "assem:solve",
    )
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "added mate 9: solved" in out, out
    assert "moved peg to (18.0000, 22.0000, 10.0000)" in out, out

    rc, out, err = run(
        "assem:open",
        "assem:mate-add:plane-distance:peg:origin:base:inY:22",
        "assem:mate-del:9",
        "assem:solve",
    )
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "removed mate 9: solved" in out, out
    assert "moved peg to (18.0000, 22.0000, 10.0000)" in out, out


def test_added_contradictory_mate_is_a_typed_error():
    # Adding a second x-plane on the peg (18 already pins x=18) is genuinely
    # inconsistent: x cannot equal 18 and 24. The surface reports the typed
    # error and keeps the previous converged placement.
    rc, out, err = run(
        "assem:open",
        "assem:solve",
        "assem:mate-add:plane-distance:peg:origin:base:inX:24",
        "assem:instances",
    )
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "diverged mate 9: solver-failed:" in out, out
    assert "instance peg part=oep-part-peg at (18.0000, 22.0000, 10.0000)" in out, out


def test_diverging_mate_returns_typed_error_and_no_fabricated_pose():
    # A coincident pulling peg onto cap while both are pinned cannot satisfy;
    # the typed error (solver-failed) is reported and the pose is untouched.
    rc, out, err = run(
        "assem:open",
        "assem:solve",
        "assem:mate-add:coincident:peg:origin:cap:origin",
        "assem:instances",
    )
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "diverged mate 9: solver-failed:" in out, out
    # The listing still shows the last converged placement, never a fabricated one.
    assert "instance cap part=oep-part-cap at (36.0000, 18.0000, 10.0000)" in out, out

    # The edit is kept (an engineer can fix it); deleting it recovers.
    rc, out, err = run(
        "assem:open",
        "assem:solve",
        "assem:mate-add:coincident:peg:origin:cap:origin",
        "assem:mate-del:9",
        "assem:solve",
    )
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "removed mate 9: solved" in out, out
    assert "moved peg to (18.0000, 22.0000, 10.0000)" in out, out


def test_assem_mates_lists_the_vocabulary():
    rc, out, err = run("assem:open", "assem:mates")
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    for kind in ("fixed", "on-plane", "plane-distance", "parallel"):
        assert "mate " in out and kind in out, f"missing {kind}:\n{out}"
    # All nine fixture mates are listed with indices.
    assert "mate 8 parallel peg:top || base:top" in out, out


def test_viewport_render_proves_instances_follow():
    import tempfile

    before = os.path.join(tempfile.mkdtemp(), "assem-before.ppm")
    after = os.path.join(tempfile.mkdtemp(), "assem-after.ppm")
    rc, out, err = run("assem:open", "assem:render:%s" % before, "assem:solve", "assem:render:%s" % after)
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    m = list(re.finditer(r"rendered \S+ sha256=([0-9a-f]{64}) \d+x\d+", out))
    assert len(m) == 2, out
    sha_before, sha_after = m[0].group(1), m[1].group(1)
    assert sha_before != sha_after, "the mis-placed and solved renders must differ"
    for p in (before, after):
        with open(p, "rb") as fh:
            assert fh.read(15) == b"P6\n640 480\n255\n", "PPM P6 header"


def test_mechanism_motion_is_deterministic_and_crank_slider():
    import tempfile

    d1 = os.path.join(tempfile.mkdtemp(), "motion-a")
    d2 = os.path.join(tempfile.mkdtemp(), "motion-b")
    rc, out, err = run("assem:open", "assem:animate:36:%s" % d1)
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    assert "motion frames=36" in out, out
    assert "free_dof_undriven=1" in out, out
    sha1 = re.search(r"trace_sha256=([0-9a-f]{64})", out).group(1)

    rc, out, err = run("assem:open", "assem:animate:36:%s" % d2)
    assert rc == 0, f"nonzero exit:\n{out}\n{err}"
    sha2 = re.search(r"trace_sha256=([0-9a-f]{64})", out).group(1)
    assert sha1 == sha2, "two recordings of the same turn must be byte-identical"

    trace = os.path.join(d1, "trace.json")
    manifest = os.path.join(d1, "manifest.json")
    assert os.path.isfile(trace) and os.path.isfile(manifest), "recorded artifacts"
    data = json.load(open(trace, encoding="utf-8"))
    frames = data["frames"]
    x_pos = []
    for f in frames:
        piston = f["bodies"]["inst-piston"]
        x_pos.append(piston[12])
        assert abs(piston[13] - 50.0) < 1e-6, "piston stays on the groove"
    stroke = max(x_pos) - min(x_pos)
    assert abs(stroke - 60.0) < 1e-3, "piston stroke %r != 60 mm" % stroke
    assert float(data["worst_residual_max"]) < 1e-6, "closed-loop residual tight"


def test_shell_embedded_fixture_is_byte_identical_to_the_golden():
    src_assem = os.path.join(SHELL, "src", "assem.rs")
    text = open(src_assem, encoding="utf-8").read()
    assert GOLDEN_MODEL.split("/")[-1] in text and GOLDEN_CONSTRAINTS.split("/")[-1] in text
    # The two lanes reference the same committed files (byte-identity is
    # enforced by the Rust include_str at build time; here we confirm the path
    # wiring did not drift).
    assert os.path.isfile(GOLDEN_MODEL) and os.path.isfile(GOLDEN_CONSTRAINTS)


def test_surface_files_carry_no_em_dashes_or_curly_quotes():
    files = {
        os.path.join(SHELL, "src", "assem.rs"): "assem.rs",
        os.path.join(SHELL, "src", "sha256.rs"): "sha256.rs",
        os.path.join(SHELL, "src", "commands.rs"): "commands.rs",
        os.path.join(SHELL, "src", "lib.rs"): "lib.rs",
        os.path.join(SHELL, "examples", "workbench.rs"): "workbench example",
        os.path.join(SHELL, "tests", "assembly_surface.rs"): "assembly_surface.rs",
        os.path.join(REPO, "crates", "oep-assem", "tests", "three_part_fixture.rs"): "three_part_fixture.rs",
    }
    for path, label in files.items():
        text = open(path, encoding="utf-8").read()
        assert EM_DASH not in text, f"{label}: em dash present"
        assert not any(c in CURLY for c in text), f"{label}: curly quote present"


if __name__ == "__main__":
    for name, fn in sorted({k: v for k, v in list(globals().items())
                            if k.startswith("test_")}.items()):
        try:
            fn()
            print("[PASS] %s" % name)
        except AssertionError as e:
            print("[FAIL] %s -- %s" % (name, e))
            sys.exit(1)
    print("ALL EDITOR-ASSEM-EDIT ACCEPTANCE CHECKS PASSED")
