//! P8-M5-T02 acceptance: offline working set, sync-back on reconnect,
//! conflicting edits surface a 3-way-merge UI, resolution verified by hash.
//!
//! RED-first acceptance contract (TDD): these tests drive a real oep-pdm
//! service over real sockets (`StubServer`, which IS the axum service) and the
//! client-side `OfflineWorkingSet` + `SyncEngine`, with no in-process shortcuts
//! and no mock transport. Hash verification is the acceptance:
//!
//!   1. an edit recorded locally while the client has no server contact
//!      syncs back on reconnect as the item's next revision, and the server's
//!      committed blob sha-256 equals the locally computed digest;
//!   2. divergent edits (someone else advanced the head while we were offline)
//!      surface a conflict with base/ours/theirs snapshots, rendered by GET
//!      /ui/sync/conflicts/{id} as a 3-way merge page;
//!   3. resolving the conflict commits a new revision whose blob sha-256 equals
//!      a locally computed digest of the resolved bytes (hash after commit).

use std::path::PathBuf;

use oep_pdm::api::{CreateItemRequest, SyncPushRequest, SyncPushResult, SyncResolveRequest};
use oep_pdm::client::PdmClient;
use oep_pdm::store::VaultStore;
use oep_pdm::stub::StubServer;
use oep_pdm::sync::{OfflineWorkingSet, SyncEngine};
use sha2::{Digest, Sha256};

use base64::Engine as _;

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn upload(bytes: &[u8], name: &str) -> oep_pdm::api::FileUpload {
    use base64::Engine;
    oep_pdm::api::FileUpload {
        filename: name.to_string(),
        format: "step".to_string(),
        content_type: Some("application/octet-stream".to_string()),
        content: base64::engine::general_purpose::STANDARD.encode(bytes),
    }
}

/// Create an item and its first revision; returns (item_id, base_revision, base_hash).
fn seed_base(server: &StubServer, name: &str, bytes: &[u8]) -> (String, String, String) {
    let c = PdmClient::new(&server.base_url()).unwrap();
    let item = c
        .create_item(&CreateItemRequest {
            name: name.to_string(),
            kind: "part".to_string(),
            description: Some("P8-M5-T02 fixture".to_string()),
            ..CreateItemRequest::default_min()
        })
        .expect("create item");
    let r1 = c
        .create_revision(
            &item.id,
            &oep_pdm::api::CreateRevisionRequest {
                state: None,
                comment: Some("base".to_string()),
                created_by: None,
                file: upload(bytes, "box-mini.step"),
            },
        )
        .expect("seed base revision");
    (
        item.id.clone(),
        r1.number.clone(),
        r1.files[0].blob_sha256.clone(),
    )
}

