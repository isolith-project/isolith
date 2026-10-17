//! Typed, tolerant failure model for the oep-step reader.
//!
//! The reader never panics on input: every failure mode is a variant of
//! [`Error`] with a documented meaning, matching the program's
//! tolerant-failure doctrine (P2-M1-T04).

use std::fmt;

/// A typed parse failure. Every variant is produced deliberately; the reader
/// never panics on junk, truncated, or unknown bytes.
#[derive(Debug)]
pub enum Error {
    /// Reading the byte source failed (I/O layer).
    Io(String),
    /// The bytes are not an ISO 10303-21 STEP physical file (no
    /// `ISO-10303-21;` header marker).
    NotStep,
    /// The underlying OSS STEP library rejected the payload.
    Parse(String),
    /// The bytes parsed as STEP but contain no faceted geometry this reader
    /// maps (no shell-based surface model found).
    NoFacetedGeometry(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(msg) => write!(f, "STEP I/O error: {msg}"),
            Error::NotStep => write!(f, "not an ISO 10303-21 STEP physical file"),
            Error::Parse(msg) => write!(f, "STEP parse error: {msg}"),
            Error::NoFacetedGeometry(msg) => write!(f, "no faceted geometry in STEP part: {msg}"),
        }
    }
}

impl std::error::Error for Error {}
