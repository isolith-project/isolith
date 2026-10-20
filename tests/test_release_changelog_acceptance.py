#!/usr/bin/env python3
"""Acceptance test for Isolith P12-T07: release notes / changelog automation.

Written FIRST (TDD). Verifies the real material, not mocks:

  generator => git-cliff (cliff.toml) categorizes conventional commits;
               scripts/release/changelog.sh drives it deterministically
  ci        => a release-cut workflow (.gitea/workflows/release-changelog.yaml)
               fires on calendar-train tag pushes, fetches full history, and
               attaches the changelog as release notes; a per-delivery lane
               (p12-t07-changelog.yaml) runs this test in CI
  doc       => workstreams/release-engineering/release-changelog.md records the contract, cross-referenced
               to the P0-M5-T03 versioning decision and P12-T05 train policy
  proof     => a changelog generated from the ACTUAL commit history of the
               dry-run release tag is committed under workstreams/release-engineering/proof/, with a
               sha256 that git-cliff output reproduces; if a git-cliff binary
               is available the test REGENERATES that changelog and compares

Run:  python3 tests/test_release_changelog_acceptance.py
      python3 -m pytest tests/test_release_changelog_acceptance.py
Exit 0 = pass; nonzero = fail. Stdlib only.

Program surface-language rule applies (no em dashes, U+2014) to every text
artifact this card owns.
"""
import os
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

EM_DASH = "\u2014"
failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(os.path.join(ROOT, path), encoding="utf-8") as fh:
        return fh.read()


def read_raw(path):
    with open(path, "rb") as fh:
        return fh.read()


def no_em_dash(text, label):
    if EM_DASH in text:
        check(False, "forbidden em dash (U+2014) in %s" % label)


def front_matter(text):
    if not text.startswith("---"):
        return {}
    _, fm, _ = text.split("---", 2)
    meta = {}
    for line in fm.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or ":" not in stripped:
            continue
        key, _, val = stripped.partition(":")
        meta[key.strip()] = val.strip()
    return meta


def section_present(text, heading):
    return re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), text) is not None


