"""IDENTITY-CONFORMANCE (t_d0941d83): the retired codename must not surface.

Don directive 2026-10-27: before any public surface opens, no retired codename
may appear on anything a public reader can see. This is the drift guard.

It fails when any of the three forbidden strings -- `keelworks`, `keelwb`, or
the retired `KW-` prefix -- appears (case-insensitive) anywhere in the repo
outside the explicitly named allowed paths below. The exemption list is kept
explicit and visible in this file so a future addition to it is a reviewed
edit, not an automatic one.

Scan policy (a text-integrity guard, not a forensic byte scanner):
  - the relative PATH of every file is always checked (so `kw-bracket.json`
    is caught, not just strings inside files);
  - a file's CONTENT is checked when it is text-like (its head decodes as
    UTF-8), for all three strings;
  - binary blobs are checked for the two full codename words only
    (`keelworks`, `keelwb`), never the 3-character `kw-` prefix, so random
    binary data (base64 blobs, packed formats) cannot false-positive the
    guard. Committed shipping binaries that DO carry the codename are
    recorded as explicit exemptions below.

Run: pytest tests/test_brand_identity.py   (or python3 tests/test_brand_identity.py)
"""

from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# Text + path patterns (case-insensitive via .lower()).
PATTERNS_STR = ("keelworks", "keelwb", "kw-")
# Binary-content patterns: the full codename words only (see module docstring).
PATTERNS_BYTES = (b"keelworks", b"keelwb")
# Every pattern, as bytes, used only to prove an allowed path is not dead
# (never on the broad repo scan).
ALL_PATTERNS_BYTES = (b"keelworks", b"keelwb", b"kw-")

# ---------------------------------------------------------------------------
# Explicit allowed paths. Every entry is a reviewed exemption; keep this list
# as small and as commented as possible.
# ---------------------------------------------------------------------------
ALLOWED_DIR_PREFIXES = (
    # The identity kit IS the retirement record: NAME.md documents the former
    # codename as retired, USAGE.md and the trademark evidence reference it.
    "assets/brand/",
    # Archived signed-release artifacts; their signatures cannot be reproduced.
    "workstreams/security/proof/signed-release-2027.1.0-rc1/",
)
ALLOWED_FILES = (
    # The charter carries the retirement statement.
    "charter.md",
    # The competitive parity research document records the identity-conformance
    # audit finding (retired KW- prefix) and its resolution; it is the docs-side
    # retirement record and must be able to name the codename it documents.
    "docs/competitive/parity-2026-10.md",
    # The trademark acceptance test uses the retired codename as its control
    # term to prove the collision-detection method is sensitive.
    "tests/test_brand_trademark_acceptance.py",
    # This drift test itself must contain the forbidden literals to search for
    # them.
    "tests/test_brand_identity.py",
    # Binary DR-restore proof fixture captured in the retired-brand era;
    # regenerating binary proof fixtures is explicitly out of scope for this
    # card (noted on t_d0941d83).
    "docs/delivery/disaster-recovery/proof/archive/geometry/box-part-a/1/box-part.3dxml",
    # Committed mechanism-motion capture whose rendered frames carry the old
    # part numbers; only the drawing sheet re-render was ordered this card.
    "docs-site/assets/images/04-mechanism-animation.mp4",
    # Verbatim `script(1)` session transcripts recorded on the retired host
    # name. Editing them would falsify the recorded capture; regenerating them
    # requires re-running the walkthrough on the renamed VM.
    "workstreams/delivery/update-checker/proof/video/machine-b-online.typescript",
    "workstreams/delivery/update-checker/proof/video/machine-b-online-sliced.typescript",
)

SKIP_DIRS = {
    ".git", "target", "__pycache__", ".venv", "node_modules",
    ".pytest_cache", ".mypy_cache", ".ruff_cache", ".herschel",
    # Sibling, untracked git worktree checkouts materialize under this name
    # when the scan runs from the primary checkout of a worktree-hosting repo
    # (the wave-A gate runs from such a tree). They are environment, not repo
    # content: never tracked, never shipped, and they carry retired-brand-era
    # files. Skipping keeps the guard green in a worktree and in CI alike.
    ".worktrees",
}

CONTENT_HEAD = 256 * 1024  # content scan budget per file (per path header)


def _rel(path: Path) -> str:
    """Repo-relative posix path, e.g. 'crates/oep-geo/src/lib.rs'."""
    return path.relative_to(REPO).as_posix()


def _is_allowed(rel: str) -> bool:
    return any(rel.startswith(p) for p in ALLOWED_DIR_PREFIXES) or rel in ALLOWED_FILES


def _content_hit(path: Path, patterns: tuple[bytes, ...]) -> bool:
    """Return True if any byte pattern appears in the file's bytes.

    Reads the whole file: the retired codename can sit deep in a binary
    (verified at offsets past 3 MB in the archived dist builds).
    """
    try:
        with open(path, "rb") as fh:
            data = fh.read(64 * 1024 * 1024)
    except OSError:
        return False
    data = data.lower()
    return any(p in data for p in patterns)


def _text_content_hit(path: Path, patterns: tuple[str, ...]) -> bool:
    """Lowercased decoded head for text-like files, else None (binary)."""
    try:
        with open(path, "rb") as fh:
            head = fh.read(CONTENT_HEAD)
    except OSError:
        return False
    try:
        text = head.decode("utf-8").lower()
    except UnicodeDecodeError:
        return False
    return any(p in text for p in patterns)


