//! P8-M4-T01 web UI handlers.
//!
//! Every handler reads through the same `Store` the REST API uses and returns
//! server-rendered HTML (or a PNG for the viewer embed). HTML is hand-built
//! with simple escaping: no template dependency, deterministic output, and
//! every surface is testable over real HTTP.

use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use oep_geo::Document;
use oep_pdm::graph::RelationshipGraph;
use oep_pdm::service::Store;

use crate::web::preview::render_document_png;

/// The viewer-embed preview renderer (P6 oep-viewer seam).
pub mod preview;

/// Re-export the drawn-pixel counter for callers (tests, docs).
pub use preview::drawn_pixel_count;

const VIEW_WIDTH: u32 = 320;
const VIEW_HEIGHT: u32 = 240;

/// Build the merged router: the oep-pdm REST API v1 router plus the web UI
/// routes, all over the same store.
pub fn build_router(store: Store) -> Router<()> {
    let api = oep_pdm::service::build_router(store.clone());
    let ui = Router::new()
        .route("/", get(explorer))
        .route("/ui/items/{item_id}", get(item_page))
        .route("/ui/items/{item_id}/bom", get(bom_page))
        .route("/ui/items/{item_id}/preview/{rev}", get(preview_png))
        .route("/ui/compare", get(compare_page))
        .with_state(store);
    api.merge(ui)
}

/// The deterministic inline stylesheet for every UI page (no external
/// assets, no CDN, keeps the UI self-contained and offline-friendly).
fn static_css() -> &'static str {
    "body{font-family:system-ui,sans-serif;margin:2rem;color:#1c1c1c;background:#fafafa}\
     h1{font-size:1.4rem}table{border-collapse:collapse}\
     td,th{border:1px solid #ccc;padding:.4rem .7rem;text-align:left}\
     .bom ul{list-style:none;padding-left:1.4rem}.bom .qty{color:#666}\
     .pick{color:#0b5bd3}img.preview{border:1px solid #222;background:#1c1e22}"
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn page(title: &str, body: &str) -> Response {
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <title>{title}</title><style>{css}</style></head>\
         <body>{body}</body></html>",
        title = esc(title),
        css = static_css(),
        body = body,
    );
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

fn store_locked() -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, "store lock poisoned").into_response()
}

/// `GET /` -- item explorer: the vault catalogue.
pub async fn explorer(State(store): State<Store>) -> Response {
    let list = match store.lock() {
        Ok(g) => g.list_items("", 500, 0),
        Err(_) => return store_locked(),
    };
    let mut rows = String::new();
    for item in &list.items {
        rows.push_str(&format!(
            "<tr><td class=\"pick\"><a href=\"/ui/items/{id}\">{name}</a></td>\
             <td>{kind}</td><td>{rev}</td><td>{count}</td></tr>",
            id = esc(&item.id),
            name = esc(&item.name),
            kind = esc(&item.kind),
            rev = esc(item.latest_revision.as_deref().unwrap_or("-")),
            count = item.revision_count,
        ));
    }
    let body = format!(
        "<h1>OEP PDM item explorer</h1>\
         <p>{total} item(s) in the vault.</p>\
         <table><thead><tr><th>Name</th><th>Kind</th><th>Latest</th><th>Revisions</th></tr></thead>\
         <tbody>{rows}</tbody></table>",
        total = list.total,
        rows = rows,
    );
    page("Item explorer", &body)
}

