//! P7-M2-T02 acceptance contract: the OEP modeler/browser PDM stub speaks the
//! P8 REST API v1 contract (OpenAPI spec at P8-M6/openapi-v1.yaml).
//!
//! This test is the RED-first expression of the acceptance criteria:
//!   * A real HTTP stub server is booted on an ephemeral port (no in-process
//!     shortcuts, no mock transport).
//!   * The Rust client (`oep_pdm::client::PdmClient`) drives EVERY verb of the
//!     contract over real sockets: health, list items, create item, get item,
//!     list revisions, save a revision (upload), open a revision file
//!     (download, byte-exact), open the latest file, plus the error surface
//!     (404 / 400 / 409 with the contract error envelope).
//!   * Content addressing is real: the server stores blobs by sha-256 and the
//!     returned blob_sha256 matches a locally computed digest of the bytes.
//!   * Revision isolation: opening revision 1 still returns the original bytes
//!     after revision 2 is saved.
//!
//! "Real material" for an API task is the real fixture STEP file (committed)
//! round-tripped over a real socket, byte for byte.

use oep_pdm::api::{CreateItemRequest, CreateRevisionRequest, FileUpload, API_VERSION, ITEMS_PATH};
use oep_pdm::client::{FilePayload, PdmClient, PdmError};
use oep_pdm::stub::StubServer;
use sha2::{Digest, Sha256};

const FIXTURE: &str = "tests/fixtures/box-mini.step";

fn hex_sha256(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    format!("{:x}", h.finalize())
}

fn fixture_bytes() -> Vec<u8> {
    std::fs::read(FIXTURE).expect("fixture STEP file present")
}

fn upload(bytes: Vec<u8>, name: &str) -> FileUpload {
    use base64::Engine;
    FileUpload {
        filename: name.to_string(),
        format: "step".to_string(),
        content_type: Some("application/octet-stream".to_string()),
        content: base64::engine::general_purpose::STANDARD.encode(&bytes),
    }
}

#[test]
fn accept_health_verifies_contract_identity() {
    let srv = StubServer::start("127.0.0.1:0").expect("stub server starts");
    let c = PdmClient::new(&srv.base_url()).expect("client parses base url");

    let h = c.health().expect("GET /api/v1/health succeeds");
    assert_eq!(h.status, "ok");
    assert_eq!(h.service, "oep-pdm-stub");
    assert_eq!(
        h.version, API_VERSION,
        "service reports the contract version"
    );
}

#[test]
fn accept_open_latest_after_save_byte_round_trip() {
    let srv = StubServer::start("127.0.0.1:0").unwrap();
    let c = PdmClient::new(&srv.base_url()).unwrap();
    let part = fixture_bytes();

    // Create an item (empty catalogue first).
    let health = c.health().unwrap();
    assert_eq!(health.status, "ok");
    let list0 = c.list_items(None).unwrap();
    assert_eq!(list0.total, 0, "fresh stub has an empty catalogue");

    let item = c
        .create_item(&CreateItemRequest {
            name: "OpenBox".to_string(),
            kind: "part".to_string(),
            description: Some("round-trip fixture".to_string()),
            owner: Some("p7".to_string()),
            context: Some("w2".to_string()),
        })
        .expect("create item 201");
    assert_eq!(item.name, "OpenBox");
    assert_eq!(item.kind, "part");
    assert_eq!(item.latest_revision, None);
    assert_eq!(item.revision_count, 0);
    assert_eq!(item.context.as_deref(), Some("w2"));

    // Save revision 1 (the save-revision-to-PDM verb).
    let rev1 = c
        .create_revision(
            &item.id,
            &CreateRevisionRequest {
                state: Some("working".to_string()),
                comment: Some("initial geometry".to_string()),
                created_by: Some("p7-modeler".to_string()),
                file: upload(part.clone(), "box-mini.step"),
            },
        )
        .expect("create revision 201");
    assert_eq!(rev1.number, "1");
    assert_eq!(rev1.state, "working");
    assert_eq!(rev1.comment.as_deref(), Some("initial geometry"));
    assert_eq!(rev1.files.len(), 1);
    assert_eq!(rev1.files[0].size as usize, part.len());
    assert_eq!(rev1.files[0].blob_sha256, hex_sha256(&part));

    // Item now reflects the latest revision.
    let item2 = c.get_item(&item.id).unwrap();
    assert_eq!(item2.latest_revision.as_deref(), Some("1"));
    assert_eq!(item2.revision_count, 1);

    // Browser list shows the item with its latest revision.
    let list1 = c.list_items(None).unwrap();
    assert_eq!(list1.total, 1);
    assert_eq!(list1.items[0].id, item.id);
    assert_eq!(list1.items[0].latest_revision.as_deref(), Some("1"));

    // Open-from-PDM: download the latest file, byte-exact.
    let got: FilePayload = c.open_latest_file(&item.id).expect("open latest 200");
    assert_eq!(got.bytes, part, "latest file round-trips byte-exact");
    assert_eq!(got.filename, "box-mini.step");
    assert_eq!(got.format, "step");
    assert_eq!(
        got.blob_sha256,
        hex_sha256(&part),
        "download is content-addressed"
    );
    assert_eq!(got.rev, "1");
}

