#!/usr/bin/env bash
# Adaptive pre-commit quality gate (v1.1), vendored into the Isolith repo
# from the Hermes `precommit-quality-gate` skill so the SAME gate runs locally
# (as .git/hooks/pre-commit) and in CI (scripts/gate run by Gitea Actions).
# Usage: precommit-gate.sh [repo-dir  (default: cwd; git runs hooks from the repo root)]
# Hard gates (syntax, tests) exit 1 and block the commit. Impeccable detector
# findings are advisory (warn), because design taste is not a commit-stopper;
# flip the two detector branches to set fail=1 if you want them to gate.
#
# Detector discovery: read `impeccable_cli` from `.precommit-gate.json`, else
# $IMPECCABLE_CLI. Invoke via `node <cli> detect` (the skill CLI), NOT npx:
# npx --no-install impeccable is NOT resolvable on a default host and would try
# to fetch from the registry at commit time.
set -u
cd "${1:-$(pwd)}"
# Git sets GIT_DIR/GIT_WORK_TREE/GIT_INDEX_FILE for hooks; scrub them so
# hermetic tests and tooling never inherit a pointer to the real repo
# (the incident-120 bug class recurs when a tool skips its own scrub).
unset "${!GIT_@}" 2>/dev/null || true
fail=0

echo "== quality gate: $(basename "$(pwd)") =="

PY=""
if [ -n "${VIRTUAL_ENV:-}" ]; then PY="$VIRTUAL_ENV/bin/python";
elif [ -x ".venv/bin/python" ]; then PY=".venv/bin/python";
elif command -v python3 >/dev/null 2>&1; then PY="python3"; fi

cfg=.precommit-gate.json
cli_val() { $PY -c "import json;print(json.load(open('$cfg')).get('$1',''))" 2>/dev/null; }
cli_sub() { $PY -c "import json;d=json.load(open('$cfg')).get('$1',{});print(d.get('$2',''))" 2>/dev/null; }

staged() { git diff --cached --name-only 2>/dev/null | grep -Eq "$1"; }

# 1) python syntax (only files that still exist: staged deletions have no body)
if staged '\.py$' && [ -n "$PY" ]; then
  files=$(git diff --cached --name-only --diff-filter=ACM | grep '\.py$' || true)
  [ -n "$files" ] || files=""
  if [ -n "$files" ]; then
    if $PY -m py_compile $files 2>/dev/null; then
      echo "  [ok]   python syntax"; else echo "  [FAIL] python syntax"; fail=1; fi
  else
    echo "  [skip] no staged .py to syntax-check (deletions/renames only)"
  fi
fi

# 2) python tests
if [ -n "$PY" ] && { [ -f pyproject.toml ] || [ -f pytest.ini ] || [ -d tests ]; }; then
  echo "  [run]  pytest"
  out=$($PY -m pytest -q 2>&1); rc=$?
  if [ $rc -eq 0 ]; then echo "  [ok]   pytest ($(echo "$out" | tail -1))";
  else echo "  [FAIL] pytest"; echo "$out" | tail -6; fail=1; fi
fi

# 3) cargo test
if [ -f Cargo.toml ] && command -v cargo >/dev/null 2>&1; then
  echo "  [run]  cargo test"; out=$(cargo test -q 2>&1); rc=$?
  if [ $rc -eq 0 ]; then echo "  [ok]   cargo test";
  else echo "  [FAIL] cargo test"; echo "$out" | tail -6; fail=1; fi
fi

# 4) go test
if [ -f go.mod ] && command -v go >/dev/null 2>&1; then
  echo "  [run]  go test ./..."
  if go test ./... >/dev/null 2>&1; then echo "  [ok]   go test";
  else echo "  [FAIL] go test"; fail=1; fi
fi

# 5) impeccable design detector (UI; advisory).
DET=$(cli_val impeccable_cli); [ -n "$DET" ] || DET="${IMPECCABLE_CLI:-}"
[ -n "$DET" ] && [ -x "$(command -v node)" ] || DET=""
UI_RE='\.(tsx|jsx|vue|svelte|astro|html|css|scss|sass|less)$'
have_ui=$(git diff --cached --name-only 2>/dev/null | grep -Ec "$UI_RE" || true)

if [ "${have_ui:-0}" -gt 0 ]; then
  if [ -n "$DET" ]; then
    files=$(git diff --cached --name-only | grep -E "$UI_RE")
    echo "  [run]  impeccable detect ($(echo "$files" | wc -l | tr -d ' ') UI file(s))"
    node "$DET" detect --quiet $files >/tmp/impeccable-file.log 2>&1
    if [ $? -eq 0 ]; then echo "  [ok]   impeccable detect clean";
    else echo "  [WARN] impeccable detect findings (advisory)"; sed 's/^/         /' /tmp/impeccable-file.log | tail -20; fi
  else echo "  [warn] no impeccable CLI (set impeccable_cli in $cfg or IMPECCABLE_CLI env)"; fi
else
  echo "  [skip] no UI files staged"
  if [ -f "$cfg" ] && [ -n "$DET" ]; then
    url=$(cli_sub render url); boot=$(cli_sub render boot); wait="$(cli_sub render wait_sec)"
    if [ -n "$url" ] && [ -n "$boot" ]; then
      echo "  [run]  render app + impeccable detect $url"
      ( eval "$boot" ) >/tmp/precommit-app.log 2>&1 &
      pid=$!; sleep "${wait:-6}"
      node "$DET" detect --quiet "$url" >/tmp/impeccable-render.log 2>&1; rc=$?
      kill "$pid" 2>/dev/null
      if [ $rc -eq 0 ]; then echo "  [ok]   impeccable detect (render) clean";
      else echo "  [WARN] impeccable detect (render) findings"; sed 's/^/         /' /tmp/impeccable-render.log | tail -20; fi
    else echo "  [skip] render config incomplete"; fi
  else echo "  [skip] render mode not configured"; fi
fi

echo "== gate $([ $fail -eq 0 ] && echo PASS || echo FAIL) =="
exit $fail
