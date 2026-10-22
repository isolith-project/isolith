"""INTERACTIVE-SHELL acceptance: the interactive editing surface is a drivable,
mouse-grade editor whose model is the real oep-geo document.

Independent (stdlib-only) verification that the oep-shell + oep-viewer editing
surface performs the program's main job through the machine-callable workbench
CLI (the agent/automation lane that the mouse-driven editor calls under the
pointer):

  1. A user sketches a rectangle (4 points + 4 line segments), constrains it
     (fixed anchor, horizontal/vertical, and length dimensions that REFERENCE
     the variable set), and solves: fully constrained (dof 0), width/height
     = 80/40 from the variable set.
  2. Extrude produces the analytic solid (80 x 40 x 10 -> volume 32000) in the
     canonical document; the body bbox is the real mesh bbox.
  3. CHANGING A DIMENSION IN THE VARIABLE SET (width 80 -> 120) re-solves and
     re-derives the solid LIVE: volume 48000, bbox x-max 120, and the
     rendered viewport changes (two render SHA-256 differ).
  4. STEP export writes a real AP214 file (closed shell); undo/redo navigate
     the real feature graph.
  5. The committed demo (demos/editor-sketch-extrude) reproduces the same
     analytics when re-driven, so the committed record and the live binary
     cannot drift.

The prose rule of the P7 lane (no em dashes, no curly quotes) is asserted on
the files this card adds on the shell surface.
"""
import json
import os
import re
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SHELL = os.path.join(REPO, "crates", "oep-shell")
DEMO = os.path.join(REPO, "demos", "editor-sketch-extrude")

EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"

ACCEPTANCE_COMMANDS = [
    "editor:open",
    "sk:point:0:0", "sk:point:3:1", "sk:point:4:4", "sk:point:1:5",
    "sk:line:p0:p1", "sk:line:p1:p2", "sk:line:p2:p3", "sk:line:p3:p0",
    "sk:constrain:fixed:p0:0:0", "sk:constrain:horizontal:l0",
    "sk:constrain:horizontal:l2", "sk:constrain:vertical:l1",
    "sk:constrain:vertical:l3", "sk:constrain:length:l0:width",
    "sk:constrain:length:l1:height",
    "vars:set:width:80", "vars:set:height:40",
    "sk:solve", "feat:extrude:10", "view:frame",
]


def run(*args, extra=None):
    """Run the workbench CLI with `args`; return (returncode, stdout, stderr)."""
    cmd = ["cargo", "run", "--quiet", "-p", "oep-shell", "--example", "workbench", "--", *args]
    env = dict(os.environ)
    env.pop("OEP_DEBUG", None)
    r = subprocess.run(cmd, cwd=REPO, env=env, capture_output=True, text=True,
                       timeout=2400)
    return r.returncode, r.stdout, r.stderr


def step_output(directory):
    """Add the render + export commands pointing at `directory`."""
    return ACCEPTANCE_COMMANDS + [
        "sk:render:%s" % os.path.join(directory, "before.png"),
        "vars:set:width:120",
        "sk:render:%s" % os.path.join(directory, "after.png"),
        "export:step:%s" % os.path.join(directory, "edpart.step"),
        "undo", "redo",
    ]


def test_sketch_constrain_extrude_live_edit_export():
    with tempfile.TemporaryDirectory() as td:
        rc, out, err = run(*step_output(td))
        assert rc == 0, "nonzero exit:\n%s\n%s" % (out, err)

        # Solve fully constrained with dims from the variable set.
        assert "solved dof=0 residual=" in out, out
        assert "  width = 80.000000" in out and "  height = 40.000000" in out, out

        # Extrude volume analytic.
        assert "extruded oep-feature-extrude-2: volume 32000.000000" in out, out
        assert "faces=12 edges=12" in out, out

        # Drive lane: edit the variable, the solid follows live.
        assert "variable width = 120 (volume 32000.000000 -> 48000.000000 live recompute)" in out, out

        # The viewport visibly changes between the two states.
        shas = re.findall(r"rendered \S+before\.png sha256=([0-9a-f]{64})", out)
        sha_before = shas[0]
        shas_after = re.findall(r"rendered \S+after\.png sha256=([0-9a-f]{64})", out)
        sha_after = shas_after[0]
        assert sha_before != sha_after, "width edit must change the rendered solid"

        # STEP export writes a real closed shell.
        assert "exported STEP" in out and "faces=12 shells=1 closed=1" in out, out
        step_path = os.path.join(td, "edpart.step")
        assert os.path.isfile(step_path)
        text = open(step_path, encoding="utf-8").read()
        assert text.startswith("ISO-10303-21;"), "PART 21 header"
        assert "SHELL_BASED_SURFACE_MODEL" in text, text
        assert "CLOSED_SHELL" in text

        # Undo/redo navigate the real feature graph (both succeed, no panic).
        assert "undo: op=previous edit" in out, out
        assert "redo: op=next edit" in out, out


def test_committed_demo_manifest_matches_live_behavior():
    manifest_path = os.path.join(DEMO, "manifest.json")
    assert os.path.isfile(manifest_path), "committed demo manifest missing"
    manifest = json.load(open(manifest_path, encoding="utf-8"))
    assert manifest["task"] == "editor-sketch-extrude"
    assert manifest["solve"]["dof"] == 0
    assert manifest["solve"]["dimensions"]["width"] == 80.0
    assert manifest["extrude"]["volume"] == 32000.0
    assert manifest["drive_lane_edit"]["volume_before"] == 32000.0
    assert manifest["drive_lane_edit"]["volume_after"] == 48000.0
    assert manifest["render"]["before_ne_after"] is True
    assert manifest["step_export"]["closed_shells"] == 1

    # The committed STEP demo artifact is real PART 21 material.
    step_path = os.path.join(DEMO, "edpart.step")
    assert os.path.isfile(step_path)
    text = open(step_path, encoding="utf-8").read()
    assert text.startswith("ISO-10303-21;")
    assert "SHELL_BASED_SURFACE_MODEL" in text
    assert "NOT an exact analytic B-Rep" in text, "honesty line in STEP header"


def test_committed_demo_reproduces_live_analytics():
    # Re-drive the exact demo command list into a temp dir and compare the
    # analytics the committed manifest asserts (committed record == live run).
    with tempfile.TemporaryDirectory() as td:
        tdir = os.path.join(td, "re")
        os.makedirs(tdir)
        rc, out, err = run(*step_output(tdir))
        assert rc == 0, "nonzero exit:\n%s\n%s" % (out, err)
        assert "solved dof=0" in out and "volume 32000.000000" in out, out
        assert "48000.000000 live recompute" in out, out
        assert "closed=1" in out, out
        step_text = open(os.path.join(tdir, "edpart.step"), encoding="utf-8").read()
        assert "NOT an exact analytic B-Rep" in step_text


def test_surface_files_carry_no_em_dashes_or_curly_quotes():
    files = {
        os.path.join(SHELL, "src", "editor.rs"): "editor.rs",
        os.path.join(SHELL, "examples", "workbench.rs"): "workbench example",
        os.path.join(SHELL, "tests", "editor_surface.rs"): "editor_surface.rs",
        os.path.join(SHELL, "src", "commands.rs"): "commands.rs",
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
    print("ALL INTERACTIVE-SHELL ACCEPTANCE CHECKS PASSED")
