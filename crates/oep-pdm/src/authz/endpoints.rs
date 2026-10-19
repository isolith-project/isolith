//! Endpoint to RBAC-action mapping (P11-T03).
//!
//! The single, explicit place where the REST v1 wire surface meets the
//! P8-M3-T02 action space. Every contract route is mapped here by
//! (method, path); anything not mapped is `Unknown` and the gate returns a
//! uniform `404` (never an existence oracle). The acceptance suite in
//! `tests/auth_boundary.rs` and the cross-check in `P11/auth-boundary.md` pin
//! this table to the routes `service::build_router` actually serves.

use crate::rbac::Action;

/// Base path of the v1 contract.
pub const API_PREFIX: &str = "/api/v1";

/// A classified request target: the action required plus the vault object the
/// request addresses (when object-level), so the gate can tenant-scope it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The RBAC action the caller must hold for this request.
    pub action: Action,
    /// The target vault item id, when the endpoint names one. `None` for
    /// collection- and system-level endpoints (no object to tenant-scope).
    pub item_id: Option<String>,
}

/// The result of classifying one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classified {
    /// No authorization required (health/liveness only).
    Open,
    /// Not part of the mapped contract surface: respond 404, no oracle.
    Unknown,
    /// A protected endpoint requiring `Target`.
    Protected(Target),
}

/// Build a `Protected(Target)` value.
fn target(action: Action, item_id: Option<&str>) -> Classified {
    Classified::Protected(Target {
        action,
        item_id: item_id.map(str::to_string),
    })
}

/// Strip the `/api/v1` base; return the request path below it.
pub fn strip_prefix(path: &str) -> &str {
    path.strip_prefix(API_PREFIX).unwrap_or(path)
}

/// Split a path into its non-empty segments.
fn segs(path: &str) -> Vec<&str> {
    path.split('/').filter(|s| !s.is_empty()).collect()
}

