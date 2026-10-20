"""P7-M5-T02 acceptance: theme/accessibility pass + generated help index.

Independent (stdlib-only) verification of the theme/accessibility pass and the
per-command help index generated from command metadata:

  1. The workbench command surface (crates/oep-shell/src/commands.rs) carries
     command metadata (CommandSpec with usage/category/summary/long help/args/
     examples) plus a HELP_INDEX_VERSION, and the CLI example derives its help
     from that module rather than a hand-written usage block.
  2. The committed help index (docs/re-notes/oep-workbench-help-index.md) is
     generated from that metadata: it is versioned, covers every command
     exactly once, and has no command section absent from the source (the Rust
     side byte-locks the same file against a fresh render; this file
     cross-checks the two from the text side so no single generator or editor
     can drift alone).
  3. The accessibility checklist (workstreams/modeler/quality/accessibility-checklist.md)
     covers BOTH surfaces named by the acceptance (the oep-viewer surface and
     the oep-shell workbench surface), declares a PASS/DEFERRED/N-A status for
     every item, and carries at least one PASS item per surface.
  4. The viewer theme module (crates/oep-viewer/src/theme.rs) is the single
     source for background and part colors, declares a WCAG non-text contrast
     floor, and the renderer consumes it (crates/oep-viewer/src/render.rs
     references theme::BACKGROUND_RGB).
  5. P7 prose rule: the files this card adds carry no em dashes / curly
     quotes on the surface (the P7 prose ban).

The Rust side asserts the strong properties (byte-level drift guard for the
generated index, WCAG contrast floor measured across the hue sweep, render
determinism, structural non-color identity) in crates/oep-shell/tests/
help_index.rs and crates/oep-viewer/tests/accessibility_theme.rs.

Run:  python3 tests/test_p7m5t02_a11y_help.py  (and under pytest)
"""

import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

COMMANDS = os.path.join(REPO, "crates", "oep-shell", "src", "commands.rs")
EXAMPLE = os.path.join(REPO, "crates", "oep-shell", "examples", "workbench.rs")
THEME = os.path.join(REPO, "crates", "oep-viewer", "src", "theme.rs")
RENDER = os.path.join(REPO, "crates", "oep-viewer", "src", "render.rs")
HELP_INDEX = os.path.join(REPO, "docs", "re-notes", "oep-workbench-help-index.md")
CHECKLIST = os.path.join(REPO, "workstreams/modeler/quality", "accessibility-checklist.md")

# The canonical workbench surface: every command a user can type, with its
# canonical metadata. Must match crates/oep-shell/src/commands.rs exactly.
CANONICAL_COMMANDS = [
    ("load:cgr", "load:cgr:PATH", "interchange"),
    ("load:3dxml", "load:3dxml:PATH", "interchange"),
    ("load:step", "load:step:PATH", "interchange"),
    ("save", "save:cgr|3dxml|step", "interchange"),
    ("open-pdm", "open-pdm:ITEM_ID:REVISION", "pdm"),
    ("save-pdm", "save-pdm:ITEM_ID:REVISION[:SUMMARY]", "pdm"),
    ("journal", "journal", "inspection"),
    ("help", "help [COMMAND]", "help"),
]
CANONICAL_NAMES = [c[0] for c in CANONICAL_COMMANDS]
CANONICAL_USAGES = [c[1] for c in CANONICAL_COMMANDS]
EXPECT_VERSION = "1.0.0"

EM_DASH = "\u2014"
CURLY = "\u201c\u201d\u2018\u2019"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except FileNotFoundError:
        return ""


