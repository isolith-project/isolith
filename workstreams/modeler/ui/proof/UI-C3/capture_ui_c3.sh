#!/usr/bin/env bash
# UI-C3 UI-VARIABLE-TABLE: capture the real gate output into transcripts/.
#
# Runs each acceptance command once and saves its real output (tee'd), the
# same discipline every wave card follows. All outputs here are the actual
# command outputs on this branch, never a summary.
set -uo pipefail
# Repo root regardless of where the script lives (works in a git worktree).
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

T=workstreams/modeler/ui/proof/UI-C3/transcripts
mkdir -p "$T"

{
    echo '# STEP 0: merge the wave head into the card branch'
    git fetch github 2>&1
    echo "wave head: $(git rev-parse --short github/wave/ui-c)"
    git merge github/wave/ui-c 2>&1 | tail -3
    echo "POST-STEP HEAD: $(git rev-parse --short HEAD)"
} | tee "$T/00-step0-merge.txt"

{
    echo '# cargo fmt --all -- --check'
    cargo fmt --all -- --check
    echo "fmt exit=$?"
} | tee "$T/01-fmt.txt"

{
    echo '# cargo clippy -p oep-app --all-targets -- -D warnings'
    cargo clippy -p oep-app --all-targets -- -D warnings 2>&1 | tail -4
    echo "clippy exit=${PIPESTATUS[0]}"
} | tee "$T/02-clippy.txt"

{
    echo '# cargo test -p oep-app --test variable_table'
    cargo test -p oep-app --test variable_table 2>&1 | grep -E "^(test result:|error|failures)"
} | tee "$T/03-variable_table.txt"

{
    echo '# cargo test -p oep-app (per-target summaries)'
    cargo test -p oep-app 2>&1 | grep -E "^test result:"
    echo "-- aggregate --"
    cargo test -p oep-app 2>&1 | grep -E "^test result:" | python3 -c "
import sys, re
t=f=0; n=0
for line in sys.stdin:
    m=re.search(r'(\d+) passed; (\d+) failed', line)
    if m: t+=int(m.group(1)); f+=int(m.group(2)); n+=1
print(f'{n} targets, {t} passed, {f} failed')
"
} | tee "$T/04-oep-app.txt"

{
    echo '# cargo test --workspace (aggregate)'
    cargo test --workspace > /tmp/ui-c3-ws.log 2>&1
    echo "workspace exit=$?"
    grep -E "^test result:" /tmp/ui-c3-ws.log | python3 -c "
import sys, re
t=f=0; n=0
for line in sys.stdin:
    m=re.search(r'(\d+) passed; (\d+) failed', line)
    if m: t+=int(m.group(1)); f+=int(m.group(2)); n+=1
print(f'{n} targets, {t} passed, {f} failed')
"
} | tee "$T/05-workspace.txt"

{
    echo '# xvfb-run -a cargo run -p oep-app -- --smoke'
    timeout 300 xvfb-run -a cargo run -p oep-app -- --smoke 2>&1 | tail -4
    echo "smoke exit=${PIPESTATUS[0]}"
} | tee "$T/06-smoke.txt"
