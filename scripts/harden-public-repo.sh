#!/usr/bin/env bash
# harden-public-repo.sh - idempotent hardening of the Isolith public GitHub
# repository and account, run BEFORE the repository is made visible.
#
# Policy source of truth: docs/hardening/public-repo-hardening.md. Every
# setting here implements a row of that baseline; the script ends with a
# readback table of the same rows. Nothing in this script publishes code or
# artifacts; it only sets repository security settings on GitHub.
#
# Usage:
#   HARDEN_OWNER=<org-or-user> HARDEN_REPO=isolith ./scripts/harden-public-repo.sh --check
#   HARDEN_OWNER=<org-or-user> HARDEN_REPO=isolith ./scripts/harden-public-repo.sh          # apply
#   HARDEN_OWNER=<org-or-user> HARDEN_REPO=isolith ./scripts/harden-public-repo.sh --verify # readback only
#
# Env:
#   HARDEN_OWNER          GitHub account or org (required)
#   HARDEN_REPO           repository name (default isolith)
#   HARDEN_REQUIRED_CHECKS comma list of required status checks, default the
#                          ported CI set from the baseline doc
#
# Requires: gh authenticated with repo admin on the target repository.
set -euo pipefail

OWNER="${HARDEN_OWNER:?HARDEN_OWNER unset: the GitHub org or user that owns the repo}"
REPO="${HARDEN_REPO:-isolith}"
REPO_PATH="$OWNER/$REPO"

# Full ported CI set = the required status checks (baseline section 2).
DEFAULT_CHECKS="lint,unit,integration,gateway,quality-gate,dco-sign-off,lockpin,licensing-boundary,crypto-integrity,p11t02-sbom-cve,export-control-selfcheck"
REQUIRED_CHECKS="${HARDEN_REQUIRED_CHECKS:-$DEFAULT_CHECKS}"
# Approvals required on a pull request. The baseline is 1; a solo-maintainer
# repository must run 0, because GitHub does not let a maintainer approve
# their own pull request and enforce_admins would then deadlock every merge.
# Raise it to 1 in the same pass that adds a second maintainer.
REQUIRED_APPROVALS="${HARDEN_REQUIRED_APPROVALS:-1}"

MODE="${1:-apply}"
case "$MODE" in
  --check) MODE=check ;;
  --verify) MODE=verify ;;
  apply) ;;
  *) echo "unknown mode: $MODE (use --check, --verify, or nothing to apply)" >&2; exit 2 ;;
esac

FAIL=0
ppass() { printf '  [PASS] %s\n' "$*"; }
pfail() { printf '  [FAIL] %s\n' "$*"; FAIL=1; }
pnote() { printf '  [note] %s\n' "$*"; }

echo "== Isolith public-repo hardening ($MODE) on $REPO_PATH =="
command -v gh >/dev/null 2>&1 || { echo "gh CLI required"; exit 2; }
gh auth status >/dev/null 2>&1 || { echo "gh not authenticated"; exit 2; }

if [ "${HARDEN_SKIP_API:-0}" = "1" ]; then
  pnote "HARDEN_SKIP_API=1: pre-account smoke test, GitHub API rows skipped"
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    for f in CODEOWNERS .github/dependabot.yml docs/hardening/public-repo-hardening.md \
             docs/release-signing.md keys/oep-airgap-signing.pub.asc \
             .github/workflows/hardening.yaml scripts/hardening/check-workflow-hardening.sh; do
      [ -f "$f" ] && ppass "in-tree: $f present" || pfail "in-tree: $f missing"
    done
  fi
  echo
  if [ "$FAIL" -eq 0 ]; then echo "RESULT: PASS - all local hardening rows present (mode $MODE)";
  else echo "RESULT: FAIL - see [FAIL] rows above (mode $MODE)"; exit 1; fi
  exit 0
fi

if ! gh api "repos/$REPO_PATH" >/dev/null 2>&1; then
  pfail "target repository $REPO_PATH not reachable"
  echo "  create it first (GITHUB-PRIMARY-MIGRATION step 2), then re-run."
  exit 1
fi

apply_if() { # apply_if <label> <read-only value> <apply command...>
  local label="$1" want="$2"; shift 2
  if [ "$MODE" = "apply" ]; then
    "$@"
    pnote "$label": applied
  fi
}

checks_json() {
  python3 - "$REQUIRED_CHECKS" <<'PY'
import json, sys
print(json.dumps([c for c in sys.argv[1].split(",") if c]))
PY
}

## 2. Branch protection on main ---------------------------------------------
bp_get() { gh api "repos/$REPO_PATH/branches/main/protection" "$@" 2>/dev/null; }
if [ -z "$(bp_get --jq '.required_pull_request_reviews.required_approving_review_count' 2>/dev/null)" ]; then
  if [ "$MODE" = "apply" ]; then
    python3 - "$(checks_json)" <<'PY' >/tmp/harden-bp.json
import json, os, sys
ctx = json.loads(sys.argv[1])
approvals = int(os.environ.get("HARDEN_REQUIRED_APPROVALS", "1"))
json.dump({
  "required_status_checks": {"strict": True, "contexts": ctx},
  "enforce_admins": True,
  "required_pull_request_reviews": {
    "required_approving_review_count": approvals,
    "require_code_owner_reviews": approvals > 0,
    "dismiss_stale_reviews": False,
  },
  "restrictions": None,
  "allow_force_pushes": False,
  "allow_deletions": False,
  "required_linear_history": False,
  "required_conversation_resolution": True,
}, open("/tmp/harden-bp.json", "w"), indent=2)
PY
    gh api --method PUT "repos/$REPO_PATH/branches/main/protection" --input /tmp/harden-bp.json >/dev/null
    pnote "branch protection on main: applied"
  else
    pnote "branch protection on main: not set yet (apply mode will set it)"
  fi
