//! P8-M3-T03: append-only, tamper-evident audit log.
//!
//! Records *who* did *what* to *which* vault object -- covering reads as well
//! as changes and releases, because under the sovereignty principle a read is
//! an access event and leaves a footprint too. The store is **append-only**
//! (there is no update or delete path; the only writer is an `O_APPEND`
//! handle) and every entry carries a chained SHA-256 so that any tampering
//! with the on-disk log is **detected** on open or on [`AuditLog::verify`].
//!
//! On-disk layout under the store root:
//!
//! ```text
//! <root>/audit.log    JSON-lines: one Entry per line, appended only
//! <root>/audit.head   head checkpoint {seq, hash, count}, committed atomically
//! ```
//!
//! Each entry records the hash of the whole previous entry as `prev_hash`;
//! its own `hash` is
//!
//! ```text
//! SHA-256( prev_hash || seq || actor || target || action || at_unix || detail )
//! ```
//!
//! over an explicit length-prefixed canonical byte layout (see
//! [`chain_hash`]) that is independent of the JSON encoding, so verification
//! recomputes the chain from the parsed fields alone. The genesis entry chains
//! from 64 zero bytes. Because hashes chain, editing an entry breaks either
//! its own hash or the next entry's link; deleting a middle entry breaks the
//! sequence; deleting the tail disagrees with the persisted head checkpoint;
//! reordering breaks the sequence. Every one of those mutations is surfaced as
//! [`AuditError::TamperDetected`] -- the acceptance proof lives in
//! `crates/oep-pdm/tests/audit_log.rs`.
//!
//! Honest boundary: this log is **tamper-evident**, not tamper-proof. It
//! detects any modification of the log, but an adversary who can rewrite both
//! the log and the head checkpoint *and* recompute the chain could forge a
//! self-consistent log. Binding the head hash to an external anchor (a
//! separate WORM/SIEM sink, or a periodic seal) is the documented hardening
//! path and is out of scope for this card.
//!
//! Actor identity is a plain identifier string for now. When the P8-M3-T01
//! identity card lands, callers will pass the authenticated principal's
//! identifier here; P8-M3-T02 decides who may read/change/release, and this
//! log records what actually happened.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Name of the append-only log file inside the store root.
pub const LOG_FILE: &str = "audit.log";
/// Name of the head-checkpoint file inside the store root.
pub const HEAD_FILE: &str = "audit.head";
/// The hash an empty/`prev`-less log chains from: 64 hex zeros (genesis).
pub const GENESIS_HEX: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// The audited action categories this log records.
///
/// `Read` exists deliberately: under the sovereignty principle, reading a
/// vault object is an access event and is logged exactly like a mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// A principal read a vault object (sovereignty: reads are access events).
    Read,
    /// A vault object was created or modified.
    Change,
    /// A revision was released (or otherwise transitioned a lifecycle gate).
    Release,
}

impl Action {
    /// Canonical lowercase tag used in the chain layout (`"read"` / `"change"` / `"release"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Read => "read",
            Action::Change => "change",
            Action::Release => "release",
        }
    }

    /// Single-byte tag used inside the canonical hash layout (0/1/2).
    fn tag(self) -> u8 {
        match self {
            Action::Read => 0,
            Action::Change => 1,
            Action::Release => 2,
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single audit entry -- the chaining unit of the log.
///
/// `prev_hash` is the `hash` of the chronologically previous entry (or
/// [`GENESIS_HEX`] for the first entry); `hash` is the cryptographic link of
/// this entry (see module docs). [`Entry`] values are immutable once written;
/// the log exposes no way to alter an existing entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// 1-based sequence number; the log's real order (the clock is advisory).
    pub seq: u64,
    /// Who acted -- a principal identifier (identity, never a secret).
    pub actor: String,
    /// What kind of event this is.
    pub action: Action,
    /// What was acted on (e.g. `"bracket/rev:3"`, an item id, a ticket id).
    pub target: String,
    /// Free-form note, e.g. `"maturity=final"` on a release.
    pub detail: Option<String>,
    /// Wall-clock unix seconds the event was recorded (informational only).
    pub at_unix: u64,
    /// The `hash` of the previous entry; `GENESIS_HEX` for `seq == 1`.
    pub prev_hash: String,
    /// This entry's chained SHA-256 (see module docs for the exact bytes).
    pub hash: String,
}

impl Entry {
    /// The semantic payload the chain hash covers (everything but the two
    /// chain bookkeeping fields).
    fn payload(&self) -> HashPayload {
        HashPayload {
            seq: self.seq,
            actor: self.actor.clone(),
            action: self.action,
            target: self.target.clone(),
            at_unix: self.at_unix,
            detail: self.detail.clone(),
        }
    }
}

/// The semantic fields hashed into the chain (see [`chain_hash`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct HashPayload {
    seq: u64,
    actor: String,
    action: Action,
    target: String,
    at_unix: u64,
    detail: Option<String>,
}

