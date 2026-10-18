//! P8-M6-T03 acceptance (Rust): the additive GA-gate surface (working-set
//! lock, lifecycle transition, audit) served over real sockets by the live
//! `PdmService`, driven by the durable Rust `PdmClient`.
//!
//! This mirrors how the M6-T02 adapter tests boot the live service on an
//! ephemeral port; here we exercise the *gate* verbs that the P8 GA gate
//! (python/oep_tooling/ga_gate_p8.py) drives. TDD: written against the
//! expected wire behaviour (RED until the gate surface landed), now GREEN.

use std::time::Duration;

use oep_pdm::api::{AuditVerifyReply, LockAcquireRequest, LockReleaseRequest, TransitionRequest};
use oep_pdm::client::PdmClient;
use oep_pdm::service::PdmService;
use oep_pdm::store::VaultStore;

fn boot() -> (PdmService, std::path::PathBuf) {
    // Real deployment posture: a persisted vault carries the tamper-evident
    // audit log too (P8-M6-T03), so the audit leg is live over the wire.
    let dir = std::env::temp_dir().join(format!("oep-pdm-gate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp root");
    let mut store = VaultStore::new();
    store.set_persist_dir(&dir).expect("persist dir");
    store.enable_audit().expect("audit enabled");
    let srv = PdmService::start_with_store("127.0.0.1:0", store).expect("boot service");
    (srv, dir)
}

fn wait_healthy(c: &PdmClient) {
    for _ in 0..50 {
        if c.health().is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("service did not become healthy");
}

#[test]
fn gate_lock_publish_immutable_audit_over_wire() {
    let (srv, dir) = boot();
    gate_body(&srv);
    let _ = std::fs::remove_dir_all(&dir);
}

fn gate_body(srv: &PdmService) {
    let c = PdmClient::new(&srv.base_url()).expect("client");
    wait_healthy(&c);

    // Seed one item + one working revision.
    let item = c
        .create_item(&oep_pdm::api::CreateItemRequest {
            name: "gate-rust".into(),
            kind: "part".into(),
            description: None,
            owner: Some("rust-gate".into()),
            context: Some("p8-m6-t03".into()),
        })
        .expect("create item");
    let iid = item.id.clone();
    c.save_revision(
        &iid,
        oep_pdm::client::SaveRevisionInput {
            filename: "blob.step".into(),
            format: "step".into(),
            content_type: None,
            bytes: b"ISO-10303-21; rust gate;".to_vec(),
            state: None,
            comment: Some("rust gate working".into()),
            created_by: Some("rust-gate".into()),
        },
    )
    .expect("save working revision");

    // 1. Locks respected: the second acquire on the same item is refused.
    let lock1 = c
        .acquire_lock(
            &iid,
            &LockAcquireRequest {
                holder: "alice".into(),
            },
        )
        .expect("alice acquires");
    let conflict = c.acquire_lock(
        &iid,
        &LockAcquireRequest {
            holder: "bob".into(),
        },
    );
    let conflict_code = match conflict {
        Err(oep_pdm::client::PdmError::Http { code, .. }) => code,
        other => panic!("expected lock conflict for bob, got {other:?}"),
    };
    assert_eq!(
        conflict_code, "lock_conflict",
        "second acquire must conflict"
    );

    // 2. Publish gate: a forged token cannot release; alice's token can.
    let forged = c.transition_revision(
        &iid,
        "1",
        &TransitionRequest {
            to: "released".into(),
            token: "lk-0000000000000000".into(),
        },
    );
    assert!(forged.is_err(), "non-holder transition must be refused");
    let released = c
        .transition_revision(
            &iid,
            "1",
            &TransitionRequest {
                to: "released".into(),
                token: lock1.token.clone(),
            },
        )
        .expect("alice releases");
    assert_eq!(released.state, "released", "released after valid publish");

    // 3. Immutable after release: released -> working is refused.
    let imm = c.transition_revision(
        &iid,
        "1",
        &TransitionRequest {
            to: "working".into(),
            token: lock1.token.clone(),
        },
    );
    assert!(
        imm.is_err(),
        "released revision must be immutable (hop refused)"
    );

    // 4. Release the lock with the right token.
    c.release_lock(
        &iid,
        &LockReleaseRequest {
            token: lock1.token.clone(),
        },
    )
    .expect("alice releases lock");
    let relock = c.acquire_lock(
        &iid,
        &LockAcquireRequest {
            holder: "bob".into(),
        },
    );
    assert!(relock.is_ok(), "item acquirable again after release");

    // 5. Audit: the log exists and the chain verifies.
    let view = c.list_audit().expect("audit view");
    assert!(view.append_only, "audit log is append-only by construction");
    assert!(view.count > 0, "audit log has entries over the wire");
    let verify: AuditVerifyReply = c.verify_audit().expect("audit verify");
    assert!(verify.ok, "tamper-evident chain re-verifies");
    assert_eq!(verify.count, view.count, "verify count matches view count");
}