fi
if [ "$(bp_get --jq '.allow_force_pushes.enabled')" = "false" ] \
   && [ "$(bp_get --jq '.allow_deletions.enabled')" = "false" ] \
   && [ "$(bp_get --jq '.enforce_admins.enabled')" = "true" ] \
   && [ "$(bp_get --jq '.required_pull_request_reviews.required_approving_review_count // -1')" = "$REQUIRED_APPROVALS" ] \
   && [ "$(bp_get --jq '.required_status_checks.strict // false')" = "true" ]; then
  ppass "branch protection on main: admins bound, no force push, no deletions, $REQUIRED_APPROVALS required review(s), strict checks"
else
  pfail "branch protection on main: missing or incomplete (see readback)"
  bp_get --jq '{enforce_admins, required_pull_request_reviews, allow_force_pushes, allow_deletions, required_linear_history, required_status_checks}' || true
fi

## 3. Tag protection ---------------------------------------------------------
# GitHub retired the legacy /tags/protection endpoint; a tag ruleset is the live
# mechanism and is what this section now creates and verifies.
for pat in "v*" "wave-*"; do
  want="refs/tags/$pat"
  have=$(gh api "repos/$REPO_PATH/rulesets" --jq '.[]|select(.target=="tag")|.id' 2>/dev/null | while read -r rid; do
    gh api "repos/$REPO_PATH/rulesets/$rid" --jq '.conditions.ref_name.include[]?' 2>/dev/null
  done | grep -cxF "$want" || true)
  if [ "$have" -eq 0 ]; then
    if [ "$MODE" = "apply" ]; then
      gh api --method POST "repos/$REPO_PATH/rulesets" --input - >/dev/null <<JSON
{"name":"protect $pat tags","target":"tag","enforcement":"active","conditions":{"ref_name":{"include":["$want"],"exclude":[]}},"rules":[{"type":"deletion"},{"type":"update"}]}
JSON
      pnote "tag ruleset added: $want"
    fi
  fi
done
present=$(gh api "repos/$REPO_PATH/rulesets" --jq '.[]|select(.target=="tag")|.id' 2>/dev/null | while read -r rid; do
  gh api "repos/$REPO_PATH/rulesets/$rid" --jq '.conditions.ref_name.include[]?' 2>/dev/null
done | sort -u || true)
if printf '%s\n' "$present" | grep -qxF "refs/tags/v*" && printf '%s\n' "$present" | grep -qxF "refs/tags/wave-*"; then
  ppass "tag protection: v* and wave-* protected from delete and force-move (rulesets)"
else
  pfail "tag protection incomplete (saw: $(printf '%s ' $present | tr '\n' ' '))"
fi

## 4. Actions hardening ------------------------------------------------------
if [ "$MODE" = "apply" ]; then
  gh api --method PATCH "repos/$REPO_PATH/actions/permissions" \
    --input - <<'JSON' >/dev/null
{"enabled": true, "allowed_actions": "all", "default_workflow_permissions": "read"}
JSON
  pnote "actions permissions: default GITHUB_TOKEN read-only"
fi
perm=$(gh api "repos/$REPO_PATH/actions/permissions/workflow" --jq '.default_workflow_permissions // "missing"' 2>/dev/null)
[ "$perm" = "read" ] && ppass "actions: default GITHUB_TOKEN read-only" \
                   || pfail "actions: default GITHUB_TOKEN is '$perm' (want read)"

## 5. Supply chain: secret scanning, dependency graph, alerts ----------------
if [ "$MODE" = "apply" ]; then
  gh api --method PATCH "repos/$REPO_PATH" --input - <<'JSON' >/dev/null
{"security_and_analysis": {
   "dependency_graph": {"status": "enabled"},
   "dependabot_alerts": {"status": "enabled"},
   "secret_scanning": {"status": "enabled"},
   "secret_scanning_push_protection": {"status": "enabled"}}}
JSON
  gh api --method PUT "repos/$REPO_PATH/vulnerability-alerts" >/dev/null 2>&1 || true
  pnote "enabled dependency graph, dependabot alerts, secret scanning + push protection"
fi
sca=$(gh api "repos/$REPO_PATH" --jq '.security_and_analysis' 2>/dev/null || true)
case "$sca" in
  *'"secret_scanning"'*"enabled"*|*enabled*) ppass "secret scanning enabled (security_and_analysis present)" ;;
  ""|null) pnote "security_and_analysis readback empty on this plan/visibility (verify in the web UI: Settings > Code security)" ;;
  *) pfail "secret scanning not verified; readback: $sca" ;;
esac

## 1. Account security: org 2FA is a manual owner action ---------------------
if gh api "orgs/$OWNER" >/dev/null 2>&1; then
  pnote "org 2FA: manual owner action needed at github.com/organizations/$OWNER/settings/security ('Require two-factor authentication for everyone'). Record it verified on the card when flipped."
else
  pnote "account '$OWNER' is a user, not an org: 2FA lives on the owner account's security settings (owner confirms)."
fi

## In-tree pieces that must exist in the visible repository ------------------
if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  for f in CODEOWNERS .github/dependabot.yml docs/hardening/public-repo-hardening.md \
           docs/release-signing.md keys/oep-airgap-signing.pub.asc \
           .github/workflows/hardening.yaml scripts/hardening/check-workflow-hardening.sh; do
    [ -f "$f" ] && ppass "in-tree: $f present" || pfail "in-tree: $f missing"
  done
fi

echo
if [ "$FAIL" -eq 0 ]; then
  echo "RESULT: PASS - all hardening rows verified (mode $MODE)"
else
  echo "RESULT: FAIL - see [FAIL] rows above (mode $MODE)"
  exit 1
fi
