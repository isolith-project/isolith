#!/usr/bin/env bash
# LICENSE-FILES-AND-BOUNDARY deliberate-failure repro (t_fb3a286b): prove the
# licensing boundary check FAILS on a tree that violates it. Mirrors the
# sbom-cve-fail-repro.sh pattern: the repro is healthy iff the check fails on
# every seeded tree.
#
# Legs (each against a hermetic local clone, offline, no network):
#   A. crate-boundary: a workspace crate whose Cargo.toml declares a licence
#      that disagrees with docs/licensing-boundary.json MUST fail the check.
#   C. dependency allow-list: a dependency that resolves to a licence outside
#      the allow-list (here a seeded GPL-3.0 package, the clean-room bar) MUST
#      fail the check.
#   E. root files: a tree missing LICENSE-AGPL MUST fail the check.
#
# Exit-code contract: 0 iff the check FAILED on every seeded tree (the gate is
# alive); 1 if the check passed on a seeded tree (the gate is broken) or on a
# usage error. Records the proof at docs/proof/licensing-check-fail-repro.json
# and docs/proof/licensing-check-fail-repro.txt.
set -u
DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export DIR
CHECK="$DIR/scripts/licensing-check.py"
WORK="$(mktemp -d /tmp/licfail.XXXXXX)"
trap 'rm -rf "$WORK"' EXIT

# Hermetic clone of the real tree (no network; local object db).
git clone -q --no-local "$DIR" "$WORK/repo" 2>/dev/null || {
  echo "REPRO BROKEN: could not clone repo" >&2; exit 1; }
cd "$WORK/repo" || exit 1

seed_and_run() { # $1=label  $2=seed-script  -> echoes rc
  git checkout -q -b "seed-$1" 2>/dev/null
  bash -c "$2" || { echo "REPRO BROKEN: seed failed for $1" >&2; exit 1; }
  python3 "$CHECK" --repo "$PWD" --out "-" >/dev/null 2>&1
  echo $?
}

# Leg A: crate declared licence flips away from the boundary map.
A_RC=$(seed_and_run "crate-boundary" '
  sed -i "s/^license = \"Apache-2.0\"$/license = \"AGPL-3.0\"/" crates/oep-cgr/Cargo.toml
')
# Leg C: a locked dependency whose licence is outside the allow-list (GPL-3.0).
C_RC=$(seed_and_run "dependency-allowlist" '
  mkdir -p docs
  [ -f docs/dependency-licenses.json ] || cp "$DIR/docs/dependency-licenses.json" docs/dependency-licenses.json
  printf "\n[[package]]\nname = \"isolith-fixture-gpl\"\nversion = \"0.9.9\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\n" >> Cargo.lock
  python3 - <<PY
import json
p = "docs/dependency-licenses.json"
d = json.load(open(p))
d["dependencies"]["isolith-fixture-gpl@0.9.9"] = {"name": "isolith-fixture-gpl", "version": "0.9.9", "license": "GPL-3.0"}
json.dump(d, open(p, "w"), indent=2); open(p, "a").write("\n")
PY
')
# Leg E: root files missing the AGPL text.
E_RC=$(seed_and_run "root-files" 'rm -f LICENSE-AGPL')

echo "crate-boundary   seeded rc=$A_RC (expect 1)"
echo "dependency-allow rc=$C_RC (expect 1)"
echo "root-files       seeded rc=$E_RC (expect 1)"

[ "$A_RC" = "1" ] || { echo "REPRO BROKEN: leg A did not fail" >&2; exit 1; }
[ "$C_RC" = "1" ] || { echo "REPRO BROKEN: leg C did not fail" >&2; exit 1; }
[ "$E_RC" = "1" ] || { echo "REPRO BROKEN: leg E did not fail" >&2; exit 1; }

# Record the proof alongside the green evidence in docs/proof/ of the REAL
# repo (the shell cwd is still the temp clone, so pass $DIR explicitly).
DATE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
mkdir -p "$DIR/docs/proof"
python3 - "$DATE" "$DIR/docs/proof" "$DIR" <<'PY'
import json, sys
date, proof_dir, repo = sys.argv[1], sys.argv[2], sys.argv[3]
b = json.load(open(repo + "/docs/licensing-boundary.json"))
totals = b["license_map_total"]
doc = {
  "repro": "LICENSE-FILES-AND-BOUNDARY deliberate-failure: a tree that "
           "violates the per-crate boundary, the dependency allow-list, or "
           "the root-file set MUST fail scripts/licensing-check.py (exit 1)",
  "timestamp": date,
  "legs": [
    {"id": "crate-boundary", "seed": "crates/oep-cgr/Cargo.toml license flipped Apache-2.0 -> AGPL-3.0", "expected_rc": 1, "gate_alive": True},
    {"id": "dependency-allowlist", "seed": "isolith-fixture-gpl@0.9.9 with license GPL-3.0 injected into Cargo.lock + dependency-licenses.json", "expected_rc": 1, "gate_alive": True},
    {"id": "root-files", "seed": "LICENSE-AGPL removed from the tree", "expected_rc": 1, "gate_alive": True},
  ],
  "boundary_map": {"rows": b["count"], "apache": totals["Apache-2.0"], "agpl": totals["AGPL-3.0"]},
  "result": "PASS (the check failed on every seeded tree; the gate is alive)",
}
open(proof_dir + "/licensing-check-fail-repro.json", "w").write(json.dumps(doc, indent=2) + "\n")
open(proof_dir + "/licensing-check-fail-repro.txt", "w").write(
  "LICENSE-FILES-AND-BOUNDARY deliberate-failure repro\n"
  "timestamp: " + date + "\n\n"
  "Every seeded tree MUST fail scripts/licensing-check.py (exit 1):\n"
  "  crate-boundary     : oep-cgr declared AGPL-3.0 vs map Apache-2.0\n"
  "  dependency-allowlist: GPL-3.0 dependency injected into the lock\n"
  "  root-files         : LICENSE-AGPL removed\n"
  "Result: PASS - the gate failed on every seeded tree.\n")
PY

echo "repro proof recorded: docs/proof/licensing-check-fail-repro.{json,txt}"
exit 0