def _scan() -> list[str]:
    hits: list[str] = []
    for root, dirs, files in os.walk(REPO):
        dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS)
        for name in sorted(files):
            p = Path(root) / name
            if not p.is_file():
                continue
            rel = _rel(p)
            if _is_allowed(rel):
                continue
            rel_low = rel.lower()
            # path check (all patterns)
            if any(pat in rel_low for pat in PATTERNS_STR):
                hits.append(f"{rel}: forbidden string in the file path")
            # text content check (all patterns)
            if _text_content_hit(p, PATTERNS_STR):
                hits.append(f"{rel}: forbidden string in the file content")
            # binary content check (codename words only)
            elif _content_hit(p, PATTERNS_BYTES):
                hits.append(f"{rel}: retired codename in a binary file")
    return hits


def test_retired_codename_absent_outside_allowed_paths() -> None:
    hits = _scan()
    assert not hits, (
        "retired codename strings found outside the allowed paths:\n  "
        + "\n  ".join(hits)
    )


def test_scan_still_reports_a_planted_violation() -> None:
    """The .worktrees skip is a directory-list correction, not a blind spot: a
    real violation planted at a tracked-looking path inside the repo is still
    reported, so the guard never weakens.
    """
    probe = Path(tempfile.mkdtemp(prefix="brand-scan-probe-", dir=REPO))
    try:
        planted = probe / "keelworks-config.json"
        planted.write_text('{"name": "keelworks"}', encoding="utf-8")
        hits = _scan()
        assert hits, "scan must report a planted violation under %s" % probe.name
        assert any("keelworks-config.json" in h for h in hits), (
            "scan must name the planted file; got: %s" % hits
        )
    finally:
        shutil.rmtree(probe, ignore_errors=True)


def test_worktrees_copy_is_ignored() -> None:
    """A copy of the retired token inside .worktrees/<x>/ is environment (the
    sibling-checkout hosting dir), not repo content, and the scan must skip it.
    This is the defect-2 correction: the gate runs green from a worktree-hosted
    checkout and from a clean CI tree alike.
    """
    probe = REPO / ".worktrees" / ("brand-scan-probe-" + str(os.getpid()))
    probe.mkdir(parents=True, exist_ok=True)
    try:
        planted = probe / "keelworks-config.json"
        planted.write_text('{"name": "keelworks"}', encoding="utf-8")
        hits = _scan()
        worktrees_hits = [h for h in hits if ".worktrees" in h]
        assert not worktrees_hits, (
            ".worktrees copies must be ignored; got: %s" % worktrees_hits
        )
    finally:
        shutil.rmtree(probe, ignore_errors=True)
        # Remove the hosting dir only if we just created it and it is now empty
        # (os.rmdir refuses on a non-empty dir, so a real sibling-hosting
        # .worktrees/ is left untouched).
        try:
            (REPO / ".worktrees").rmdir()
        except OSError:
            pass


def test_allowed_paths_exist() -> None:
    """The exemption list must resolve, so it cannot rot silently."""
    missing = [p for p in ALLOWED_DIR_PREFIXES if not (REPO / p).exists()]
    missing += [f for f in ALLOWED_FILES if not (REPO / f).exists()]
    assert not missing, "allowed exemption path(s) missing: " + ", ".join(missing)


def test_allowed_paths_are_used() -> None:
    """Every exemption must actually carry a retired string somewhere today,
    else it is dead weight the next reviewer should remove."""
    unused: list[str] = []
    for prefix in ALLOWED_DIR_PREFIXES:
        found = False
        prefix_dir = REPO / prefix
        for root, dirs, files in os.walk(prefix_dir):
            if found:
                break
            for name in files:
                p = Path(root) / name
                rel_low = _rel(p).lower()
                if any(pat in rel_low for pat in PATTERNS_STR):
                    found = True
                    break
                try:
                    with open(p, "rb") as fh:
                        raw = fh.read(64 * 1024 * 1024).lower()
                    if any(bp in raw for bp in ALL_PATTERNS_BYTES):
                        found = True
                        break
                except OSError:
                    continue
        if not found:
            unused.append(prefix)
    for f in ALLOWED_FILES:
        path = REPO / f
        rel_low = f.lower()
        if any(pat in rel_low for pat in PATTERNS_STR):
            continue
        hit = False
        try:
            with open(path, "rb") as fh:
                raw = fh.read(64 * 1024 * 1024).lower()
            hit = any(bp in raw for bp in ALL_PATTERNS_BYTES)
        except OSError:
            hit = False
        if not hit:
            unused.append(f)
    assert not unused, (
        "allowed path(s) no longer carry the retired strings; remove them:\n  "
        + "\n  ".join(unused)
    )


def main() -> int:
    fns = [(n, f) for n, f in sorted(globals().items())
           if n.startswith("test_") and callable(f)]
    failures = []
    for name, fn in fns:
        try:
            fn()
            print(f"[PASS] {name}")
        except AssertionError as e:
            failures.append((name, e))
            print(f"[FAIL] {name}: {e}")
    print(f"{len(fns) - len(failures)}/{len(fns)} brand-identity checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