/// `GET /ui/items/{item_id}` -- item detail with BOM tree + embedded viewer.
pub async fn item_page(State(store): State<Store>, Path(item_id): Path<String>) -> Response {
    let item = match store.lock() {
        Ok(g) => match g.get_item(&item_id) {
            Ok(i) => i,
            Err(e) => {
                return (StatusCode::NOT_FOUND, e.to_string()).into_response();
            }
        },
        Err(_) => return store_locked(),
    };

    let mut rev_table = String::new();
    let mut rev_options = String::new();
    let mut preview_imgs = String::new();
    let mut latest_rev: Option<String> = None;

    match store.lock() {
        Ok(g) => match g.list_revisions(&item_id) {
            Ok(rl) => {
                let revs: Vec<String> = rl.revisions.iter().map(|r| r.number.clone()).collect();
                let mut prev: Option<&str> = None;
                for r in rl.revisions.iter() {
                    rev_options.push_str(&format!(
                        "<option value=\"{n}\"{sel}>{n}</option>",
                        n = esc(&r.number),
                        sel = if Some(&r.number) == rl.latest.as_ref() {
                            " selected"
                        } else {
                            ""
                        },
                    ));
                    rev_table.push_str(&format!(
                        "<tr><td>{n}</td><td>{state}</td><td>{by}</td><td>{comment}</td>\
                         <td><a class=\"pick\" href=\"/ui/compare?item={item}&a={a}&b={b}\">compare</a></td></tr>",
                        n = esc(&r.number),
                        state = esc(&r.state),
                        by = esc(r.created_by.as_str()),
                        comment = esc(r.comment.as_deref().unwrap_or("-")),
                        item = esc(&item_id),
                        a = if let Some(p) = prev { esc(p) } else { esc(&r.number) },
                        b = esc(&r.number),
                    ));
                    let rev_for_preview = &r.number;
                    preview_imgs.push_str(&format!(
                        "<a href=\"/ui/compare?item={item}&a={rev}&b={rev}\">\
                         <img class=\"preview\" src=\"/ui/items/{item}/preview/{rev}.png\" \
                         alt=\"revision {rev} preview\" width=\"{w}\" height=\"{h}\"></a> ",
                        item = esc(&item_id),
                        rev = esc(rev_for_preview),
                        w = VIEW_WIDTH,
                        h = VIEW_HEIGHT,
                    ));
                    prev = Some(&r.number);
                }
                if let Some(first) = revs.first() {
                    latest_rev = Some(first.clone());
                }
            }
            Err(e) => {
                return (StatusCode::NOT_FOUND, e.to_string()).into_response();
            }
        },
        Err(_) => return store_locked(),
    }
    drop(store);

    let compare_form = format!(
        "<form action=\"/ui/compare\" method=\"get\">\
         <input type=\"hidden\" name=\"item\" value=\"{id}\">\
         <select name=\"a\">{opts}</select> \
         <span>vs</span> \
         <select name=\"b\">{opts}</select> \
         <button type=\"submit\">Compare</button></form>",
        id = esc(&item_id),
        opts = rev_options,
    );

    let bom_link = format!(
        "<p>BOM tree (<a class=\"pick\" href=\"/ui/items/{id}/bom\">{id}</a>).</p>",
        id = esc(&item_id),
    );

    let body = format!(
        "<p><a class=\"pick\" href=\"/\">&larr; explorer</a></p>\
         <h1>{name}</h1>\
         <table><tbody>\
         <tr><th>Id</th><td>{id}</td></tr>\
         <tr><th>Kind</th><td>{kind}</td></tr>\
         <tr><th>Description</th><td>{desc}</td></tr>\
         <tr><th>Latest revision</th><td>{latest}</td></tr>\
         </tbody></table>\
         {compare_form}\
         <h2>Revisions</h2><table><thead><tr><th>Rev</th><th>State</th><th>By</th>\
         <th>Comment</th><th></th></tr></thead><tbody>{rev_table}</tbody></table>\
         <h2>Viewer</h2><div>{preview_imgs}</div>\
         <h2>BOM tree</h2>{bom_link}\
         <div class=\"bom\"><a class=\"pick\" href=\"/ui/items/{id}/bom\">view rendered BOM</a></div>",
        name = esc(&item.name),
        id = esc(&item.id),
        kind = esc(&item.kind),
        desc = esc(item.description.as_deref().unwrap_or("-")),
        latest = esc(latest_rev.as_deref().unwrap_or("-")),
        compare_form = compare_form,
        rev_table = rev_table,
        preview_imgs = preview_imgs,
        bom_link = bom_link,
    );
    page(&format!("Item {}", item.name), &body)
}

/// Optional query parameters on `GET /ui/items/{item_id}/bom`.
#[derive(Debug, serde::Deserialize)]
pub struct BomQuery {
    /// Optional revision number to walk; defaults to the latest.
    pub rev: Option<String>,
}