/// Why an audit-log operation failed.
#[derive(Debug)]
pub enum AuditError {
    /// An I/O error while touching the store (the underlying error is the `source`).
    Io {
        /// What was being done.
        context: String,
        /// The underlying OS error.
        source: io::Error,
    },
    /// A stored line is not a well-formed audit entry.
    CorruptRecord {
        /// 1-based line number in `audit.log`.
        line: u64,
        /// Why the line is bad.
        detail: String,
    },
    /// The chain or the head checkpoint disagrees: evidence of tampering.
    TamperDetected {
        /// Where the chain broke (audit.log line, or 0 for a head mismatch).
        at: u64,
        /// What was detected.
        detail: String,
    },
    /// `actor` or `target` was empty/blank.
    EmptyField(&'static str),
    /// An invariant the module itself guarantees was violated (a bug).
    Internal(&'static str),
}

impl fmt::Display for AuditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuditError::Io { context, source } => {
                write!(f, "audit store I/O error while {context}: {source}")
            }
            AuditError::CorruptRecord { line, detail } => {
                write!(f, "corrupt audit record at line {line}: {detail}")
            }
            AuditError::TamperDetected { at, detail } => {
                write!(f, "TAMPER DETECTED (at line {at}): {detail}")
            }
            AuditError::EmptyField(which) => {
                write!(
                    f,
                    "empty {which}: an audit entry needs both an actor and a target"
                )
            }
            AuditError::Internal(msg) => write!(f, "internal audit-log invariant violated: {msg}"),
        }
    }
}

impl std::error::Error for AuditError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AuditError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Persisted head checkpoint: the anchor a clean chain must end at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct HeadState {
    /// Sequence number of the last entry (`0` for an empty log).
    seq: u64,
    /// `hash` of the last entry (`GENESIS_HEX` for an empty log).
    hash: String,
    /// Number of entries in the log.
    count: u64,
}

impl HeadState {
    fn genesis() -> Self {
        HeadState {
            seq: 0,
            hash: GENESIS_HEX.to_string(),
            count: 0,
        }
    }
}

/// The append-only, tamper-evident audit store.
///
/// Construct with [`AuditLog::open`]. Writing is `append`/`record_*` only;
/// there is deliberately no update or delete path. Reads are [`AuditLog::verify`]
/// (re-derives the whole chain from disk) or [`AuditLog::entries`] (cached
/// view from open/append). `append_only()` is a stable predicate documenting
/// that the storage layer enforces append-only.
#[derive(Debug)]
pub struct AuditLog {
    root: PathBuf,
    log_path: PathBuf,
    head_path: PathBuf,
    /// The single writer handle; opened `O_APPEND`, never truncate.
    file: File,
    /// Entries read at open plus everything appended since.
    entries: Vec<Entry>,
    /// The anchor as of the last open/append.
    head: HeadState,
}

impl AuditLog {
    /// Open (creating if needed) the audit log rooted at `root`.
    ///
    /// The existing log is fully re-verified against its own chain and against
    /// the persisted head checkpoint; any inconsistency -- an edited entry, a
    /// reorder, a truncation, a deleted head -- is reported as
    /// [`AuditError::TamperDetected`] so a tampered store cannot be silently
    /// reopened.
    pub fn open(root: &Path) -> Result<AuditLog, AuditError> {
        fs::create_dir_all(root).map_err(|e| AuditError::Io {
            context: format!("create store root {}", root.display()),
            source: e,
        })?;
        let log_path = root.join(LOG_FILE);
        let head_path = root.join(HEAD_FILE);

        // Storage-layer append-only enforcement: the log is only ever opened
        // with append(true). Nothing in this module truncates or rewrites it.
        let file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&log_path)
            .map_err(|e| AuditError::Io {
                context: format!("open log {}", log_path.display()),
                source: e,
            })?;

