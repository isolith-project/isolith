#!/bin/bash
# UI-A8 acceptance #1: every wave-A card branch is an ancestor of the wave head,
# and every card's proof dir exists on the wave head.
set -u
WAVE=github/wave/ui-a
CARDS="ui/a1-deps ui/a2-harness ui/a3-theme ui/a4-layout ui/a5-viewport ui/a6-palette ui/a7-open-save ui/a9-contrast"
rc=0
for b in $CARDS; do
  if git merge-base --is-ancestor "$b" "$WAVE" 2>/dev/null; then
    echo "ANCESTOR ok      : $b is an ancestor of $WAVE"
  else
    echo "ANCESTOR FAIL    : $b is NOT an ancestor of $WAVE"
    rc=1
  fi
done
echo
echo "=== wave head ==="
git log --oneline -1 "$WAVE"
echo
echo "=== proof dir presence on wave head ==="
for d in ui-a1 ui-a2 ui-a3 ui-a4 ui-a5 ui-a6 ui-a7 ui-a8 ui-a9; do
  if git cat-file -e "$WAVE:workstreams/modeler/ui/proof/$d/PROOF.md" 2>/dev/null; then
    echo "PROOF ok         : proof/$d/PROOF.md present"
  else
    echo "PROOF MISSING    : proof/$d/PROOF.md ABSENT on $WAVE"
    rc=1
  fi
done
exit $rc
