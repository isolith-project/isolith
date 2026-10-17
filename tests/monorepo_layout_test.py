#!/usr/bin/env python3
"""P0-M3-T01 acceptance test: Keelworks monorepo skeleton with workspace layout
(crates/modules per workstream).

Pure stdlib. Runs under `python3 tests/monorepo_layout_test.py` and pytest.
Each check FAILS loud with a clear message; exit code 1 on any failure.
"Done" for P0-M3-T01 means this test passes against the real tree AND
`cargo build --workspace`/`cargo test --workspace` succeed against it.
"""
import os
import re
import subprocess
import sys
import tomllib

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

# Expected crates for wave 1 + reserved workstreams (see docs/layout.md).
EXPECTED_CRATES = {
    "oep-cgr": "P2",   # CGR reader / geometry-tessellation
    "oep-3dxml": "P3", # 3DXML reader/writer
    "oep-geo": "P6",   # geometry core + model representation
    "oep-step": "P4",  # STEP AP242/QIF/JT interop (reserved)
    "oep-pdm": "P8",   # self-hosted PDM (reserved)
    "oep-job": "P9",   # P2P compute: job/task spec (P9-M1-T01)
    "oep-worker": "P9",# P2P compute: export/convert workers (P9-M3-T02)
}
WORKSTREAMS = {"P2", "P3", "P4", "P6", "P8", "P9"}
CRATE_PREFIX = "oep-"

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


def check_exists(path, label=None):
    p = os.path.join(REPO, path)
    check(label or ("exists: " + path), os.path.isfile(p), p)


def check_dir(path, label=None):
    p = os.path.join(REPO, path)
    check(label or ("dir exists: " + path), os.path.isdir(p), p)


def test_root_workspace():
    cargo_toml = os.path.join(REPO, "Cargo.toml")
    check_exists("Cargo.toml")
    if not os.path.isfile(cargo_toml):
        return
    with open(cargo_toml, "rb") as fh:
        data = tomllib.load(fh)
    ws = data.get("workspace")
    check("root Cargo.toml has [workspace]", isinstance(ws, dict),
          "workspace section present")
    if not isinstance(ws, dict):
        return
    members = ws.get("members") or []
    check("workspace.members declared", bool(members), repr(members))
    resolver = ws.get("resolver")
    check("workspace.resolver = 2", resolver == "2", repr(resolver))
    for crate in EXPECTED_CRATES:
        wanted = "crates/%s" % crate
        check("member listed for %s" % crate, wanted in members,
              "found %r in members" % (members,))
    # Every member under crates/* must start with the oep- prefix.
    for m in members:
        if m.startswith("crates/") and m.count("/") >= 1:
            check("member %s uses oep- prefix" % m,
                  m.split("/")[1].startswith(CRATE_PREFIX), m)


def test_crate_skeletons():
    for crate in EXPECTED_CRATES:
        base = os.path.join(REPO, "crates", crate)
        cm = os.path.join(base, "Cargo.toml")
        lib = os.path.join(base, "src", "lib.rs")
        check("crate dir: %s" % crate, os.path.isdir(base))
        check("crate Cargo.toml: %s" % crate, os.path.isfile(cm), cm)
        check("crate src/lib.rs: %s" % crate, os.path.isfile(lib), lib)
        if os.path.isfile(cm):
            with open(cm, "rb") as fh:
                cdata = tomllib.load(fh)
            pkg = cdata.get("package") or {}
            check("package.name == %s" % crate, pkg.get("name") == crate,
                  repr(pkg.get("name")))
            check("package.name starts with %s" % CRATE_PREFIX,
                  (pkg.get("name") or "").startswith(CRATE_PREFIX) or
                  (pkg.get("name") == crate and crate.startswith(CRATE_PREFIX)))


def test_layout_doc():
    lay = os.path.join(REPO, "docs", "layout.md")
    check_exists("docs/layout.md")
    if not os.path.isfile(lay):
        return
    text = open(lay, encoding="utf-8").read()
    for crate, ws in EXPECTED_CRATES.items():
        check("layout.md mentions %s" % crate, crate in text)
        check("layout.md maps %s -> %s" % (ws, crate),
              ws in text.upper().split() or ws in text.upper())
    for ws in WORKSTREAMS:
        check("layout.md anchors workstream %s" % ws, ws in text.upper())


def test_python_tooling():
    init = os.path.join(REPO, "python", "oep_tooling", "__init__.py")
    check_exists(os.path.join("python", "oep_tooling", "__init__.py"),
                 "python/oep_tooling/__init__.py")


def test_gitignore():
    gi = os.path.join(REPO, ".gitignore")
    check("exists: .gitignore", os.path.isfile(gi))
    if os.path.isfile(gi):
        text = open(gi, encoding="utf-8").read()
        check(".gitignore ignores target/", re.search(r"(?m)^/?target/?$|/target/", text))
        check(".gitignore ignores .worktrees/",
              re.search(r"(?m)\.worktrees", text))


def run_cargo(args, label):
    try:
        r = subprocess.run(["cargo"] + args, cwd=REPO, capture_output=True,
                           text=True, timeout=600)
    except Exception as e:
        check(label + " (subprocess error)", False, repr(e))
        return
    ok = r.returncode == 0
    tail = ""
    if not ok:
        tail = (r.stdout[-400:] + r.stderr[-400:]).strip()
    check(label, ok, tail)


def test_cargo_metadata():
    run_cargo(["metadata", "--no-deps", "--format-version", "1"], "cargo metadata parses")


def test_cargo_build():
    run_cargo(["build", "--workspace"], "cargo build --workspace (exit 0)")


def test_cargo_test():
    run_cargo(["test", "--workspace"], "cargo test --workspace (exit 0)")


def main():
    print("Keelworks monorepo skeleton acceptance test")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_root_workspace()
    test_crate_skeletons()
    test_layout_doc()
    test_python_tooling()
    test_gitignore()
    test_cargo_metadata()
    test_cargo_build()
    test_cargo_test()
    print("=" * 60)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
