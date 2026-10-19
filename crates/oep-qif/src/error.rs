//! Typed errors + tolerant skip reasons for the QIF basic cut.

use quick_xml::Error as XmlError;
use std::fmt;
use std::io;

/// Result alias for the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Typed crate error. Identifies the unacceptable input: never a panic path,
/// never a "something went wrong" that hides what the caller can act on.
#[derive(Debug)]
pub enum Error {
    /// Bytes do not carry a QIF 3.0 document (missing QIFDocument root in the
    /// qif3 namespace or missing versionQIF).
    NotQif(String),
    /// XML is malformed / not well-formed (delegate quick-xml error).
    Xml(String),
    /// Required QIF document content is missing (e.g. no QPId, no idMax).
    Missing(String),
    /// The document declares a QIF version this crate does not handle.
    UnsupportedVersion(String),
    /// An element essential to the modeled structure repeated when it must be
    /// unique (e.g. two QPId elements).
    Duplicate(String),
    /// A modeled value failed to convert (id not a number, etc.).
    Value(String),
    /// I/O failure while reading.
    Io(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotQif(s) => write!(f, "not a QIF document: {s}"),
            Error::Xml(s) => write!(f, "XML error: {s}"),
            Error::Missing(s) => write!(f, "missing required QIF content: {s}"),
            Error::UnsupportedVersion(s) => write!(f, "unsupported QIF version: {s}"),
            Error::Duplicate(s) => write!(f, "duplicate QIF element: {s}"),
            Error::Value(s) => write!(f, "invalid QIF value: {s}"),
            Error::Io(s) => write!(f, "I/O error: {s}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<XmlError> for Error {
    /// quick-xml only fails on genuinely malformed XML / IO; pass through with
    /// enough context to debug (never a panic).
    fn from(e: XmlError) -> Self {
        match &e {
            XmlError::Io(io) => Error::Io(io.to_string()),
            other => Error::Xml(other.to_string()),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

/// One skipped out-of-cut section, recorded during a tolerant parse.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SkipNote {
    /// Element local name that was skipped (e.g. "GeometrySet").
    pub element: String,
    /// Short reason (one of the documented skip reasons from the scope doc).
    pub reason: String,
}