#[test]
fn accept_revision_isolation_and_latest_advance() {
    let srv = StubServer::start("127.0.0.1:0").unwrap();
    let c = PdmClient::new(&srv.base_url()).unwrap();
    let part = fixture_bytes();
    let modified: Vec<u8> = part.iter().map(|b| b ^ 0x5a).collect();

    let item = c
        .create_item(&CreateItemRequest {
            name: "Isolator".to_string(),
            kind: "part".to_string(),
            ..CreateItemRequest::default_min()
        })
        .unwrap();

    c.create_revision(
        &item.id,
        &CreateRevisionRequest {
            state: None,
            comment: None,
            created_by: None,
            file: upload(part.clone(), "box-mini.step"),
        },
    )
    .unwrap();

    // Save revision 2 with different bytes.
    let rev2 = c
        .create_revision(
            &item.id,
            &CreateRevisionRequest {
                state: Some("released".to_string()),
                comment: Some("geometry change".to_string()),
                created_by: Some("p7-modeler".to_string()),
                file: upload(modified.clone(), "box-mini.step"),
            },
        )
        .unwrap();
    assert_eq!(rev2.number, "2");
    assert_eq!(rev2.state, "released");
    assert_eq!(rev2.files[0].blob_sha256, hex_sha256(&modified));

    // Latest advanced to 2; revision 1 must still return the original bytes.
    assert_eq!(
        c.get_item(&item.id).unwrap().latest_revision.as_deref(),
        Some("2")
    );
    assert_eq!(c.open_latest_file(&item.id).unwrap().bytes, modified);
    assert_eq!(c.open_revision_file(&item.id, "1").unwrap().bytes, part);

    let rl = c.list_revisions(&item.id).unwrap();
    assert_eq!(rl.item_id, item.id);
    assert_eq!(rl.latest.as_deref(), Some("2"));
    assert_eq!(rl.revisions.len(), 2);

    // Revision metadata endpoint (no bytes) round-trips the second revision.
    let meta = c.get_revision(&item.id, "2").unwrap();
    assert_eq!(meta.number, "2");
    assert_eq!(meta.state, "released");
    assert_eq!(meta.files[0].size as usize, modified.len());
}

