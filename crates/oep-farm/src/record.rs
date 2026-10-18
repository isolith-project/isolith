//! The durable progress record: a [Checkpoint].
//!
//! While a long job runs, the worker periodically writes a checkpoint that
//! says, in effect, *"items `0..next_item` are complete and the artifact
//! output bytes `0..output_len` are exactly what they should be."* The
//! `next_item` is the resume point; `output_len` + `output_fnv` are the
//! byte-level integrity check a resumed worker performs on the already-written
//! artifact prefix before it continues (byte-equivalence gate).
//!
//! The wire format is self-contained and deterministic (std-only doctrine):
//!
//! ```text
//! magic      : "OEPF" (4 bytes)
//! version    : u32 LE
//! job_id_len : u32 LE
//! job_id     : job_id_len bytes (UTF-8)
//! next_item  : u64 LE
//! output_len : u64 LE
//! output_fnv : u64 LE   (FNV-1a64 over the artifact prefix [0..output_len))
//! integrity  : u64 LE   (FNV-1a64 over all the preceding bytes)
//! ```
//!
//! Every field is fixed-width except the length-prefixed job id, so a decode
//! is bounds-checked and a mismatch in magic/version/length/integrity fails
//! loudly ([`CheckpointError`]) instead of being silently misread.

use crate::hash::fnv1a64;

/// Canonical `format` header identifying this checkpoint format.
pub const FORMAT: &str = "oep-farm";
/// Current schema version of the checkpoint format.
pub const VERSION: u32 = 1;

/// The four-byte magic that opens a serialized checkpoint.
pub const MAGIC: [u8; 4] = *b"OEPF";

/// A durable progress record for one farm job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// Unique job identity (matches the scheduler's job / idempotency key).
    pub job_id: String,
    /// Number of work items completed; the next item to run is `next_item`.
    pub next_item: u64,
    /// Bytes of artifact output durably written so far (prefix length).
    pub output_len: u64,
    /// FNV-1a64 over `artifact[0..output_len)` - the byte-equivalence gate.
    pub output_fnv: u64,
}

impl Checkpoint {
    /// Build a checkpoint from a fresh run state.
    pub fn new(
        job_id: impl Into<String>,
        next_item: u64,
        output_len: u64,
        output_fnv: u64,
    ) -> Self {
        Checkpoint {
            job_id: job_id.into(),
            next_item,
            output_len,
            output_fnv,
        }
    }

    /// Serialize to the deterministic wire format (see module docs).
    pub fn encode(&self) -> Vec<u8> {
        let body = self.encode_body();
        let mut out = Vec::with_capacity(body.len() + 8);
        out.extend_from_slice(&body);
        out.extend_from_slice(&fnv1a64(&body).to_le_bytes());
        out
    }

    /// Parse a checkpoint from the wire format, validating magic, version,
    /// bounds and the integrity tag.
    pub fn decode(bytes: &[u8]) -> Result<Self, CheckpointError> {
        if bytes.len() < 4 + 4 + 4 + 8 + 8 + 8 {
            return Err(CheckpointError::Truncated);
        }
        if bytes[0..4] != MAGIC {
            return Err(CheckpointError::BadMagic);
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != VERSION {
            return Err(CheckpointError::BadVersion(version));
        }
        let body = &bytes[..bytes.len() - 8];
        let expect_integrity = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap());
        if fnv1a64(body) != expect_integrity {
            return Err(CheckpointError::BadIntegrity);
        }
        let (job_id, next_item, output_len, output_fnv) = Self::decode_body(body)?;
        // Sanity: output_len is the length of a prefix that must exist.
        if output_len > 0 && output_fnv == 0 {
            return Err(CheckpointError::BadIntegrity);
        }
        Ok(Checkpoint {
            job_id,
            next_item,
            output_len,
            output_fnv,
        })
    }

