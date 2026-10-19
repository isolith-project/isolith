//! Browser/PDM integration demo for P7-M2-T02: the two verbs, end to end,
//! against a live stub server on an ephemeral port.
//!
//!   * open-from-PDM        `PdmClient::open_latest_file` (and `open_revision_file`)
//!   * save-revisions-to-PDM `PdmClient::save_revision` (bytes in, next revision out)
//!
//! Run: `cargo run -p oep-pdm --example browser-open-save-demo`.
//! The same calls are what the P7-M2-T01/OEP workbench browser buttons will make;
//! integration points are documented in `workstreams/P7-M2/pdm-integration.md`.

use std::io::Write as _;

use oep_pdm::api::CreateItemRequest;
use oep_pdm::client::{PdmClient, SaveRevisionInput};
use oep_pdm::stub::StubServer;

fn main() {
    let srv = StubServer::start("127.0.0.1:0").expect("stub server starts");
    let c = PdmClient::new(&srv.base_url()).expect("client parses base url");
    let step = b"ISO-10303-21; HEADER; FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }')); ENDSEC; DATA; ENDSEC; END-ISO-10303-21;".to_vec();

    println!("PDM contract root : {}", c.contract_root());
    println!("health            : {:#?}", c.health().unwrap());

    // The browser "new part, save to PDM" path.
    let item = c
        .create_item(&CreateItemRequest {
            name: "DemoBracket".to_string(),
            kind: "part".to_string(),
            description: Some("browser demo".to_string()),
            ..CreateItemRequest::default_min()
        })
        .expect("create item");
    println!("\ncreated item : {} (kind={})", item.name, item.kind);

    // Save revision 1 (initial geometry).
    let r1 = c
        .save_revision(
            &item.id,
            SaveRevisionInput {
                filename: "demobracket.step".to_string(),
                format: "step".to_string(),
                content_type: Some("application/octet-stream".to_string()),
                bytes: step.clone(),
                state: Some("working".to_string()),
                comment: Some("initial".to_string()),
                created_by: Some("p7-modeler".to_string()),
            },
        )
        .expect("save revision 1");
    println!(
        "saved revision  : #{} [{}] blob={}",
        r1.number, r1.state, r1.files[0].blob_sha256
    );

    // Save revision 2 (geometry change).
    let mut edited = step.clone();
    edited[0] = b'X';
    let r2 = c
        .save_revision(
            &item.id,
            SaveRevisionInput {
                filename: "demobracket.step".to_string(),
                format: "step".to_string(),
                content_type: None,
                bytes: edited.clone(),
                state: Some("released".to_string()),
                comment: Some("hole pattern added".to_string()),
                created_by: Some("p7-modeler".to_string()),
            },
        )
        .expect("save revision 2");
    println!(
        "saved revision  : #{} [{}] blob={}",
        r2.number, r2.state, r2.files[0].blob_sha256
    );

    // The browser "open from PDM" path: open the latest file back out.
    let open = c.open_latest_file(&item.id).expect("open latest");
    assert_eq!(open.bytes, edited, "open-from-PDM returns the latest bytes");
    assert_eq!(
        open.blob_sha256, r2.files[0].blob_sha256,
        "byte-exact + content addressed"
    );
    println!(
        "\nopened latest    : {} rev={} format={} {} bytes sha={}",
        open.filename,
        open.rev,
        open.format,
        open.bytes.len(),
        &open.blob_sha256[..12]
    );

    // Revision 1 is still the original bytes (revision isolation).
    let r1_file = c.open_revision_file(&item.id, "1").expect("open rev 1");
    assert_eq!(r1_file.bytes, step);
    println!(
        "opened rev 1     : {} bytes (unchanged by later save)",
        r1_file.bytes.len()
    );

    // The browser catalogue shows latest revision.
    let list = c.list_items(None).unwrap();
    println!(
        "\ncatalogue        : {} item(s); {}@rev{}\n",
        list.total,
        list.items[0].name,
        list.items[0].latest_revision.as_deref().unwrap_or("-")
    );
    let _ = std::io::stdout().flush();
}