#[test]
fn accept_content_addressing_dedupes_identical_bytes() {
    let srv = StubServer::start("127.0.0.1:0").unwrap();
    let c = PdmClient::new(&srv.base_url()).unwrap();
    let part = fixture_bytes();
    let item = c
        .create_item(&CreateItemRequest {
            name: "Dedupe".to_string(),
            kind: "document".to_string(),
            ..CreateItemRequest::default_min()
        })
        .unwrap();

    let r1 = c
        .create_revision(
            &item.id,
            &CreateRevisionRequest {
                state: None,
                comment: None,
                created_by: None,
                file: upload(part.clone(), "box-mini.step"),
            },
        )
        .unwrap();
    let r2 = c
        .create_revision(
            &item.id,
            &CreateRevisionRequest {
                state: None,
                comment: None,
                created_by: None,
                file: upload(part.clone(), "box-mini.step"),
            },
        )
        .unwrap();
    assert_eq!(
        r1.files[0].blob_sha256, r2.files[0].blob_sha256,
        "identical bytes share one content address"
    );
}

#[test]
fn accept_error_surface_speaks_contract_envelope() {
    let srv = StubServer::start("127.0.0.1:0").unwrap();
    let c = PdmClient::new(&srv.base_url()).unwrap();

    // 404 unknown item on every item-scoped verb.
    let missing_req = CreateRevisionRequest {
        state: None,
        comment: None,
        created_by: None,
        file: upload(fixture_bytes(), "box-mini.step"),
    };
    let not_found_verbs: [Result<(), PdmError>; 4] = [
        c.get_item("missing-item").map(|_| ()),
        c.list_revisions("missing-item").map(|_| ()),
        c.open_latest_file("missing-item").map(|_| ()),
        c.create_revision("missing-item", &missing_req).map(|_| ()),
    ];
    for res in not_found_verbs {
        match res {
            Err(PdmError::Http { status, code, .. }) => {
                assert_eq!(status, 404);
                assert_eq!(code, "not_found");
            }
            other => panic!("expected 404 not_found, got {:?}", other.map(|_| ())),
        }
    }

    // Duplicate item name is a conflict.
    c.create_item(&CreateItemRequest {
        name: "Twice".to_string(),
        kind: "part".to_string(),
        ..CreateItemRequest::default_min()
    })
    .unwrap();
    let dup = c.create_item(&CreateItemRequest {
        name: "Twice".to_string(),
        kind: "part".to_string(),
        ..CreateItemRequest::default_min()
    });
    match dup {
        Err(PdmError::Http { status, code, .. }) => {
            assert_eq!(status, 409);
            assert_eq!(code, "duplicate_name");
        }
        other => panic!("expected 409 duplicate_name, got {:?}", other.map(|_| ())),
    }

    // Unknown revision state is a bad request.
    let item = c
        .create_item(&CreateItemRequest {
            name: "Stateful".to_string(),
            kind: "part".to_string(),
            ..CreateItemRequest::default_min()
        })
        .unwrap();
    let bad = c.create_revision(
        &item.id,
        &CreateRevisionRequest {
            state: Some("not-a-state".to_string()),
            comment: None,
            created_by: None,
            file: upload(fixture_bytes(), "box-mini.step"),
        },
    );
    match bad {
        Err(PdmError::Http { status, code, .. }) => {
            assert_eq!(status, 400);
            assert_eq!(code, "invalid_state");
        }
        other => panic!("expected 400 invalid_state, got {:?}", other.map(|_| ())),
    }

    // Item with no revisions yet: latest-file open is 404 no_file.
    match c.open_latest_file(&item.id) {
        Err(PdmError::Http { status, code, .. }) => {
            assert_eq!(status, 404);
            assert_eq!(code, "no_file");
        }
        other => panic!("expected 404 no_file, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn accept_base_path_is_contract_root() {
    // The client always speaks under /api/v1; the OpenAPI contract is anchored
    // there regardless of host/port. Probe the raw path directly.
    let srv = StubServer::start("127.0.0.1:0").unwrap();
    let url = srv.base_url();
    assert!(
        url.ends_with("/api/v1"),
        "base url anchors the contract root: {url}"
    );
    assert_eq!(ITEMS_PATH, "/api/v1/items");
}