/// `GET /ui/items/{item_id}/bom` -- render the BOM tree for the item.
pub async fn bom_page(
    State(store): State<Store>,
    Path(item_id): Path<String>,
    Query(q): Query<BomQuery>,
) -> Response {
    let (doc_bytes, _format_tag) = match store.lock() {
        Ok(g) => match q.rev.as_deref() {
            Some(rev) => match g.open_revision(&item_id, rev) {
                Ok((_r, file, bytes)) if file.format == "oep-geo" => (bytes, file.format),
                Ok((_r, _file, _bytes)) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        "item's file is not an oep-geo model; no BOM tree available".to_string(),
                    )
                        .into_response();
                }
                Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
            },
            None => match g.open_latest(&item_id) {
                Ok((_rev, file, bytes)) if file.format == "oep-geo" => (bytes, file.format),
                Ok((_rev, _file, _bytes)) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        "item's file is not an oep-geo model; no BOM tree available".to_string(),
                    )
                        .into_response();
                }
                Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
            },
        },
        Err(_) => return store_locked(),
    };

    let doc = match Document::from_json_pretty(&String::from_utf8_lossy(&doc_bytes)) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("stored document is not parseable oep-geo: {e}"),
            )
                .into_response();
        }
    };
    let graph = match RelationshipGraph::from_oepgeo_document(&doc) {
        Ok(g) => g,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("could not build relationship graph: {e}"),
            )
                .into_response();
        }
    };

    let roots = graph.roots();
    let mut tree = String::new();
    for root in &roots {
        render_bom_node(&graph, root, None, &mut tree);
    }
    if roots.is_empty() {
        tree.push_str("<li>no top-level items in this graph</li>");
    }

    let body = format!(
        "<p><a class=\"pick\" href=\"/ui/items/{id}\">&larr; item</a></p>\
         <h1>BOM tree: {id}</h1>\
         <p class=\"bom\"><ul>{tree}</ul></p>",
        id = esc(&item_id),
        tree = tree,
    );
    page(&format!("BOM tree {item_id}"), &body)
}

/// Render one node (and its subtree) of the relationship graph as a nested
/// list item. `edge_qty` is the quantity on the edge that led here (`None`
/// for a root), so quantity shows on every BOM edge.
fn render_bom_node(
    graph: &RelationshipGraph,
    item_id: &str,
    edge_qty: Option<u32>,
    out: &mut String,
) {
    let name = graph
        .item(item_id)
        .map(|m| m.name.clone())
        .unwrap_or_else(|| item_id.to_string());
    let kind = match graph.item(item_id) {
        Some(m) => match m.kind {
            oep_pdm::ItemKind::Part => "part",
            oep_pdm::ItemKind::Assembly => "assembly",
            oep_pdm::ItemKind::Document => "document",
        },
        None => "unknown",
    };
    let qty_suffix = match edge_qty {
        Some(1) | None => String::new(),
        Some(q) => format!(" <span class=\"qty\">(x{q})</span>"),
    };
    out.push_str(&format!(
        "<li data-id=\"{id}\"><span title=\"{kind}\">{name}</span>{qty_suffix}",
        id = esc(item_id),
        kind = esc(kind),
        name = esc(&name),
        qty_suffix = qty_suffix,
    ));
    let children = graph.children(item_id);
    if !children.is_empty() {
        out.push_str("<ul>");
        for edge in &children {
            if graph.contains(&edge.child) {
                render_bom_node(graph, &edge.child, Some(edge.qty), out);
            } else {
                // Tolerant: dangling edge renders as a marker, never crashes.
                out.push_str(&format!(
                    "<li class=\"qty\">missing child {name}</li>",
                    name = esc(&edge.child),
                ));
            }
        }
        out.push_str("</ul>");
    }
    out.push_str("</li>");
}