        let mut audit = AuditLog {
            root: root.to_path_buf(),
            log_path,
            head_path,
            file,
            entries: Vec::new(),
            head: HeadState::genesis(),
        };

        let (entries, chain_head) = audit.verify_chain()?;
        audit.entries = entries;
        let anchor = read_head(&audit.head_path).map_err(|e| AuditError::Io {
            context: format!("read head {}", audit.head_path.display()),
            source: e,
        })?;
        if chain_head != anchor {
            return Err(AuditError::TamperDetected {
                at: 0,
                detail: format!(
                    "head checkpoint disagrees with the log: log chain ends at seq {} (hash {}), anchored head is seq {} (hash {})",
                    chain_head.seq, chain_head.hash, anchor.seq, anchor.hash
                ),
            });
        }
        audit.head = anchor;
        Ok(audit)
    }

    /// Append one entry to the log and return the stored entry.
    ///
    /// Validates that both `actor` and `target` are non-blank, computes the
    /// chained hash, appends the JSON line with the single `O_APPEND` write
    /// handle, then atomically advances the head checkpoint. There is no way
    /// to amend or delete an entry once written.
    pub fn append(
        &mut self,
        actor: &str,
        action: Action,
        target: &str,
        detail: Option<&str>,
    ) -> Result<Entry, AuditError> {
        let actor = actor.trim();
        let target = target.trim();
        if actor.is_empty() {
            return Err(AuditError::EmptyField("actor"));
        }
        if target.is_empty() {
            return Err(AuditError::EmptyField("target"));
        }

        let seq = self.entries.len() as u64 + 1;
        let at_unix = unix_now();
        let payload = HashPayload {
            seq,
            actor: actor.to_owned(),
            action,
            target: target.to_owned(),
            at_unix,
            detail: detail.map(str::to_owned),
        };
        let prev = parse_hex64(&self.head.hash).ok_or(AuditError::Internal(
            "in-memory head hash is not a 64-hex digest",
        ))?;
        let hash = chain_hash(&prev, &payload);

        let entry = Entry {
            seq,
            actor: actor.to_owned(),
            action,
            target: target.to_owned(),
            detail: payload.detail.clone(),
            at_unix,
            prev_hash: self.head.hash.clone(),
            hash: hash.clone(),
        };

        let mut line =
            serde_json::to_string(&entry).map_err(|_| AuditError::Internal("serialize entry"))?;
        line.push('\n');

        self.file
            .write_all(line.as_bytes())
            .map_err(|e| AuditError::Io {
                context: "append entry to audit.log".into(),
                source: e,
            })?;
        self.file.flush().map_err(|e| AuditError::Io {
            context: "flush audit.log".into(),
            source: e,
        })?;

        let new_head = HeadState {
            seq,
            hash,
            count: seq,
        };
        write_head(&self.head_path, &new_head)?;
        self.head = new_head;
        self.entries.push(entry.clone());
        Ok(entry)
    }

    /// Record a read-access event (sovereignty: reads are audited too).
    pub fn record_read(&mut self, actor: &str, target: &str) -> Result<Entry, AuditError> {
        self.append(actor, Action::Read, target, None)
    }

    /// Record a change (create/modify) event.
    pub fn record_change(
        &mut self,
        actor: &str,
        target: &str,
        detail: Option<&str>,
    ) -> Result<Entry, AuditError> {
        self.append(actor, Action::Change, target, detail)
    }

    /// Record a release (lifecycle gate) event, e.g. with `detail` = `"maturity=final"`.
    pub fn record_release(
        &mut self,
        actor: &str,
        target: &str,
        detail: Option<&str>,
    ) -> Result<Entry, AuditError> {
        self.append(actor, Action::Release, target, detail)
    }

    /// Re-derive the whole chain from disk and confirm it is intact.
    ///
    /// This is the public, callable guardian: an operator or watchdog process
    /// can call it repeatedly on a live store. It returns the entries on
    /// success, or [`AuditError::TamperDetected`]/[`AuditError::CorruptRecord`]
    /// (`Err`) at the first sign of tampering. A clean [`AuditError::Io`] is
    /// reported if the store cannot be read.
    pub fn verify(&self) -> Result<Vec<Entry>, AuditError> {
        let (entries, chain_head) = self.verify_chain()?;
        if chain_head != self.head {
            return Err(AuditError::TamperDetected {
                at: 0,
                detail: format!(
                    "head checkpoint diverged from the log: log ends at seq {} (hash {}), anchored head is seq {} (hash {})",
                    chain_head.seq, chain_head.hash, self.head.seq, self.head.hash
                ),
            });
        }
        Ok(entries)
    }

    /// Read the log file and re-verify its hash chain end to end.
    ///
    /// Returns the entries plus the chain head implied by the log alone
    /// (not yet cross-checked against the head checkpoint).
    fn verify_chain(&self) -> Result<(Vec<Entry>, HeadState), AuditError> {
        let file = File::open(&self.log_path).map_err(|e| AuditError::Io {
            context: format!("open {}", self.log_path.display()),
            source: e,
        })?;
        let reader = BufReader::new(file);
        let mut entries: Vec<Entry> = Vec::new();
        let mut prev = [0u8; 32];

        for (line_no, line) in (1_u64..).zip(reader.lines()) {
            let line = line.map_err(|e| AuditError::Io {
                context: format!("read line {line_no} of {}", self.log_path.display()),
                source: e,
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let entry: Entry =
                serde_json::from_str(&line).map_err(|e| AuditError::CorruptRecord {
                    line: line_no,
                    detail: format!("not a valid audit entry: {e}"),
                })?;

            let expected_seq = entries.len() as u64 + 1;
            if entry.seq != expected_seq {
                return Err(AuditError::TamperDetected {
                    at: line_no,
                    detail: format!(
                        "out-of-order sequence: expected {expected_seq}, found {}",
                        entry.seq
                    ),
                });
            }
            if entry.prev_hash != hex_str(&prev) {
                return Err(AuditError::TamperDetected {
                    at: line_no,
                    detail: format!(
                        "entry {} does not chain from the previous hash {}",
                        entry.seq,
                        hex_str(&prev)
                    ),
                });
            }
            let recomputed = chain_hash(&prev, &entry.payload());
            if entry.hash != recomputed {
                return Err(AuditError::TamperDetected {
                    at: line_no,
                    detail: format!(
                        "entry {} hash mismatch: stored {} != recomputed {recomputed}",
                        entry.seq, entry.hash
                    ),
                });
            }
            prev = parse_hex64(&entry.hash).ok_or_else(|| AuditError::CorruptRecord {
                line: line_no,
                detail: "entry hash is not a 64-hex digest".into(),
            })?;
            entries.push(entry);
        }

        let chain_head = HeadState {
            seq: entries.last().map(|e| e.seq).unwrap_or(0),
            hash: hex_str(&prev),
            count: entries.len() as u64,
        };
        Ok((entries, chain_head))
    }

    /// The store root this log lives under.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Absolute path of the append-only log file (for operators/backup).
    pub fn log_path(&self) -> &Path {
        &self.log_path
    }

    /// Absolute path of the head-checkpoint file.
    pub fn head_path(&self) -> &Path {
        &self.head_path
    }

    /// All entries currently known (from open plus appends by this handle).
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the log has no entries yet.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The chained hash of the last entry (`GENESIS_HEX` when empty).
    pub fn head_hash(&self) -> &str {
        &self.head.hash
    }

    /// True by construction: the storage layer is append-only.
    ///
    /// The log file is only ever opened with `append(true)`; this module
    /// exposes no update or delete operation, and appending never rewrites
    /// previously written bytes (verified in the acceptance tests).
    pub fn append_only(&self) -> bool {
        true
    }
}

/// Read the persisted head checkpoint, defaulting to genesis when absent.
fn read_head(path: &Path) -> io::Result<HeadState> {
    let raw = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(HeadState::genesis()),
        Err(e) => return Err(e),
    };
    serde_json::from_str(raw.trim()).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid head checkpoint \"{}\": {e}", path.display()),
        )
    })
}