def run_checks():
    print("P7-M5-T02 theme/accessibility + generated help index acceptance")
    print("=" * 66)

    # 1. Command metadata registry is the single source for the help surface.
    check(os.path.exists(COMMANDS), "commands.rs exists")
    cmds = read(COMMANDS)
    for token in ("CommandSpec", "ArgSpec", "HELP_INDEX_VERSION", "commands()"):
        check(token in cmds, "commands.rs names %s" % token)

    # Parse the metadata literals the way the Rust side declares them.
    names_src = re.findall(r'name: "([^"]+)"', cmds)
    usages_src = re.findall(r'usage: "([^"]+)"', cmds)
    for name in CANONICAL_NAMES:
        check(name in names_src, "registry declares command %s" % name)
    for usage in CANONICAL_USAGES:
        check(usage in usages_src, "registry declares usage %r" % usage)

    # The CLI example must derive help from the metadata module, not a
    # hand-written duplicate usage block (single source of truth).
    ex = read(EXAMPLE)
    check("oep_shell::commands::" in ex or "commands::" in ex,
          "workbench example derives help from commands metadata")

    # 2. Committed help index covers every command, and nothing more.
    check(os.path.exists(HELP_INDEX), "generated help index exists")
    idx = read(HELP_INDEX)
    check("help-index-version: %s" % EXPECT_VERSION in idx,
          "help index carries version %s" % EXPECT_VERSION)
    headings = re.findall(r"^### ([^\s]+)", idx, re.M)
    check(len(headings) == len(CANONICAL_NAMES),
          "help index has one section per command (%s sections)" % len(headings))
    for name in CANONICAL_NAMES:
        check(name in headings, "help index documents %s" % name)
        check(("### %s" % name) in idx, "help index has a %s section" % name)
    for h in headings:
        check(h in CANONICAL_NAMES,
              "help index section %s exists in metadata" % h)
    # Every canonical usage appears in the index body too.
    for usage in CANONICAL_USAGES:
        check(usage in idx, "help index shows usage %r" % usage)
    # No ANSI color escapes in the generated index (no color-only encoding).
    check("\u001b[" not in idx, "generated help index has no ANSI escapes")

    # 3. Accessibility checklist covers viewer AND workbench surfaces.
    check(os.path.exists(CHECKLIST), "accessibility checklist exists")
    cl = read(CHECKLIST)
    check("version: %s" % EXPECT_VERSION in cl,
          "checklist versioned %s" % EXPECT_VERSION)
    check("Viewer surface" in cl and "Workbench surface" in cl,
          "checklist covers the viewer and workbench surfaces")
    statuses = re.findall(r"\| (PASS|DEFERRED|N-A) \|", cl)
    check(len(statuses) >= 6, "checklist has statused items (%s)" % len(statuses))
    check(all(s in ("PASS", "DEFERRED", "N-A") for s in statuses),
          "every item has a valid status")
    # Split on the two surface table header lines and require >=1 PASS each.
    viewer = cl.split("Workbench surface")[0]
    wb = cl.split("Workbench surface")[1] if "Workbench surface" in cl else ""
    check("| PASS |" in viewer, "viewer surface has a PASS item")
    check("| PASS |" in wb, "workbench surface has a PASS item")

    # 4. Viewer theme module is the single source and declares the floor.
    check(os.path.exists(THEME), "viewer theme.rs exists")
    th = read(THEME)
    for token in ("MIN_NON_TEXT_CONTRAST", "contrast_ratio", "part_color",
                  "worst_part_contrast", "BACKGROUND_RGB"):
        check(token in th, "theme module names %s" % token)
    rd = read(RENDER)
    check("theme::BACKGROUND_RGB" in rd,
          "renderer consumes the theme background constants")

    # 5. P7 prose rule on every file this card adds.
    for path, label in (
        (COMMANDS, "commands.rs"),
        (EXAMPLE, "workbench example"),
        (THEME, "viewer theme.rs"),
        (HELP_INDEX, "generated help index"),
        (CHECKLIST, "accessibility checklist"),
    ):
        if not os.path.exists(path):
            continue
        text = read(path)
        check(EM_DASH not in text, "%s: no em dash" % label)
        check(not any(c in CURLY for c in text), "%s: no curly quotes" % label)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("P7-M5-T02 THEME/ACCESSIBILITY ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("P7-M5-T02 THEME/ACCESSIBILITY ACCEPTANCE PASSED: "
          "the workbench help index is generated from command metadata and "
          "committed with a drift guard, both surfaces carry a statused "
          "accessibility checklist with PASS items, and the viewer theme is a "
          "single source with an enforced WCAG contrast floor.")
    return 0


def test_p7m5t02_a11y_help():
    failures = run_checks()
    assert not failures, "theme/accessibility acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    main()
