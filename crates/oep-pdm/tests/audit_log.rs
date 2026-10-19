//! P8-M3-T03 acceptance: tamper-evident, append-only audit log.
//!
//! Covers the three acceptance criteria from the card:
//!
//! 1. **Append-only at the storage layer.** The store exposes no update or
//!    delete path; appending only ever *grows* the on-disk log, never
//!    rewriting prior bytes, and the chain continues cleanly across a reopen.
//! 2. **Chained entry hashes make tampering detectable.** The proof test
//!    edits the on-disk log (modified payload, reorder, truncation) and shows
//!    `open`/`verify` fail with [`AuditError::TamperDetected`].
//! 3. **Read-access events are logged** like any change/release event -- the
//!    sovereignty stance: a read of a vault object leaves a tamper-evident
//!    footprint too.
//!
//! The log is JSON-lines under `<root>/audit.log` with a head checkpoint at
//! `<root>/audit.head`; both paths are public so an operator can point a
//! backing store at them. See `P8-M3/audit-log.md` for the format.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use oep_pdm::audit::{Action, AuditError, AuditLog};

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A unique, throwaway temp directory that cleans itself up on drop.
struct TempRoot {
    dir: PathBuf,
}

impl TempRoot {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("oep-pdm-audit-it-{}-{tag}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp root");
        Self { dir }
    }

    fn log_path(&self) -> PathBuf {
        self.dir.join("audit.log")
    }

    fn head_path(&self) -> PathBuf {
        self.dir.join("audit.head")
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// A vault target under audit, using the P8 (item, revision) identity style.
const ITEM: &str = "bracket/rev:1";

/// The 64-hex-char zero hash every *first* entry chains from (genesis).
fn genesis_hash() -> String {
    "0".repeat(64)
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[test]
fn append_only_across_reopen_chains_cleanly() {
    let t = TempRoot::new("append-only");
    let mut log = AuditLog::open(&t.dir).unwrap();
    assert!(log.is_empty());
    log.append("hal", Action::Change, ITEM, None).unwrap();
    log.record_read("grace", ITEM).unwrap();
    log.record_release("ada", ITEM, Some("maturity=final"))
        .unwrap();
    assert_eq!(log.len(), 3);
    log.verify().unwrap();

    // Reopen: every entry survives and the next append chains on.
    drop(log);
    let mut log2 = AuditLog::open(&t.dir).unwrap();
    assert_eq!(log2.entries().len(), 3);
    assert_eq!(log2.entries()[0].seq, 1);
    assert_eq!(log2.entries()[0].prev_hash, genesis_hash());
    assert_eq!(log2.entries()[2].action, Action::Release);
    log2.record_change("hal", "other/bracket/rev:1", None)
        .unwrap();
    assert_eq!(log2.len(), 4);
    log2.verify().unwrap();
}

#[test]
fn appending_only_grows_the_file_never_rewrites_prior_bytes() {
    let t = TempRoot::new("grow-only");
    let mut log = AuditLog::open(&t.dir).unwrap();
    log.record_change("alice", ITEM, None).unwrap();
    let after_one = fs::read(t.log_path()).unwrap();
    assert!(
        t.head_path().exists(),
        "head checkpoint must be written on first append"
    );
    log.record_read("bob", ITEM).unwrap();
    let after_two = fs::read(t.log_path()).unwrap();
    assert!(after_two.len() > after_one.len());
    assert!(
        after_two.starts_with(&after_one),
        "prior bytes must be untouched"
    );
    log.record_release("carol", ITEM, None).unwrap();
    let after_three = fs::read(t.log_path()).unwrap();
    assert!(after_three.starts_with(&after_two));
    log.verify().unwrap();
}

#[test]
fn entry_hashes_form_a_chain() {
    let t = TempRoot::new("chain");
    let mut log = AuditLog::open(&t.dir).unwrap();
    log.append("alice", Action::Change, ITEM, None).unwrap();
    log.append("bob", Action::Read, ITEM, None).unwrap();
    log.append("carol", Action::Release, ITEM, Some("maturity=final"))
        .unwrap();
    let e = log.entries();
    assert_eq!(e.len(), 3);

    // First entry chains from genesis; every later entry chains from the
    // hash of the previous entry.
    assert_eq!(e[0].prev_hash, genesis_hash());
    assert_eq!(e[1].prev_hash, e[0].hash);
    assert_eq!(e[2].prev_hash, e[1].hash);

    // All chaining hashes are real sha-256 fingerprints (64 hex lowercase).
    for entry in e {
        assert!(
            is_hex64(&entry.prev_hash),
            "bad prev_hash {}",
            entry.prev_hash
        );
        assert!(is_hex64(&entry.hash), "bad hash {}", entry.hash);
        assert_eq!(&entry.hash.to_lowercase(), &entry.hash);
    }
    // Distinct payloads produce distinct hashes.
    assert_ne!(e[0].hash, e[1].hash);
    assert_ne!(e[1].hash, e[2].hash);

    // The store's head matches the last chain link.
    assert_eq!(log.head_hash(), e[2].hash);
}

/// The acceptance proof: an out-of-band edit to the middle of the on-disk log
/// must be detected on reopen.
#[test]
fn tampering_with_middle_entry_is_detected() {
    let t = TempRoot::new("tamper-middle");
    {
        let mut log = AuditLog::open(&t.dir).unwrap();
        log.append("alice", Action::Change, ITEM, None).unwrap();
        log.append("bob", Action::Change, ITEM, Some("blueprint"))
            .unwrap();
        log.append("carol", Action::Release, ITEM, None).unwrap();
        log.verify().unwrap();
    }

    // An in-band tool silently rewrites one payload field on disk.
    let raw = fs::read_to_string(t.log_path()).unwrap();
    let tampered = raw.replace("\"actor\":\"bob\"", "\"actor\":\"eve\"");
    assert_ne!(raw, tampered, "the test must actually change a payload");
    fs::write(t.log_path(), tampered).unwrap();

    let err = AuditLog::open(&t.dir).unwrap_err();
    assert!(
        matches!(err, AuditError::TamperDetected { .. }),
        "expected TamperDetected, got {err:?}"
    );
}

#[test]
fn verify_detects_edit_after_open() {
    let t = TempRoot::new("verify-edit");
    let mut log = AuditLog::open(&t.dir).unwrap();
    log.append("alice", Action::Change, ITEM, None).unwrap();
    log.append("bob", Action::Read, ITEM, None).unwrap();
    log.verify().unwrap();

    // Edit the file under a live store: a fresh verify must fail.
    let raw = fs::read_to_string(t.log_path()).unwrap();
    let tampered = raw.replace(ITEM, "sabotaged/rev:9");
    fs::write(t.log_path(), tampered).unwrap();
    let err = log.verify().unwrap_err();
    assert!(
        matches!(err, AuditError::TamperDetected { .. }),
        "expected TamperDetected, got {err:?}"
    );
}

#[test]
fn reordered_entries_are_detected() {
    let t = TempRoot::new("reorder");
    {
        let mut log = AuditLog::open(&t.dir).unwrap();
        log.record_change("alice", ITEM, None).unwrap();
        log.record_change("bob", "second/rev:2", None).unwrap();
        log.record_change("carol", "third/rev:3", None).unwrap();
        log.verify().unwrap();
    }

    // Swap the first two lines: the sequence numbers become 2,1,3.
    let raw = fs::read_to_string(t.log_path()).unwrap();
    let mut lines: Vec<&str> = raw.lines().collect();
    assert_eq!(lines.len(), 3);
    lines.swap(0, 1);
    fs::write(t.log_path(), format!("{}\n", lines.join("\n"))).unwrap();

    let err = AuditLog::open(&t.dir).unwrap_err();
    assert!(
        matches!(err, AuditError::TamperDetected { .. })
            || matches!(err, AuditError::CorruptRecord { .. }),
        "expected a tamper/corruption error, got {err:?}"
    );
}

#[test]
fn truncated_log_is_detected() {
    let t = TempRoot::new("truncate");
    {
        let mut log = AuditLog::open(&t.dir).unwrap();
        for i in 0..5u64 {
            log.append(&format!("user{i}"), Action::Read, ITEM, None)
                .unwrap();
        }
        log.verify().unwrap();
    }

    // An adversary drops the tail of the log. The remaining lines still chain
    // among themselves, so only the persisted head anchor exposes the loss.
    let raw = fs::read_to_string(t.log_path()).unwrap();
    let mut lines: Vec<&str> = raw.lines().collect();
    let dropped = lines.pop().expect("log has lines").to_string();
    fs::write(t.log_path(), format!("{}\n", lines.join("\n"))).unwrap();
    assert!(!dropped.is_empty());

    let err = AuditLog::open(&t.dir).unwrap_err();
    assert!(
        matches!(err, AuditError::TamperDetected { .. }),
        "expected TamperDetected for a truncated log, got {err:?}"
    );
}

/// Acceptance #3: read-access events are logged like anything else (the
/// sovereignty stance -- reads leave a tamper-evident footprint).
#[test]
fn read_access_events_are_logged() {
    let t = TempRoot::new("reads-logged");
    let mut log = AuditLog::open(&t.dir).unwrap();
    let r1 = log.record_read("grady", "bracket/rev:3").unwrap();
    let r2 = log.record_read("jin", "bracket/rev:3").unwrap();
    assert_eq!(r1.action, Action::Read);
    assert_eq!(r2.action, Action::Read);
    assert_eq!(log.len(), 2);
    log.verify().unwrap();

    // Reads survive a reopen and are part of the same chain.
    let mut reopened = AuditLog::open(&t.dir).unwrap();
    assert!(reopened.entries().iter().all(|e| e.action == Action::Read));
    reopened
        .record_release("ada", "bracket/rev:3", None)
        .unwrap();
    reopened.verify().unwrap();
}

#[test]
fn empty_actor_or_target_is_rejected() {
    let t = TempRoot::new("validation");
    let mut log = AuditLog::open(&t.dir).unwrap();
    assert!(matches!(
        log.append("", Action::Read, ITEM, None),
        Err(AuditError::EmptyField(_))
    ));
    assert!(matches!(
        log.append("alice", Action::Read, "", None),
        Err(AuditError::EmptyField(_))
    ));
    assert!(matches!(
        log.record_read("  ", ITEM),
        Err(AuditError::EmptyField(_))
    ));
    assert_eq!(log.len(), 0, "rejected appends must not write anything");
    log.verify().unwrap();
}

/// Detached store check: `verify` is a public, callable guardian on a path,
/// so an operator process can watchdog the same log independently.
#[test]
fn open_missing_root_creates_an_empty_store() {
    let t = TempRoot::new("fresh");
    let log = AuditLog::open(&t.dir).unwrap();
    assert!(log.entries().is_empty());
    assert_eq!(log.head_hash(), genesis_hash());
    assert!(Path::new(&t.log_path()).exists());
    log.verify().unwrap();
}