/// `GET /ui/items/{item_id}/preview/{rev}.png` -- viewer embed: render the
/// stored oep-geo model at that revision and serve a PNG.
///
/// The route is `/preview/{rev}` and callers append the `.png` extension for
/// clarity in `<img>` tags; the handler strips a trailing `.png` if present.
pub async fn preview_png(
    State(store): State<Store>,
    Path((item_id, rev)): Path<(String, String)>,
) -> Response {
    let rev = rev.strip_suffix(".png").unwrap_or(&rev);
    let (file_ref, bytes) = match store.lock() {
        Ok(g) => match g.open_revision(&item_id, rev) {
            Ok((_r, file, bytes)) => (file, bytes),
            Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
        },
        Err(_) => return store_locked(),
    };
    if file_ref.format != "oep-geo" {
        return (
            StatusCode::BAD_REQUEST,
            format!(
                "item {} revision {rev} is not an oep-geo model (format {})",
                item_id, file_ref.format
            ),
        )
            .into_response();
    }
    let doc = match Document::from_json_pretty(&String::from_utf8_lossy(&bytes)) {
        Ok(d) => d,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("document at {item_id}/{rev} is not parseable oep-geo: {e}"),
            )
                .into_response();
        }
    };
    match render_document_png(&doc, VIEW_WIDTH, VIEW_HEIGHT) {
        Ok((png, drawn)) => {
            let resp = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/png")
                .header("X-Oep-Item", &item_id)
                .header("X-Oep-Rev", rev)
                .header("X-Oep-Drawn-Pixels", drawn.to_string())
                .body(axum::body::Body::from(png));
            match resp {
                Ok(r) => r,
                Err(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "cannot build PNG response",
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("render failed: {e}"),
        )
            .into_response(),
    }
}

/// Query parameters for `GET /ui/compare`: which item and which two revision
/// numbers to compare (`a` and `b`).
#[derive(Debug, serde::Deserialize)]
pub struct CompareParams {
    item: String,
    a: String,
    b: String,
}

/// `GET /ui/compare?item={id}&a={revA}&b={revB}`: compare two revisions.
pub async fn compare_page(
    State(store): State<Store>,
    Query(params): Query<CompareParams>,
) -> Response {
    let get_rev_meta = |rev: &str| -> Result<(String, oep_pdm::api::Revision), String> {
        store
            .lock()
            .map_err(|_| "poisoned".to_string())
            .and_then(|g| {
                g.get_revision(&params.item, rev)
                    .map(|r| (rev.to_string(), r))
                    .map_err(|e| e.to_string())
            })
    };
    let (a_num, a_rev) = match get_rev_meta(&params.a) {
        Ok(x) => x,
        Err(e) => return (StatusCode::NOT_FOUND, e).into_response(),
    };
    let (b_num, b_rev) = match get_rev_meta(&params.b) {
        Ok(x) => x,
        Err(e) => return (StatusCode::NOT_FOUND, e).into_response(),
    };

    let (a_sha, a_size) = a_rev
        .files
        .first()
        .map(|f| (f.blob_sha256.clone(), f.size))
        .unwrap_or_default();
    let (b_sha, b_size) = b_rev
        .files
        .first()
        .map(|f| (f.blob_sha256.clone(), f.size))
        .unwrap_or_default();
    let identical = a_sha == b_sha && a_size == b_size;

    let diff_flag = if identical {
        "<p>Revisions carry the same file content: no difference detected.</p>"
    } else {
        "<p class=\"pick\">Revisions differ: file content (and/or size) changed.</p>"
    };

    let preview = |rev: &str, label: &str, r: &oep_pdm::api::Revision| -> String {
        let comment = r.comment.as_deref().unwrap_or("-");
        let sha = r
            .files
            .first()
            .map(|f| f.blob_sha256.as_str())
            .unwrap_or("-");
        format!(
            "<div style=\"display:inline-block;margin-right:1.5rem;vertical-align:top\">\
             <h3>{label}</h3>\
             <table><tbody>\
             <tr><th>Revision</th><td>{rev}</td></tr>\
             <tr><th>State</th><td>{state}</td></tr>\
             <tr><th>Comment</th><td>{comment}</td></tr>\
             <tr><th>Blob sha256</th><td>{sha}</td></tr>\
             </tbody></table>\
             <img class=\"preview\" src=\"/ui/items/{item}/preview/{rev}.png\" \
             alt=\"{label} preview\" width=\"{w}\" height=\"{h}\"></div>",
            label = esc(label),
            rev = esc(rev),
            state = esc(&r.state),
            comment = esc(comment),
            sha = esc(sha),
            item = esc(&params.item),
            w = VIEW_WIDTH,
            h = VIEW_HEIGHT,
        )
    };

    let body = format!(
        "<p><a class=\"pick\" href=\"/ui/items/{item}\">&larr; item</a></p>\
         <h1>Compare {item}: {a} vs {b}</h1>\
         {diff_flag}\
         <div>{pv_a}{pv_b}</div>",
        item = esc(&params.item),
        a = esc(&a_num),
        b = esc(&b_num),
        diff_flag = diff_flag,
        pv_a = preview(&a_num, "Revision 1", &a_rev),
        pv_b = preview(&b_num, "Revision 2", &b_rev),
    );
    page(&format!("Compare {}", params.item), &body)
}