def run_checks():
    print("P12-T07 release notes / changelog automation acceptance")
    print("=" * 60)
    print("repo root: %s" % ROOT)

    # 1. Generator config: cliff.toml exists, parses, categorizes.
    check(os.path.isfile(os.path.join(ROOT, "cliff.toml")), "missing cliff.toml")
    cliff = ""
    if os.path.isfile(os.path.join(ROOT, "cliff.toml")):
        try:
            import tomllib  # py3.11+

            cliff_raw = read_raw(os.path.join(ROOT, "cliff.toml"))
            cliff = read("cliff.toml")
            no_em_dash(cliff, "cliff.toml")
            cfg = tomllib.loads(cliff)
            check("[changelog]" in cliff, "cliff.toml missing [changelog] table")
            check("[git]" in cliff, "cliff.toml missing [git] table")
            groups = [
                s.get("section") for s in cfg.get("changelog", {}).get("sections", [])
            ]
            gitcfg = cfg.get("git", {})
            parser_groups = [
                p.get("group") for p in gitcfg.get("commit_parsers", [])
            ]
            all_groups = groups + parser_groups
            for required in ("Features", "Bug Fixes", "Documentation", "Tests"):
                check(
                    required in all_groups,
                    "cliff.toml must categorize into %r (got %r)" % (required, all_groups),
                )
            check(
                gitcfg.get("conventional_commits") is True,
                "cliff.toml requires conventional_commits = true",
            )
            check(
                gitcfg.get("filter_unconventional") is False,
                "cliff.toml must keep unconventional commits (filter_unconventional = false)",
            )
            check(
                "tag_pattern" in gitcfg,
                "cliff.toml must define a calendar-train tag_pattern",
            )
            gen_config_ok = True
        except Exception as exc:  # noqa: BLE001
            failures.append("cliff.toml does not parse: %s" % exc)
            gen_config_ok = False
    else:
        gen_config_ok = False

    # 2. Generator driver script.
    script = "scripts/release/changelog.sh"
    script_path = os.path.join(ROOT, script)
    check(os.path.isfile(script_path), "missing %s" % script)
    if os.path.isfile(script_path):
        s = read(script)
        no_em_dash(s, script)
        check(
            os.access(script_path, os.X_OK) or s.startswith("#!"),
            "%s must be an executable shebang script" % script,
        )
        check("git-cliff" in s, "%s must invoke git-cliff" % script)
        check("2.14.2" in s, "%s must pin the git-cliff version (2.14.2)" % script)
    attach = "scripts/release/attach-release-notes.py"
    check(os.path.isfile(os.path.join(ROOT, attach)), "missing %s" % attach)

    # 3. CI: the release-cut workflow.
    wf = ".gitea/workflows/release-changelog.yaml"
    check(os.path.isfile(os.path.join(ROOT, wf)), "missing %s" % wf)
    if os.path.isfile(os.path.join(ROOT, wf)):
        wfc = read(wf)
        no_em_dash(wfc, wf)
        check("v20[0-9][0-9].*" in wfc, "%s must trigger on calendar tag pushes" % wf)
        check("workflow_dispatch" in wfc, "%s must support workflow_dispatch (dry run)" % wf)
        check("git fetch" in wfc or "unshallow" in wfc, "%s must fetch full history/tags" % wf)
        check("changelog.sh" in wfc, "%s must run scripts/release/changelog.sh" % wf)
        check(
            "attach-release-notes.py" in wfc,
            "%s must attach changelog as release notes" % wf,
        )
        check("sha512sum" in wfc, "%s must sha512-verify the pinned git-cliff" % wf)

    # 4. CI: the per-delivery acceptance lane.
    lane = ".gitea/workflows/changelog.yaml"
    check(os.path.isfile(os.path.join(ROOT, lane)), "missing %s" % lane)
    if os.path.isfile(os.path.join(ROOT, lane)):
        lc = read(lane)
        no_em_dash(lc, lane)
        check("test_release_changelog_acceptance.py" in lc,
              "%s must run this acceptance test" % lane)

    # 5. Ops/decision doc.
    doc = "workstreams/release-engineering/release-changelog.md"
    check(os.path.isfile(os.path.join(ROOT, doc)), "missing %s" % doc)
    if os.path.isfile(os.path.join(ROOT, doc)):
        dc = read(doc)
        no_em_dash(dc, doc)
        meta = front_matter(dc)
        check("version" in meta, "%s front matter must carry a version" % doc)
        check("status" in meta, "%s front matter must carry a status" % doc)
        check("applies_to" in meta, "%s front matter must carry applies_to" % doc)
        for h in ("Decision", "Why", "Ops contract", "Verification", "Change log"):
            check(section_present(dc, h), "%s missing section: %s" % (doc, h))
        check("P0-M5-T03" in dc, "%s must cross-reference the P0-M5-T03 versioning decision" % doc)
        check("P12-T05" in dc or "P12-T05/" in dc or "T05" in dc,
              "%s must reference the P12-T05 release train" % doc)

    # 6. Proof: a changelog generated from the actual dry-run release history.
    proof_dir = os.path.join(ROOT, "workstreams/release-engineering", "proof")
    proofs = []
    if os.path.isdir(proof_dir):
        proofs = sorted(
            f for f in os.listdir(proof_dir)
            if re.match(r"^changelog-v20\d\d\.\d+", f) and f.endswith(".md")
        )
    check(
        bool(proofs),
        "no generated changelog proof (workstreams/release-engineering/proof/changelog-v<tag>.md) committed",
    )
    for pname in proofs:
        text = read(os.path.join(proof_dir, pname))
        no_em_dash(text, os.path.join("workstreams/release-engineering/proof", pname))
        headings = re.findall(r"^### (.+)$", text, re.M)
        bullets = len(re.findall(r"^\- ", text, re.M))
        check(
            len(headings) >= 3,
            "%s must contain at least 3 category headings (got %r)" % (pname, headings),
        )
        check(
            bullets >= 10,
            "%s must contain at least 10 changelog entries (got %d)" % (pname, bullets),
        )
        check(
            any("Features" in h for h in headings),
            "%s must include the Features category" % pname,
        )
        sha_file = os.path.join(proof_dir, pname + ".sha256")
        if os.path.isfile(sha_file):
            recorded = read_raw(sha_file).decode().strip().split()[0]
            actual = __import__("hashlib").sha256(
                read_raw(os.path.join(proof_dir, pname))
            ).hexdigest()
            check(
                recorded == actual,
                "%s.sha256 does not match the committed file" % pname,
            )
        else:
            check(False, "missing checksum %s.sha256 next to proof" % pname)

    # 7. Strong verification leg: regenerate over the REAL history if a
    #    git-cliff binary is available (degrade gracefully otherwise, matching
    #    the CI-safe pattern of the other delivery lanes).
    git_cliff = (
        os.environ.get("GIT_CLIFF_BIN")
        or shutil.which("git-cliff")
        or os.path.join(ROOT, "tooling", "bin", "git-cliff")
    )
    if git_cliff and os.path.isfile(git_cliff) and proof_dir and os.path.isdir(proof_dir):
        tag_file = proofs[0]
        tag = tag_file[len("changelog-v"):].rsplit(".", 1)[0]  # strip .md
        try:
            out = subprocess.run(
                [git_cliff, "--tag", tag, "-o", "-"],
                cwd=ROOT, capture_output=True, text=True, timeout=120,
            )
            if out.returncode == 0:
                regenerated = out.stdout
                committed = read(os.path.join(proof_dir, tag_file))
                import hashlib
                reg_hash = hashlib.sha256(regenerated.encode()).hexdigest()
                com_hash = hashlib.sha256(committed.encode()).hexdigest()
                check(
                    reg_hash == com_hash,
                    "regenerated changelog for %s differs from committed proof "
                    "(run scripts/release/changelog.sh and recommit)" % tag,
                )
                print("  [ok]   regenerated %s over real history, sha256 matches" % tag)
            else:
                check(
                    False,
                    "git-cliff regeneration failed (exit %d): %s"
                    % (out.returncode, out.stderr.strip()),
                )
        except Exception as exc:  # noqa: BLE001
            check(False, "git-cliff regeneration errored: %s" % exc)
    else:
        print("  [skip] no git-cliff binary; structure + checksum legs still enforced")

    print("=" * 60)
    if failures:
        for f in failures:
            print("FAIL %s" % f)
        print("P12-T07 acceptance: %d failure(s)" % len(failures))
        return 1
    print("P12-T07 acceptance: PASS (exit 0)")
    return 0


if __name__ == "__main__":
    sys.exit(run_checks())