/// Atomically replace the head checkpoint (write temp, then rename).
fn write_head(path: &Path, state: &HeadState) -> Result<(), AuditError> {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(HEAD_FILE);
    let tmp = path.with_file_name(format!("{name}.tmp"));
    let mut body =
        serde_json::to_string(state).map_err(|_| AuditError::Internal("serialize head"))?;
    body.push('\n');
    fs::write(&tmp, body).map_err(|e| AuditError::Io {
        context: format!("write head temp {}", tmp.display()),
        source: e,
    })?;
    fs::rename(&tmp, path).map_err(|e| AuditError::Io {
        context: format!("commit head checkpoint {}", path.display()),
        source: e,
    })?;
    Ok(())
}

/// The chained hash for an entry: SHA-256 over the previous hash plus the
/// canonical length-prefixed payload bytes.
///
/// ```text
/// SHA-256( prev(32) || seq(8 BE) || actor_len(4 BE) || actor
///        || action_tag(1) || target_len(4 BE) || target
///        || at_unix(8 BE) || detail_flag(1)
///        || [ detail_len(4 BE) || detail ]              only when detail set )
/// ```
///
/// This byte layout is deliberate and versioned in `P8-M3/audit-log.md` so a
/// future format revision can migrate the chain without guessing. It is
/// independent of the JSON line encoding, so verification can recompute the
/// hash from the parsed entry fields alone.
fn chain_hash(prev: &[u8; 32], payload: &HashPayload) -> String {
    let mut h = Sha256::new();
    h.update(prev);
    h.update(payload.seq.to_be_bytes());
    h.update((payload.actor.len() as u32).to_be_bytes());
    h.update(payload.actor.as_bytes());
    h.update([payload.action.tag()]);
    h.update((payload.target.len() as u32).to_be_bytes());
    h.update(payload.target.as_bytes());
    h.update(payload.at_unix.to_be_bytes());
    match &payload.detail {
        None => h.update([0u8]),
        Some(detail) => {
            h.update([1u8]);
            h.update((detail.len() as u32).to_be_bytes());
            h.update(detail.as_bytes());
        }
    }
    hex_str(&h.finalize())
}

