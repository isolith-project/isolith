"""P11-T05 acceptance: the export-control checkbox in the release pipeline.

The release drill, driven on the real gate (python/oep_tooling/
export_control_gate.py) through the CI/bash wrapper:

PASS direction (file present):
  with P11/export-control/<V>.classification.toml committed for the drill
  release, `verify --release <V>` must exit 0 -- the release is cleared.

FAIL direction (file removed):
  with the classification file removed from the per-release tree,
  `verify --release <V>` must exit non-zero with a "release blocked" message.
  This is the literal "absence fails the release" acceptance.

Also pins the policy doc, the template, the committed per-release instance,
the CI workflow that makes the release branch require the file, and the
recorded proof. Pure stdlib; runs under pytest and standalone.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DOC = REPO_ROOT / "P11" / "export-control.md"
TPL = REPO_ROOT / "P11" / "export-control" / "classification.template.toml"
REL_DIR = REPO_ROOT / "P11" / "export-control"
GATE_MOD = REPO_ROOT / "python" / "oep_tooling" / "export_control_gate.py"
WRAPPER = REPO_ROOT / "scripts" / "export-control" / "verify-classification.sh"
WORKFLOW = REPO_ROOT / ".gitea" / "workflows" / "ga-gate-p11-export-control.yaml"
PROOF_DIR = REPO_ROOT / "P11" / "proof"

DRILL_RELEASE = "2027.1.0-rc1"
DRILL_FILE = REL_DIR / f"{DRILL_RELEASE}.classification.toml"


def _run(*args: str | os.PathLike[str], cwd: Path = REPO_ROOT) -> subprocess.CompletedProcess:
    return subprocess.run(
        [str(a) for a in args], cwd=str(cwd), capture_output=True, text=True, timeout=180
    )


# --- the drill: PASS with the file present, FAIL with it removed ------------


def test_release_drill_pass_when_file_present() -> None:
    # Must run on the real committed tree: the release-cleared direction.
    assert DRILL_FILE.is_file(), f"drill classification file missing: {DRILL_FILE}"
    out = _run(str(WRAPPER), "verify", "--release", DRILL_RELEASE)
    assert out.returncode == 0, f"verify should PASS with file present:\n{out.stdout}{out.stderr}"
    assert "PASS" in out.stdout, "gate output must say PASS"


def test_release_drill_fails_when_file_removed() -> None:
    # Hermetic: copy the per-release dir to a temp repo, delete the file,
    # and confirm the gate blocks the release. Repo copy is read-only source.
    with tempfile.TemporaryDirectory() as td:
        fake_repo = Path(td) / "repo"
        (fake_repo / "P11").mkdir(parents=True)
        shutil.copytree(REL_DIR, fake_repo / "P11" / "export-control")
        victim = fake_repo / "P11" / "export-control" / f"{DRILL_RELEASE}.classification.toml"
        victim.unlink()  # absence = the repro
        out = _run(str(WRAPPER), "verify", "--release", DRILL_RELEASE, "--repo", str(fake_repo))
        assert out.returncode != 0, "gate must FAIL when the classification file is absent"
        assert "BLOCKED" in out.stdout, "failure must say the release is blocked"


def test_release_drill_fails_when_content_flagged() -> None:
    # Repo: a classification file that is present but flags restricted content
    # must also fail (the checkbox is not just existence, it is the assertion).
    with tempfile.TemporaryDirectory() as td:
        fake_repo = Path(td) / "repo"
        (fake_repo / "P11").mkdir(parents=True)
        shutil.copytree(REL_DIR, fake_repo / "P11" / "export-control")
        flag_file = fake_repo / "P11" / "export-control" / f"{DRILL_RELEASE}.classification.toml"
        text = flag_file.read_text(encoding="utf-8")
        text = text.replace("contains_itar_technical_data = false",
                            "contains_itar_technical_data = true")
        flag_file.write_text(text, encoding="utf-8")
        out = _run(str(WRAPPER), "verify", "--release", DRILL_RELEASE, "--repo", str(fake_repo))
        assert out.returncode != 0, "gate must FAIL when restricted content is flagged"
        assert "FLAGGED" in out.stdout, "failure must say the release is flagged"


# --- committed artifacts ----------------------------------------------------


def test_policy_doc_and_template_committed() -> None:
    assert DOC.is_file(), f"policy doc missing: {DOC}"
    text = DOC.read_text(encoding="utf-8")
    for needle in (
        "P11-T05",
        "P0-M2-T04",
        "UNCLASSIFIED",
        "data-locality",
        "data_locality",
        "ITAR",
        "EAR99",
        "5D002",
        "734.7(c)",
        "absence fails the release",
    ):
        assert needle in text, f"policy doc missing: {needle}"
    assert TPL.is_file(), f"template missing: {TPL}"
    # Template must keep the <V> placeholder (it is the form, not a release).
    assert "<V>" in TPL.read_text(encoding="utf-8")


def test_gate_tool_and_wrapper_executable() -> None:
    assert GATE_MOD.is_file()
    assert WRAPPER.is_file(), "gate wrapper missing"
    assert os.access(WRAPPER, os.X_OK), "verify-classification.sh must be executable"
    assert "P11-T05" in WRAPPER.read_text(encoding="utf-8")


def test_committed_release_classification_file_present() -> None:
    # Every release version must carry its own file; the drill release does.
    assert DRILL_FILE.is_file(), f"per-release classification file missing: {DRILL_FILE}"
    assert DRILL_RELEASE in DRILL_FILE.name


def test_selfcheck_clean_on_committed_tree() -> None:
    out = _run(sys.executable, GATE_MOD, "selfcheck", "--repo", str(REPO_ROOT))
    assert out.returncode == 0, f"selfcheck should be clean:\n{out.stdout}{out.stderr}"
    assert "PASS" in out.stdout


def test_ci_workflow_pins_the_release_branch() -> None:
    assert WORKFLOW.is_file(), f"CI workflow missing: {WORKFLOW}"
    text = WORKFLOW.read_text(encoding="utf-8")
    assert "release/" in text, "workflow must gate the release branch line"
    assert "export-control" in text, "workflow must reference the export-control job"
    assert "selfcheck" in text


def test_proof_recorded() -> None:
    proofs = sorted(PROOF_DIR.glob("export-control-drill-*.json"))
    assert proofs, "no export-control drill proof recorded under P11/proof/"
    proof = json.loads(proofs[-1].read_text(encoding="utf-8"))
    assert proof["release"] == DRILL_RELEASE
    legs = {leg["leg"]: leg for leg in proof["legs"]}
    assert legs["drill-present"]["exit_code"] == 0, "present leg must be PASS"
    assert legs["drill-removed"]["exit_code"] != 0, "removed leg must be FAIL"
    assert legs["drill-flagged"]["exit_code"] != 0, "flagged leg must be FAIL"


if __name__ == "__main__":
    # Standalone runner (pytest also collects these test_* functions).
    import traceback

    failures = 0
    for name, fn in sorted(globals().items()):
        if name.startswith("test_") and callable(fn):
            try:
                fn()
                print(f"[PASS] {name}")
            except AssertionError as exc:
                failures += 1
                print(f"[FAIL] {name}: {exc}")
                traceback.print_exc()
            except Exception as exc:  # noqa: BLE001
                failures += 1
                print(f"[FAIL] {name}: {exc!r}")
                traceback.print_exc()
    sys.exit(1 if failures else 0)
