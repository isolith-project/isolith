#!/usr/bin/env bash
# check-workflow-hardening.sh - enforce the Actions hardening rows of
# docs/hardening/public-repo-hardening.md (section 4) over every workflow in
# .github/workflows. Runs in CI as .github/workflows/hardening.yaml and runs
# locally against any checked-out tree.
#
# Rules enforced (each a hard FAIL; exit non-zero on any violation):
#   R1  every job declares an explicit `permissions:` block (no job inherits
#       workspace write by default);
#   R2  every `uses:` reference to an out-of-repo action pins a full 40-hex
#       commit SHA (no tag/branch refs for third-party actions);
#   R3  no `pull_request_target` job that checks out or executes the PR head
#       or that passes a secret into the same job that touches contributor
#       content.
#
# Local action paths (./.github/actions/...) and `docker://` images are not
# governed by R2.
set -euo pipefail

ROOT="${1:-$(pwd)}"
cd "$ROOT"
FAIL=0
found=0

if [ ! -d .github/workflows ]; then
  echo "no .github/workflows directory; nothing to check"
  exit 0
fi

python3 - <<'PY'
import glob, re, sys

files = sorted(glob.glob('.github/workflows/*.y*ml'))
if not files:
    sys.exit(0)

fail = 0
sha_re = re.compile(r'^[0-9a-f]{40}$')

for path in files:
    text = open(path, encoding='utf-8').read()
    lines = text.splitlines()

    # R3: pull_request_target on any PR head is banned outright in the
    # current posture (baseline: no trusted-PR workflows yet). Detect the real
    # YAML trigger key, not prose in comments (a comment may say the name).
    prt_re = re.compile(r'^\s*pull_request_target\s*:\s*(#.*)?$')
    for i, ln in enumerate(lines, 1):
        if prt_re.match(ln):
            print(f"{path}:{i}: R3 pull_request_target trigger present; not allowed "
                  f"until a reviewed pattern exists (base-checkout only, no secrets)")
            fail = 1

    # R1: permissions per job. Parse top-level jobs; each job block must have a
    # `permissions:` child, or the file must declare top-level permissions AND
    # every job still records its own (the strict rule). We require per-job.
    # Simple lexical pass: each line at column 2 starting `jobs:` then job keys
    # at column 2; within a job, find a line `  permissions:` before the next
    # column-2 key.
    in_jobs = False
    job = None
    job_line = 0
    perm_ok = True
    job_perms = {}   # job -> (line, has_permissions)
    cur = None
    for i, ln in enumerate(lines):
        m = re.match(r'^jobs:', ln)
        cur_indent = len(ln) - len(ln.lstrip(' '))
        stripped = ln.strip()
        if m:
            in_jobs = True
            continue
        if in_jobs:
            if cur_indent == 0 and stripped:
                break
            if cur_indent == 2 and stripped and not stripped.startswith(('#','-')) and not ln.startswith('   '):
                # a new job key (or first line under jobs is a key)
                cur = stripped.rstrip(':')
                job_perms[cur] = [i+1, False]
                continue
            if cur and cur_indent == 4 and stripped.startswith('permissions:'):
                job_perms[cur][1] = True
    for j, (line, has) in job_perms.items():
        if not has:
            print(f"{path}:{line}: R1 job '{j}' declares no explicit permissions: block")
            fail = 1

    # R2: uses: must be SHA-pinned for out-of-repo actions. Steps are YAML
    # list items ("- uses: ..."), so strip the leading list marker first.
    for i, ln in enumerate(lines, 1):
        s = ln.strip()
        if s.startswith('- '):
            s = s[2:].strip()
        m = re.match(r'^uses:\s*(\S+)', s)
        if not m:
            continue
        target = m.group(1)
        if target.startswith('./'):
            continue          # local composite action
        if target.startswith('docker://'):
            continue
        if '@' not in target:
            print(f"{path}:{i}: R2 action '{target}' has no @ref; pin a 40-hex SHA")
            fail = 1
            continue
        ownerrepo, ref = target.rsplit('@', 1)
        if not sha_re.match(ref):
            print(f"{path}:{i}: R2 action '{target}' pinned to '{ref}' not a commit SHA")
            fail = 1

sys.exit(fail)
PY
rc=$?
[ $rc -ne 0 ] && FAIL=1

if [ $FAIL -eq 0 ]; then
  echo "workflow hardening: PASS"
else
  echo "workflow hardening: FAIL (see rows above)"
  exit 1
fi