/// Read a string field from a JSON object body (best-effort; `None` on any
/// parse failure, which the conservative mapping treats as edit-scope).
fn body_string(body: Option<&[u8]>, key: &str) -> Option<String> {
    let bytes = body?;
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    value.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// The RBAC action a lifecycle transition requires, derived from the target
/// state in the JSON body (`{"to": "..."}`, P8-M6-T03 surface).
fn transition_action(to: Option<&str>) -> Action {
    match to {
        Some("under_review") => Action::SubmitForReview,
        Some("released") => Action::Release,
        Some("obsolete") => Action::MakeObsolete,
        Some("working") => Action::Review,
        // Unparseable/missing body: conservative edit scope. A real release
        // only ever follows a parseable `"to":"released"`, which maps to
        // Release, so a malformed body can never grant a release.
        _ => Action::EditRevision,
    }
}

/// Does this route's decision depend on the request body? The gate buffers
/// the body (bounded, `gate::GATE_BODY_CAP`) only for these routes, then
/// rebuilds the request for the handler.
pub fn needs_body(method: &str, path: &str) -> bool {
    let m = method.to_ascii_uppercase();
    let seg = segs(strip_prefix(path));
    matches!(
        (m.as_str(), seg.as_slice()),
        ("POST", ["items", _, "revisions", _, "transition"]) | ("POST", ["sync", "push"])
    )
}

/// Classify a request into its required RBAC target.
///
/// * `body` is the raw request body bytes ONLY for routes that parse it for
///   the decision (`needs_body`); pass `None` elsewhere.
pub fn classify(method: &str, path: &str, body: Option<&[u8]>) -> Classified {
    let m = method.to_ascii_uppercase();
    let seg = segs(strip_prefix(path));

    match (m.as_str(), seg.as_slice()) {
        // Liveness handshake is open (contract: no auth on /health).
        ("GET", ["health"]) => Classified::Open,

        // Items.
        ("GET", ["items"]) => target(Action::Read, None),
        ("POST", ["items"]) => target(Action::CreateItem, None),
        ("GET", ["items", id]) => target(Action::Read, Some(id)),

        // Files on an item.
        ("GET", ["items", id, "file", "latest"]) => target(Action::Read, Some(id)),

        // Revisions.
        ("GET", ["items", id, "revisions"]) => target(Action::Read, Some(id)),
        ("POST", ["items", id, "revisions"]) => target(Action::EditRevision, Some(id)),
        ("GET", ["items", id, "revisions", _]) => target(Action::Read, Some(id)),
        ("GET", ["items", id, "revisions", _, "file"]) => target(Action::Read, Some(id)),

        // Working-set locks (P8-M6-T03 gate surface).
        ("POST", ["items", id, "lock"]) => target(Action::EditRevision, Some(id)),
        ("DELETE", ["items", id, "lock"]) => target(Action::EditRevision, Some(id)),

        // Lifecycle transition: action by target state (see transition_action).
        ("POST", ["items", id, "revisions", _, "transition"]) => target(
            transition_action(body_string(body, "to").as_deref()),
            Some(id),
        ),

        // Audit log: system-management visibility (see P11/auth-boundary.md,
        // decision D3: the audit trail records every principal's reads and
        // changes, so reading it is a ManageSystem action, not a Read).
        ("GET", ["audit"]) => target(Action::ManageSystem, None),
        ("GET", ["audit", "verify"]) => target(Action::ManageSystem, None),

        // Offline sync surface (P8-M5-T02); sync_pull's target item comes
        // from the body (`item_id`).
        ("POST", ["sync", "push"]) => {
            let item_id = body_string(body, "item_id");
            let item_id = item_id.filter(|s| !s.is_empty());
            target(Action::EditRevision, item_id.as_deref())
        }
        ("GET", ["sync", "conflicts", _]) => target(Action::Read, None),
        ("GET", ["sync", "conflicts", _, "file", _]) => target(Action::Read, None),
        ("POST", ["sync", "conflicts", _, "resolve"]) => target(Action::EditRevision, None),

        // 3-way-merge UI pages (walk the same actions as their API twins).
        ("GET", ["ui", "sync", "conflicts", _]) => target(Action::Read, None),
        ("POST", ["ui", "sync", "conflicts", _, "resolve"]) => target(Action::EditRevision, None),

        _ => Classified::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action_of(method: &str, path: &str, body: Option<&[u8]>) -> Option<Action> {
        match classify(method, path, body) {
            Classified::Protected(t) => Some(t.action),
            _ => None,
        }
    }

    fn item_of(method: &str, path: &str) -> Option<String> {
        match classify(method, path, None) {
            Classified::Protected(t) => t.item_id,
            _ => None,
        }
    }

    #[test]
    fn reads_map_to_read_for_items_revisions_and_files() {
        for (path, want_item) in [
            ("/api/v1/items", false),
            ("/api/v1/items/BOX-1", true),
            ("/api/v1/items/BOX-1/file/latest", true),
            ("/api/v1/items/BOX-1/revisions", true),
            ("/api/v1/items/BOX-1/revisions/3", true),
            ("/api/v1/items/BOX-1/revisions/3/file", true),
        ] {
            assert_eq!(action_of("GET", path, None), Some(Action::Read), "{path}");
            if want_item {
                assert_eq!(item_of("GET", path).as_deref(), Some("BOX-1"), "{path}");
            }
        }
    }

    #[test]
    fn writes_map_to_create_and_edit() {
        assert_eq!(
            action_of("POST", "/api/v1/items", None),
            Some(Action::CreateItem)
        );
        assert_eq!(
            action_of("POST", "/api/v1/items/BOX-1/revisions", None),
            Some(Action::EditRevision)
        );
        assert_eq!(
            action_of("GET", "/api/v1/items/BOX-1/revisions/3", None),
            Some(Action::Read)
        );
    }

    #[test]
    fn transition_action_follows_target_state() {
        let v = |to: &str| format!("{{\"to\":\"{to}\"}}").into_bytes();
        assert_eq!(
            action_of(
                "POST",
                "/api/v1/items/BOX-1/revisions/3/transition",
                Some(&v("under_review"))
            ),
            Some(Action::SubmitForReview)
        );
        assert_eq!(
            action_of(
                "POST",
                "/api/v1/items/BOX-1/revisions/3/transition",
                Some(&v("released"))
            ),
            Some(Action::Release)
        );
        assert_eq!(
            action_of(
                "POST",
                "/api/v1/items/BOX-1/revisions/3/transition",
                Some(&v("obsolete"))
            ),
            Some(Action::MakeObsolete)
        );
        assert_eq!(
            action_of(
                "POST",
                "/api/v1/items/BOX-1/revisions/3/transition",
                Some(&v("working"))
            ),
            Some(Action::Review)
        );
        // A release only grants with a parseable body.
        assert_eq!(
            action_of("POST", "/api/v1/items/BOX-1/revisions/3/transition", None),
            Some(Action::EditRevision)
        );
    }

    #[test]
    fn audit_is_system_management() {
        assert_eq!(
            action_of("GET", "/api/v1/audit", None),
            Some(Action::ManageSystem)
        );
        assert_eq!(
            action_of("GET", "/api/v1/audit/verify", None),
            Some(Action::ManageSystem)
        );
    }

    #[test]
    fn health_is_open_and_unknown_is_unknown() {
        assert!(matches!(
            classify("GET", "/api/v1/health", None),
            Classified::Open
        ));
        assert!(matches!(
            classify("GET", "/api/v1/nonexistent", None),
            Classified::Unknown
        ));
        // Any verb on a real route is mapped; non-contract verbs are not.
        assert!(matches!(
            classify("PATCH", "/api/v1/items", None),
            Classified::Unknown
        ));
    }

    #[test]
    fn needs_body_only_for_transition_and_sync_push() {
        assert!(needs_body(
            "POST",
            "/api/v1/items/BOX-1/revisions/3/transition"
        ));
        assert!(needs_body("POST", "/api/v1/sync/push"));
        assert!(!needs_body("POST", "/api/v1/items"));
        assert!(!needs_body("POST", "/api/v1/items/BOX-1/revisions"));
        assert!(!needs_body("GET", "/api/v1/items/BOX-1/revisions/3"));
    }
}
