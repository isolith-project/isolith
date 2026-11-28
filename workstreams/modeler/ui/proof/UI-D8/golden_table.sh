#!/usr/bin/env bash
# UI-D8 golden table: expected = sha256 of the file as UI-D7 committed it
# (HEAD), actual = sha256 of the file as re-rendered by the current tree.
# Prints name / expected / actual / PASS|FAIL. All must be PASS.
set -u
cd "$(git rev-parse --show-toplevel)" || exit 1

pass=0
fail=0
for f in \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Front-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Front-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Back-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Back-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Left-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Left-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Right-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Right-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Top-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Top-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Bottom-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Bottom-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Iso-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-Iso-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-all-culled-cpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D7/ui-d7-all-culled-gpu-400x300.png \
  workstreams/modeler/ui/proof/UI-D2/ui-d2-shaded.png \
  workstreams/modeler/ui/proof/UI-D2/ui-d2-edges.png \
  workstreams/modeler/ui/proof/UI-D2/ui-d2-flat.png \
  workstreams/modeler/ui/proof/UI-D2/ui-d2-smooth.png \
  workstreams/modeler/ui/proof/UI-D2/ui-d2-transparent-50.png \
  ; do
  expected=$(git show HEAD:"$f" | sha256sum | cut -d' ' -f1)
  actual=$(sha256sum "$f" | cut -d' ' -f1)
  if [ "$expected" = "$actual" ]; then
    printf "PASS  %-52s %s\n" "$(basename "$f")" "$actual"
    pass=$((pass+1))
  else
    printf "FAIL  %-52s expected=%s actual=%s\n" "$(basename "$f")" "$expected" "$actual"
    fail=$((fail+1))
  fi
done
echo "---"
echo "goldens checked=$((pass+fail)) pass=$pass fail=$fail"
[ "$fail" -eq 0 ]
