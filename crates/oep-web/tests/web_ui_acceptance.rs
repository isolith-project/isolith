//! P8-M4-T01 acceptance contract: the PDM web UI.
//!
//! This test is the RED-first expression of the acceptance criteria:
//!
//!   * explorer     -- `GET /` lists the vault catalogue (items from the
//!     store, with links into each item page).
//!   * BOM tree     -- `GET /ui/items/{id}/bom` renders a BOM tree that is
//!     walked from the PDM relationship graph (`RelationshipGraph`) built
//!     from the item's real oep-geo document: an assembly lists its composed
//!     children with quantity, recursively.
//!   * viewer embed -- `GET /ui/items/{id}/preview/{rev}.png` renders the
//!     real oep-geo model at that revision through the P6 viewer embed
//!     (oep-viewer reference rasterizer), serves a real PNG, and reports the
//!     drawn-pixel count in an `X-Oep-Drawn-Pixels` header. Renders of
//!     different revisions differ when the geometry is different.
//!   * compare view -- `GET /ui/compare?item={id}&a={revA}&b={revB}` renders
//!     a compare page showing both revisions (metadata + embedded previews)
//!     and reports the difference between them.
//!
//! All of it is driven over real HTTP on an ephemeral port (no in-process
//! shortcuts, no mock transport), seeded with the committed reference
//! vertical fixture: the golden bracket assembly (parts + a nested
//! fastener-set sub-assembly, 4 fasteners) with TWO revisions (the golden as
//! rev 1, a geometry-mutated variant as rev 2) so compare has something real
//! to show.
use std::io::Write;
use std::net::TcpStream;
use std::time::Duration;

use base64::Engine as _;
use oep_geo::Document;
use oep_pdm::api::{CreateItemRequest, CreateRevisionRequest, FileUpload};
use oep_pdm::http::{self, Response};
use oep_pdm::store::VaultStore;
use oep_web::PdmWeb;

const BRACKET_GOLDEN: &str = include_str!("../../oep-geo/tests/golden/example-bracket.oepgeo.json");

fn raw_get(addr: &str, path: &str) -> Response {
    let mut stream = TcpStream::connect(addr).expect("connect to web server");
    stream
        .set_read_timeout(Some(Duration::from_secs(20)))
        .expect("set timeout");
    let req = format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).expect("write request");
    stream.flush().ok();
    http::read_response(&mut stream).expect("parse HTTP response")
}

/// Returns a mutated copy of the golden bracket: one fewer fastener in the
/// set and a renamed top assembly. Different bytes, different geometry, so
/// revision 2 differs from revision 1 in preview and in BOM.
fn bracket_variant() -> String {
    let mut doc = Document::from_json_pretty(BRACKET_GOLDEN).expect("golden parses");
    for a in doc.assemblies.iter_mut() {
        if a.id == "oep-assembly-fastener-set" {
            a.instances.pop();
        }
        if a.id == "oep-assembly-bracket-assembly" {
            a.name = "bracket-assembly-rev2".to_string();
        }
    }
    doc.to_json_pretty().expect("serialize variant")
}

fn seed_store(store: &mut VaultStore) -> String {
    let item = store
        .create_item(&CreateItemRequest {
            name: "kw-bracket-assembly".to_string(),
            kind: "assembly".to_string(),
            description: Some("reference vertical fixture".to_string()),
            owner: Some("p8".to_string()),
            context: Some("w2".to_string()),
        })
        .expect("create fixture item");
    let upload = |bytes: Vec<u8>, rev_comment: &str| CreateRevisionRequest {
        state: Some("working".to_string()),
        comment: Some(rev_comment.to_string()),
        created_by: Some("p8-web-ui".to_string()),
        file: FileUpload {
            filename: "bracket.oepgeo.json".to_string(),
            format: "oep-geo".to_string(),
            content_type: Some("application/json".to_string()),
            content: base64::engine::general_purpose::STANDARD.encode(&bytes),
        },
    };
    store
        .create_revision(
            &item.id,
            &upload(BRACKET_GOLDEN.as_bytes().to_vec(), "golden revision 1"),
        )
        .expect("save rev 1");
    store
        .create_revision(
            &item.id,
            &upload(bracket_variant().into_bytes(), "variant revision 2"),
        )
        .expect("save rev 2");
    item.id
}

