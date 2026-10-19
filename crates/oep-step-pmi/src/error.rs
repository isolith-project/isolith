//! Typed errors for the oep-step-pmi import surface.
//!
//! Tolerant-failure doctrine (P2-M1-T04): the importer never panics on bad
//! input; every failure is a typed [`Error`] a caller can match on.

use std::fmt;

/// A typed import failure. Never panics; always a matching decision.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The file could not be read from disk as ISO 10303-21 bytes.
    Io(String),
    /// The bytes do not look like an ISO 10303-21 STEP physical file
    /// (missing the `ISO-10303-21` header line).
    NotStep(String),
    /// The STEP file parsed but the semantic PMI mapping is not possible:
    /// the PMI model carries no geometric-tolerance/datum/dimension surface.
    NoPmi(String),
    /// The wrapped OSS STEP library rejected the file (lex/parse/read error)
    /// or the degenerate model could not be walked.
    Parse(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(detail) => write!(f, "oep-step-pmi io: {detail}"),
            Error::NotStep(detail) => write!(f, "oep-step-pmi not a STEP file: {detail}"),
            Error::NoPmi(detail) => write!(f, "oep-step-pmi no PMI in file: {detail}"),
            Error::Parse(detail) => write!(f, "oep-step-pmi step parse failed: {detail}"),
        }
    }
}

impl std::error::Error for Error {}
