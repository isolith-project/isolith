//! P8-M6-T02 acceptance contract (RED-first): integration adapters -- the OEP
//! shell (P7 workbench) and any adopted tools talk to the PDM API through a
//! thin adapter layer instead of raw REST calls.
//!
//! The contract locked here, against the LIVE PDM service (the P8-M1-T01
//! axum `PdmService`, booted on an ephemeral port -- the stub path is gone):
//!
//! 1. `PdmAdapter::open_from_pdm(item)` returns the latest revision's bytes
//!    plus its metadata, byte-exact over a real socket (replaces the
//!    P7-M2-T02 local-stub path with real PDM).
//! 2. `PdmAdapter::open_from_pdm_at(item, rev)` opens a specific revision;
//!    revision 1 still returns the original bytes after revision 2 is saved
//!    (revision isolation through the adapter).
//! 3. `PdmAdapter::save_revisions_to_pdm(item, spec)` publishes bytes as the
//!    item's next revision, creating the item when it does not exist yet, and
//!    returns the server-assigned revision with its content-address.
//! 4. `PdmAdapter::catalogue()` lists the vault; `revisions(item)` lists an
//!    item's revision records. Both are typed, non-panicking outcomes.
//! 5. The adapter layer is unit-instrumented: an `AdapterInstrument` records
//!    every verb call and byte totals so integration behavior is observable
//!    and assertable from tests.
//! 6. Clean-room: the adapter is our own typed contract over the P8 REST API
//!    v1; no vendor PDM client (ENOVIA/3DEXPERIENCE/Teamcenter) API or
//!    semantics are copied, re-exported, or referenced.
//!
//! "Real material" for an integration task is a committed fixture STEP file
//! round-tripped through the live service via the adapter, byte for byte.

use oep_pdm::adapter::{AdapterInstrument, PdmAdapter};
use oep_pdm::service::PdmService;
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

fn boot_adapter() -> (PdmService, PdmAdapter) {
    let srv = PdmService::start("127.0.0.1:0").expect("real PdmService boots");
    let adapter = PdmAdapter::new(&srv.base_url()).expect("adapter binds to live service");
    (srv, adapter)
}

#[test]
fn adapter_open_and_save_round_trip_byte_exact_against_live_service() {
    let (_srv, mut adapter) = boot_adapter();
    let part = fixture_bytes();

    // Health probe through the adapter.
    let h = adapter.health().expect("health via adapter");
    assert_eq!(h.status, "ok");
    assert_eq!(h.service, "oep-pdm");

    // Save-revisions-to-PDM: the item does not exist yet and the adapter
    // creates it (the shell publishes a new part without a pre-created item).
    let saved = adapter
        .save_revisions_to_pdm(
            "OpenAdapterPart",
            "box-mini.step",
            "step",
            part.clone(),
            "initial adapter save",
        )
        .expect("save-revisions-to-PDM succeeds");
    assert_eq!(saved.revision, "1");
    assert_eq!(saved.blob_sha256, hex_sha256(&part));
    assert_eq!(saved.item_id, "OpenAdapterPart");

    // Open-from-PDM: the adapter returns the latest bytes, byte-exact.
    let opened = adapter
        .open_from_pdm("OpenAdapterPart")
        .expect("open-from-PDM succeeds");
    assert_eq!(opened.bytes, part, "latest file round-trips byte-exact");
    assert_eq!(opened.blob_sha256, hex_sha256(&part), "content-addressed");
    assert_eq!(opened.rev, "1");
    assert_eq!(opened.filename, "box-mini.step");
    assert_eq!(opened.format, "step");
}

#[test]
fn adapter_save_then_open_at_revision_isolation_via_live_service() {
    let (_srv, mut adapter) = boot_adapter();
    let part = fixture_bytes();
    let modified: Vec<u8> = part.iter().map(|b| b ^ 0x5a).collect();

    // Publish revision 1.
    adapter
        .save_revisions_to_pdm("Isolator", "box-mini.step", "step", part.clone(), "rev 1")
        .unwrap();

    // Publish revision 2 with different bytes.
    let rev2 = adapter
        .save_revisions_to_pdm(
            "Isolator",
            "box-mini.step",
            "step",
            modified.clone(),
            "rev 2",
        )
        .expect("save revision 2");
    assert_eq!(rev2.revision, "2");
    assert_eq!(rev2.blob_sha256, hex_sha256(&modified));

    // Open-from-PDM at a revision: revision 1 must still be the original.
    let r1 = adapter
        .open_from_pdm_at("Isolator", "1")
        .expect("open revision 1");
    assert_eq!(r1.bytes, part, "revision isolation through the adapter");

    // The latest open returns revision 2.
    let latest = adapter.open_from_pdm("Isolator").unwrap();
    assert_eq!(latest.bytes, modified);
    assert_eq!(latest.rev, "2");
}

#[test]
fn adapter_revisions_and_catalogue_are_typed_surfaces() {
    let (_srv, mut adapter) = boot_adapter();
    let part = fixture_bytes();

    adapter
        .save_revisions_to_pdm("Bracket", "box-mini.step", "step", part.clone(), "rev 1")
        .unwrap();
    adapter
        .save_revisions_to_pdm("Bracket", "box-mini.step", "step", part.clone(), "rev 2")
        .unwrap();

    // Catalogue lists the item once, with the latest revision.
    let catalogue = adapter.catalogue(None).expect("catalogue");
    let item = catalogue
        .iter()
        .find(|i| i.name == "Bracket")
        .expect("adapter-created item is in the catalogue");
    assert_eq!(item.latest_revision.as_deref(), Some("2"));
    assert_eq!(item.revision_count, 2);

    // Revisions surface the two records.
    let revisions = adapter.revisions("Bracket").expect("revisions");
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].number, "2"); // newest first
    assert_eq!(revisions[1].number, "1");

    // Missing items are typed failures, never panics.
    let missing = adapter
        .open_from_pdm("never-saved")
        .expect_err("typed failure");
    assert!(missing.to_string().to_lowercase().contains("not found"));
}

#[test]
fn adapter_layer_is_unit_instrumented() {
    let (_srv, mut adapter) = boot_adapter();
    let part = fixture_bytes();

    assert_eq!(adapter.instrument(), &AdapterInstrument::default());
    let _ = adapter.health();
    adapter
        .save_revisions_to_pdm("Quota", "box-mini.step", "step", part.clone(), "rev 1")
        .unwrap();
    adapter
        .save_revisions_to_pdm("Quota", "box-mini.step", "step", part.clone(), "rev 2")
        .unwrap();
    let _ = adapter.open_from_pdm("Quota");
    let _ = adapter.open_from_pdm_at("Quota", "1");
    let _ = adapter.catalogue(None);
    let _ = adapter.revisions("Quota");
    // One deliberate failure is recorded as an error too.
    let _ = adapter.open_from_pdm("does-not-exist");
    let insc = adapter.instrument();
    assert_eq!(insc.health_checks, 1);
    assert_eq!(insc.saves, 2);
    assert_eq!(insc.opens, 2); // latest + at-revision
    assert_eq!(insc.catalogue_queries, 1);
    assert_eq!(insc.revision_queries, 1);
    // Uploaded = 2 files' bytes; downloaded = 2 opens' bytes.
    assert_eq!(insc.bytes_uploaded, part.len() as u64 * 2);
    assert_eq!(insc.bytes_downloaded, part.len() as u64 * 2);
    assert_eq!(insc.errors, 1);
}