#[test]
fn explorer_lists_vault_items() {
    let mut store = VaultStore::new();
    let _item_id = seed_store(&mut store);
    let web = PdmWeb::start_with_store("127.0.0.1:0", store).expect("web server starts");
    let resp = raw_get(&web.addr(), "/");
    assert_eq!(resp.status, 200);
    let page = String::from_utf8_lossy(&resp.body).into_owned();
    assert!(
        page.contains("kw-bracket-assembly"),
        "explorer lists the item"
    );
    assert!(page.contains("assembly"), "explorer shows item kind");
    assert!(page.contains("/ui/items/"), "explorer links to item pages");
}

#[test]
fn bom_tree_walks_relationship_graph() {
    let mut store = VaultStore::new();
    let item_id = seed_store(&mut store);
    let web = PdmWeb::start_with_store("127.0.0.1:0", store).expect("web server starts");
    let addr = web.addr();
    let resp = raw_get(&addr, &format!("/ui/items/{item_id}/bom?rev=1"));
    assert_eq!(resp.status, 200);
    let page = String::from_utf8_lossy(&resp.body).into_owned();
    // Walked from the relationship graph: the top assembly composes the
    // bracket part + the fastener-set sub-assembly, and the fastener-set
    // composes 4 fasteners in rev 1 (golden).
    assert!(
        page.contains("bracket-assembly"),
        "top assembly node present"
    );
    assert!(page.contains("oep-part-bracket"), "bracket child present");
    assert!(
        page.contains("oep-assembly-fastener-set"),
        "sub-assembly child present"
    );
    assert!(page.contains("oep-part-fastener"), "fastener leaf present");
    assert!(
        page.contains("x4"),
        "fastener-set has quantity 4 in the golden"
    );
}

#[test]
fn viewer_embed_renders_real_models() {
    let mut store = VaultStore::new();
    let item_id = seed_store(&mut store);
    let web = PdmWeb::start_with_store("127.0.0.1:0", store).expect("web server starts");
    let addr = web.addr();

    let resp = raw_get(&addr, &format!("/ui/items/{item_id}/preview/1.png"));
    assert_eq!(resp.status, 200);
    let ct = resp.header("content-type").unwrap_or_default().to_string();
    assert_eq!(ct, "image/png", "preview served as PNG");
    assert_eq!(&resp.body[..8], b"\x89PNG\r\n\x1a\n", "real PNG magic");
    assert!(
        resp.body.len() > 200,
        "non-trivial render, got {} bytes",
        resp.body.len()
    );
    let drawn: usize = resp
        .header("x-oep-drawn-pixels")
        .and_then(|v| v.parse().ok())
        .expect("server reports drawn pixel count");
    assert!(drawn > 100, "viewer embed drew real geometry (non-bg px)");
    assert!(
        drawn < resp.body.len() as usize,
        "drawn pixels plausibly below total buffer"
    );

    // A different revision has different geometry => different render.
    let resp2 = raw_get(&addr, &format!("/ui/items/{item_id}/preview/2.png"));
    assert!(resp2.body != resp.body, "rev 2 render differs from rev 1");
}

#[test]
fn compare_view_shows_two_revisions() {
    let mut store = VaultStore::new();
    let item_id = seed_store(&mut store);
    let web = PdmWeb::start_with_store("127.0.0.1:0", store).expect("web server starts");
    let addr = web.addr();
    let resp = raw_get(&addr, &format!("/ui/compare?item={item_id}&a=1&b=2"));
    assert_eq!(resp.status, 200);
    let page = String::from_utf8_lossy(&resp.body).into_owned();
    assert!(page.contains("Revision 1"), "compare shows revision a");
    assert!(page.contains("Revision 2"), "compare shows revision b");
    assert!(
        page.contains("golden revision 1"),
        "compare shows rev1 comment"
    );
    assert!(
        page.contains("variant revision 2"),
        "compare shows rev2 comment"
    );
    assert!(page.contains("/preview/1.png"), "embeds rev1 preview");
    assert!(page.contains("/preview/2.png"), "embeds rev2 preview");
    assert!(
        page.contains("differ") || page.contains("Differ"),
        "compare reports a difference"
    );
}

#[test]
fn rest_api_still_served_alongside_ui() {
    let mut store = VaultStore::new();
    let _item_id = seed_store(&mut store);
    let web = PdmWeb::start_with_store("127.0.0.1:0", store).expect("web server starts");
    let resp = raw_get(&web.addr(), "/api/v1/health");
    assert_eq!(resp.status, 200);
    let text = String::from_utf8_lossy(&resp.body).into_owned();
    assert!(text.contains("\"status\":\"ok\""), "health alive: {text}");
}