/// Hex-encode bytes (lowercase, no separators).
fn hex_str(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Decode a 64-hex SHA-256 digest.
fn parse_hex64(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, pair) in s.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let hi = (pair[0] as char).to_digit(16)?;
        let lo = (pair[1] as char).to_digit(16)?;
        out[i] = ((hi << 4) | lo) as u8;
    }
    Some(out)
}

/// Current unix seconds (0 if the system clock is before the epoch).
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(
        seq: u64,
        actor: &str,
        action: Action,
        target: &str,
        detail: Option<&str>,
    ) -> HashPayload {
        HashPayload {
            seq,
            actor: actor.to_owned(),
            action,
            target: target.to_owned(),
            at_unix: 1_700_000_000,
            detail: detail.map(str::to_owned),
        }
    }

    #[test]
    fn chain_hash_couples_to_previous_hash() {
        let p = payload(1, "alice", Action::Read, "bracket", None);
        let h_prev_zero = chain_hash(&[0u8; 32], &p);
        let h_prev_other = chain_hash(&[1u8; 32], &p);
        // Same payload, different predecessor => different hash (the "chaining" property).
        assert_ne!(h_prev_zero, h_prev_other);
        let h2 = payload(2, "bob", Action::Change, "bracket", Some("x"));
        assert_ne!(h_prev_zero, chain_hash(&[0u8; 32], &h2));
    }

    #[test]
    fn hex_roundtrip() {
        let digest = chain_hash(&[0u8; 32], &payload(1, "a", Action::Release, "t", None));
        assert_eq!(digest.len(), 64);
        let bytes = parse_hex64(&digest).unwrap();
        assert_eq!(hex_str(&bytes), digest);
        assert!(parse_hex64("zz").is_none());
        assert!(parse_hex64(&digest[..63]).is_none());
    }

    #[test]
    fn action_tags_and_names() {
        assert_eq!(Action::Read.as_str(), "read");
        assert_eq!(Action::Change.as_str(), "change");
        assert_eq!(Action::Release.as_str(), "release");
        assert_eq!(Action::Read.tag(), 0);
        assert_eq!(Action::Change.tag(), 1);
        assert_eq!(Action::Release.tag(), 2);
    }

    #[test]
    fn genesis_head_is_all_zeros() {
        assert_eq!(GENESIS_HEX.len(), 64);
        assert!(parse_hex64(GENESIS_HEX).is_some());
        assert_eq!(
            HeadState::genesis(),
            HeadState {
                seq: 0,
                hash: GENESIS_HEX.to_string(),
                count: 0,
            }
        );
    }
}
