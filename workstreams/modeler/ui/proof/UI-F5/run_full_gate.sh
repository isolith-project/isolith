#!/bin/bash
# UI-F5: full six-leg gate ON THE CHECKPOINT MERGED HEAD (ad90892), one
# transcript per leg, real command output only.
set -u
cd /home/r1a/isolith-main/.worktrees/ui-f5
CH="workstreams/modeler/ui/proof/UI-F5/transcripts/checkpoint-merged-head"
mkdir -p "$CH"

{
  echo "# UI-F5 six-leg gate on the checkpoint merged head"
  echo "# checkpoint merged head: $(git rev-parse HEAD)"
  echo "# parents: $(git log -1 --format='%P')"
  echo "# main IS an ancestor of the wave head, so this tree is byte-identical to the wave tree."
  echo "# git diff --stat HEAD github/wave/ui-f ->"
  git diff --stat HEAD github/wave/ui-f
  echo "# (the empty line above means the checkpoint tree == wave tree; the gate measures the same code)"
  echo "# rustc/cargo: $(rustc --version), $(cargo --version)"
  echo "# date: $(date -Is)"
} > "$CH/00-head.txt"

# ---- leg 1: fmt ----
{ echo "# cargo fmt --all -- --check (leg 1)"; echo "# rc is the LAST line"; \
  cargo fmt --all -- --check; echo "LEG1_FMT_RC=$?"; } > "$CH/01-fmt.txt" 2>&1

# ---- leg 2: clippy ----
{ echo "# cargo clippy --workspace --all-targets -- -D warnings (leg 2)"; echo "# rc is the LAST line"; \
  cargo clippy --workspace --all-targets -- -D warnings; echo "LEG2_CLIPPY_RC=$?"; } > "$CH/02-clippy.txt" 2>&1

# ---- leg 3: cargo test --workspace (at -j 4, the wave-E proven-safe level) ----
{ echo "# cargo test --workspace (leg 3) at -j 4"; echo "# rc is the LAST line"; \
  cargo test --workspace -j 4; echo "LEG3_TEST_RC=$?"; } > "$CH/03-test-workspace.txt" 2>&1

# ---- leg 4: licensing ----
{ echo "# python3 scripts/licensing-check.py (leg 4)"; echo "# rc is the LAST line"; \
  python3 scripts/licensing-check.py; echo "LEG4_LICENSING_RC=$?"; } > "$CH/04-licensing.txt" 2>&1

# ---- leg 5: precommit-gate ----
{ echo "# bash scripts/gate/precommit-gate.sh (leg 5)"; echo "# rc is the LAST line"; \
  bash scripts/gate/precommit-gate.sh; echo "LEG5_PRECOMMIT_RC=$?"; } > "$CH/05-precommit.txt" 2>&1

# ---- leg 6: shot.sh ----
{ echo "# bash scripts/ui/shot.sh (leg 6)"; echo "# rc is the LAST line"; \
  bash scripts/ui/shot.sh; echo "LEG6_SHOT_RC=$?"; } > "$CH/06-shot.txt" 2>&1

# ---- leg 7 (record): scope_register_check ----
{ echo "# python3 python/oep_tooling/scope_register_check.py (record, not a gate leg)"; echo "# rc is the LAST line"; \
  python3 python/oep_tooling/scope_register_check.py --register docs/scope-register.json --markdown docs/scope-register.md --out -; echo "LEG7_SCOPE_RC=$?"; } > "$CH/07-scope-register.txt" 2>&1

echo "===== GATE SUMMARY ====="
for f in 01-fmt 02-clippy 03-test-workspace 04-licensing 05-precommit 06-shot 07-scope-register; do
  rc=$(grep -oE "LEG[0-9]+_.*_RC=[0-9]+" "$CH/$f.txt" | tail -1)
  echo "$f: $rc"
done