    /// Field bytes without the trailing integrity word.
    fn encode_body(&self) -> Vec<u8> {
        let id = self.job_id.as_bytes();
        let mut out = Vec::with_capacity(4 + 4 + 4 + id.len() + 8 + 8 + 8);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        out.extend_from_slice(&(id.len() as u32).to_le_bytes());
        out.extend_from_slice(id);
        out.extend_from_slice(&self.next_item.to_le_bytes());
        out.extend_from_slice(&self.output_len.to_le_bytes());
        out.extend_from_slice(&self.output_fnv.to_le_bytes());
        out
    }

    /// Parse the field bytes (the trailing integrity tag already checked).
    fn decode_body(body: &[u8]) -> Result<(String, u64, u64, u64), CheckpointError> {
        // magic + version
        let id_len = u32::from_le_bytes(body[8..12].try_into().unwrap()) as usize;
        let id_end = 12 + id_len;
        if id_end > body.len() {
            return Err(CheckpointError::Truncated);
        }
        let job_id =
            String::from_utf8(body[12..id_end].to_vec()).map_err(|_| CheckpointError::BadJobId)?;
        let tail = &body[id_end..];
        if tail.len() != 24 {
            return Err(CheckpointError::Truncated);
        }
        let next_item = u64::from_le_bytes(tail[0..8].try_into().unwrap());
        let output_len = u64::from_le_bytes(tail[8..16].try_into().unwrap());
        let output_fnv = u64::from_le_bytes(tail[16..24].try_into().unwrap());
        Ok((job_id, next_item, output_len, output_fnv))
    }
}

/// Errors from parsing a serialized checkpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointError {
    /// The byte slice is too short to hold a full checkpoint.
    Truncated,
    /// The leading four bytes are not the `OEPF` magic.
    BadMagic,
    /// The version field is not [VERSION].
    BadVersion(u32),
    /// The job id is not valid UTF-8.
    BadJobId,
    /// The trailing integrity tag does not match the body.
    BadIntegrity,
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckpointError::Truncated => write!(f, "checkpoint bytes truncated"),
            CheckpointError::BadMagic => write!(f, "bad checkpoint magic (not OEPF)"),
            CheckpointError::BadVersion(v) => write!(f, "unsupported checkpoint version {v}"),
            CheckpointError::BadJobId => write!(f, "checkpoint job id is not UTF-8"),
            CheckpointError::BadIntegrity => write!(f, "checkpoint integrity tag mismatch"),
        }
    }
}

impl std::error::Error for CheckpointError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_deterministically() {
        let ck = Checkpoint::new("render-wave-7", 12_345, 4096, 0xdead_beef_0123_4567);
        let enc = ck.encode();
        let dec = Checkpoint::decode(&enc).unwrap();
        assert_eq!(dec, ck);
        // Deterministic serialization: same object, same bytes each time.
        assert_eq!(enc, ck.encode());
    }

    #[test]
    fn rejects_corruption() {
        let ck = Checkpoint::new("job", 3, 24, 99);
        let mut enc = ck.encode();
        let last = enc.len() - 1;
        enc[last] ^= 0xff;
        assert!(matches!(
            Checkpoint::decode(&enc),
            Err(CheckpointError::BadIntegrity)
        ));
        // Truncation is caught.
        assert!(matches!(
            Checkpoint::decode(&ck.encode()[..20]),
            Err(CheckpointError::Truncated)
        ));
    }

    #[test]
    fn rejects_bad_magic_and_version() {
        let ck = Checkpoint::new("job", 1, 8, 1);
        let mut enc = ck.encode();
        enc[0] = b'X';
        assert!(matches!(
            Checkpoint::decode(&enc),
            Err(CheckpointError::BadMagic)
        ));
        let ck = Checkpoint::new("j2", 1, 8, 1);
        let mut enc = ck.encode();
        enc[4..8].copy_from_slice(&99u32.to_le_bytes());
        assert!(matches!(
            Checkpoint::decode(&enc),
            Err(CheckpointError::BadVersion(99))
        ));
    }

    #[test]
    fn accepts_zero_progress_checkpoint() {
        // A "start of job" checkpoint (nothing done yet) is valid.
        let ck = Checkpoint::new("fresh", 0, 0, 0);
        assert_eq!(Checkpoint::decode(&ck.encode()).unwrap(), ck);
    }
}