fn tmpws(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("oep-pdm-accept-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    p
}

// ---------------- Acceptance 1: offline edit syncs back ------------------

#[test]
fn accept_offline_edit_syncs_back_on_reconnect() {
    // The item + base revision live on the service. The edit is recorded into a
    // LOCAL working set with no server contact ("offline"), then, once the same
    // service is reachable again, a SyncEngine replays it.
    let server = StubServer::start("127.0.0.1:0").unwrap();
    let (item_id, base_rev, base_hash) = seed_base(
        &server,
        "OfflinePart",
        b"base geometry\nline two\nline three\n",
    );

    // Offline: record an edit with base identity, before any server sync.
    let ws_dir = tmpws("sync-back");
    let mut ws = OfflineWorkingSet::open(&ws_dir).unwrap();
    let offline_bytes = b"base geometry\nline two\nNEW line after two\nline three\n";
    let edit_id = ws
        .record_edit(
            &item_id,
            &base_rev,
            &base_hash,
            "box-mini.step",
            "step",
            None,
            Some("working".to_string()),
            Some("offline change".to_string()),
            Some("offline-user".to_string()),
            offline_bytes,
        )
        .expect("record offline edit");
    assert_eq!(ws.pending().count(), 1, "edit recorded offline and pending");

    // Reconnect: replay the pending edit through the service.
    let client = PdmClient::new(&server.base_url()).unwrap();
    let mut engine = SyncEngine {
        client: &client,
        working_set: &mut ws,
    };
    let report = engine.sync_pending().expect("sync replays cleanly");
    assert!(!report.offline, "service reachable on reconnect");
    assert_eq!(
        report.synced.len(),
        1,
        "offline edit accepted as next revision"
    );

    // Verify the accepted revision's content is our offline bytes, byte-exact.
    let list = client.list_revisions(&item_id).unwrap();
    assert_eq!(list.revisions.len(), 2, "base + the offline edit");
    let committed = client.open_latest_file(&item_id).unwrap();
    assert_eq!(
        committed.bytes, offline_bytes,
        "offline edit returned on reconnect"
    );
    assert_eq!(
        committed.blob_sha256,
        hex_sha256(offline_bytes),
        "server commits the blob under the locally computed hash"
    );
    // Local working set reflects the accepted state.
    let edit = ws.get(&edit_id).unwrap();
    match &edit.status {
        oep_pdm::sync::EditStatus::Synced { revision } => {
            assert_eq!(revision, "2", "server assigned the next revision number")
        }
        other => panic!("expected Synced, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&ws_dir);
}

// ---------------- Acceptance 2: divergent edits surface the 3-way UI -------

#[test]
fn accept_divergent_edits_surface_three_way_merge_ui() {
    let server = StubServer::start("127.0.0.1:0").unwrap();
    let base_bytes = b"STEP header\nDIMENSION A = 10\nHOLE B = 5\n";
    let (item_id, base_rev, base_hash) = seed_base(&server, "DivergentPart", base_bytes);

    // "Theirs": a second, ONLINE editor advances the head while we are offline.
    let online = PdmClient::new(&server.base_url()).unwrap();
    let theirs_bytes = b"STEP header\nDIMENSION A = 10\nHOLE B = 8\n"; // they increased the hole
    online
        .create_revision(
            &item_id,
            &oep_pdm::api::CreateRevisionRequest {
                state: Some("released".to_string()),
                comment: Some("theirs: bigger hole".to_string()),
                created_by: Some("other-user".to_string()),
                file: upload(theirs_bytes, "box-mini.step"),
            },
        )
        .expect("theirs commits on the server");

    // "Ours": we recorded an offline edit against the OLD base (rev 1).
    let ours_bytes = b"STEP header\nDIMENSION A = 12\nHOLE B = 5\n"; // we enlarged A
    let ws_dir = tmpws("threeway");
    let mut ws = OfflineWorkingSet::open(&ws_dir).unwrap();
    ws.record_edit(
        &item_id,
        &base_rev,
        &base_hash,
        "box-mini.step",
        "step",
        None,
        None,
        None,
        Some("offline-user".to_string()),
        ours_bytes,
    )
    .unwrap();

    // Reconnect + sync -> divergence is detected, edit is held pending.
    let client = PdmClient::new(&server.base_url()).unwrap();
    let sync_id;
    {
        let mut engine = SyncEngine {
            client: &client,
            working_set: &mut ws,
        };
        let report = engine.sync_pending().unwrap();
        assert_eq!(
            report.conflicted.len(),
            1,
            "divergent edit surfaces a conflict"
        );
        assert!(
            report.synced.is_empty(),
            "no silent overlay of someone else's head"
        );
        sync_id = report.conflicted[0].1.clone();
    }

    // The conflict records exactly our base/ours/theirs snapshots + hashes.
    let view = client.get_sync_conflict(&sync_id).unwrap();
    assert_eq!(view.item_id, item_id);
    assert_eq!(view.base.revision.as_deref(), Some(base_rev.as_str()));
    assert_eq!(view.base.blob_sha256, base_hash);
    assert_eq!(view.base.blob_sha256, hex_sha256(base_bytes));
    assert_eq!(
        view.ours.blob_sha256,
        hex_sha256(ours_bytes),
        "ours is the offline edit"
    );
    assert_eq!(view.ours.revision, None, "ours has no server revision yet");
    assert_eq!(view.theirs.revision.as_deref(), Some("2"));
    assert_eq!(
        view.theirs.blob_sha256,
        hex_sha256(theirs_bytes),
        "theirs is the head"
    );

    // The 3-way merge UI page renders base/ours/theirs with the hashes.
    let ui = fetch_ui_page(&server, &sync_id);
    assert!(ui.contains("3-way merge"), "UI page identifies the merge");
    assert!(ui.contains("BASE"), "UI surfaces BASE");
    assert!(ui.contains("OURS"), "UI surfaces OURS");
    assert!(ui.contains("THEIRS"), "UI surfaces THEIRS");
    assert!(ui.contains(&hex_sha256(base_bytes)));
    assert!(ui.contains(&hex_sha256(ours_bytes)));
    assert!(ui.contains(&hex_sha256(theirs_bytes)));
    // Resolving via the form action is present.
    assert!(ui.contains(&format!("/ui/sync/conflicts/{sync_id}/resolve")));
    assert!(ui.contains("value=\"ours\""));
    assert!(ui.contains("value=\"theirs\""));
    let _ = std::fs::remove_dir_all(&ws_dir);
}

// ---------------- Acceptance 3: resolution verified by hash ---------------

#[test]
fn accept_resolution_outcome_verified_by_hash_after_commit() {
    let server = StubServer::start("127.0.0.1:0").unwrap();
    let base_bytes = b"CRITICAL CONSTANT = 42\ncomputed = f(x)\n";
    let (item_id, base_rev, base_hash) = seed_base(&server, "ResolvePart", base_bytes);

    // theirs advances the head.
    let online = PdmClient::new(&server.base_url()).unwrap();
    let theirs_bytes = b"CRITICAL CONSTANT = 43\ncomputed = g(y)\n";
    online
        .create_revision(
            &item_id,
            &oep_pdm::api::CreateRevisionRequest {
                state: Some("released".to_string()),
                comment: None,
                created_by: None,
                file: upload(theirs_bytes, "box-mini.step"),
            },
        )
        .unwrap();

    // ours is an offline edit on the base.
    let ours_bytes = b"CRITICAL CONSTANT = 44\ncomputed = h(z)\n";
    let ws_dir = tmpws("resolve");
    let mut ws = OfflineWorkingSet::open(&ws_dir).unwrap();
    ws.record_edit(
        &item_id,
        &base_rev,
        &base_hash,
        "box-mini.step",
        "step",
        None,
        None,
        None,
        None,
        ours_bytes,
    )
    .unwrap();

    let client = PdmClient::new(&server.base_url()).unwrap();
    let sync_id;
    {
        let mut engine = SyncEngine {
            client: &client,
            working_set: &mut ws,
        };
        let report = engine.sync_pending().unwrap();
        sync_id = report.conflicted[0].1.clone();
    }

    // Resolve by MERGED document (a real third snapshot the client builds).
    let merged_bytes = b"CRITICAL CONSTANT = 44\ncomputed = g(y) * (f(x) + 1)\n";
    let revision = client
        .resolve_conflict(
            &sync_id,
            &SyncResolveRequest {
                choice: "merged".to_string(),
                merged: Some(upload(merged_bytes, "box-mini.step")),
            },
        )
        .expect("resolution commits");

    // Acceptance: resolution outcome verified by hash AFTER commit.
    assert_eq!(
        revision.files[0].blob_sha256,
        hex_sha256(merged_bytes),
        "committed blob hash equals the hash of the resolved bytes"
    );
    assert_eq!(
        revision.number, "3",
        "resolution is the next revision after base + theirs"
    );

    // Server-side truth: latest file bytes + hash match the resolution.
    let latest = client.open_latest_file(&item_id).unwrap();
    assert_eq!(latest.bytes, merged_bytes);
    assert_eq!(latest.blob_sha256, hex_sha256(merged_bytes));

    // The resolved conflict is gone (404), and no orphan conflict remains.
    match client.get_sync_conflict(&sync_id) {
        Err(oep_pdm::client::PdmError::Http { status: 404, .. }) => {}
        other => panic!("expected 404 for resolved conflict, got {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&ws_dir);
}

// ---------------- Direct store-level sync contract -----------------------

#[test]
fn accept_sync_push_clean_vs_conflict_at_store() {
    let mut store = VaultStore::new();
    let item = store
        .create_item(&CreateItemRequest {
            name: "StoreSync".to_string(),
            kind: "part".to_string(),
            ..CreateItemRequest::default_min()
        })
        .unwrap();
    let base = b"base content\n";
    let r1 = store
        .create_revision(
            &item.id,
            &oep_pdm::api::CreateRevisionRequest {
                state: None,
                comment: None,
                created_by: None,
                file: oep_pdm::api::FileUpload {
                    filename: "a".to_string(),
                    format: "step".to_string(),
                    content_type: None,
                    content: base64::engine::general_purpose::STANDARD.encode(base),
                },
            },
        )
        .unwrap();
    let base_hash = r1.files[0].blob_sha256.clone();

    // Clean push matching the head -> Synced next revision.
    let ours = b"base content\nadded offline\n";
    let clean = store
        .sync_push(&SyncPushRequest {
            item_id: item.id.clone(),
            base_revision: r1.number.clone(),
            base_blob_sha256: base_hash.clone(),
            state: None,
            comment: None,
            created_by: None,
            file: oep_pdm::api::FileUpload {
                filename: "a".to_string(),
                format: "step".to_string(),
                content_type: None,
                content: base64::engine::general_purpose::STANDARD.encode(ours),
            },
        })
        .unwrap();
    match clean {
        SyncPushResult::Synced { revision } => {
            assert_eq!(revision.number, "2");
            assert_eq!(revision.files[0].blob_sha256, hex_sha256(ours));
        }
        SyncPushResult::Conflict { .. } => panic!("expected clean sync"),
    }

    // Divergent push (base is now stale) -> Conflict, no revision created.
    let theirs = b"someone else went first\n";
    store
        .create_revision(
            &item.id,
            &oep_pdm::api::CreateRevisionRequest {
                state: None,
                comment: None,
                created_by: None,
                file: oep_pdm::api::FileUpload {
                    filename: "a".to_string(),
                    format: "step".to_string(),
                    content_type: None,
                    content: base64::engine::general_purpose::STANDARD.encode(theirs),
                },
            },
        )
        .unwrap();
    let div = store
        .sync_push(&SyncPushRequest {
            item_id: item.id.clone(),
            base_revision: "2".to_string(),
            base_blob_sha256: hex_sha256(ours),
            state: None,
            comment: None,
            created_by: None,
            file: oep_pdm::api::FileUpload {
                filename: "a".to_string(),
                format: "step".to_string(),
                content_type: None,
                content: base64::engine::general_purpose::STANDARD.encode(b"late edit"),
            },
        })
        .unwrap();
    match div {
        SyncPushResult::Conflict {
            base_revision,
            theirs_revision,
            theirs_blob_sha256,
            ..
        } => {
            assert_eq!(base_revision, "2");
            assert_eq!(theirs_revision, "3");
            assert_eq!(theirs_blob_sha256, hex_sha256(theirs));
        }
        SyncPushResult::Synced { .. } => panic!("expected conflict"),
    }
    // The late edit was NOT committed.
    let list = store.list_revisions(&item.id).unwrap();
    assert_eq!(list.revisions.len(), 3);
}

// ---------------- helpers -----------------------------------------------

/// Raw HTTP GET of an HTML UI page (the client is JSON-typed; the UI page is
/// plain HTML, so fetch it over a raw socket like the browser would).
fn fetch_ui_page(server: &StubServer, path_segment: &str) -> String {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let base_url = server.base_url();
    let host = base_url
        .trim_start_matches("http://")
        .trim_end_matches("/api/v1");
    let path = format!("/ui/sync/conflicts/{path_segment}");
    let mut s = TcpStream::connect(host).expect("connect");
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    let text = String::from_utf8_lossy(&buf).into_owned();
    match text.find("\r\n\r\n") {
        Some(i) => text[i + 4..].to_string(),
        None => text,
    }
}
