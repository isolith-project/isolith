//! 3DXML container **BOM**: the `Manifest.xml` index inside a ZIP-archive 3DXML.
//!
//! The manifest is the container's bill of materials: it names the primary
//! model document and carries optional authoring metadata. Observed shape on
//! the 2026-10-15 corpus (`docs/re-notes/3dxml-hex-notebook.md` §3):
//!
//! ```xml
//! <?xml version="1.0" encoding="utf-8" ?>
//! <Manifest xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
//!           xsi:noNamespaceSchemaLocation="Manifest.xsd">
//!   <Root>Abaqus_Model.3dxml</Root>      <!-- required: primary model doc -->
//!   <Date>2017-12-14</Date>              <!-- optional, CATIA V5 family -->
//!   <WithAuthoringData>false</WithAuthoringData> <!-- optional, CATIA V5 family -->
//! </Manifest>
//! ```
//!
//! Line endings are LF (Abaqus writer) or CRLF (CATIA V5 writer); the CATIA
//! family may also write the whole document on one line. That is the whole
//! vocabulary: a flat list of text-only elements under a `<Manifest>` root.
//! We parse it with a tiny, fully-tested std-only recursive scanner instead
//! of a full XML parser — the full `<Model_3dxml>` model documents
//! (P3-M1-T02) and rep XML (P3-M1-T03) are where a real XML engine belongs.
//! This module stays bounded to the manifest shape, returns typed errors on
//! anything unexpected, and never panics.

/// The parsed `Manifest.xml` (container BOM).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Value of the required `<Root>` element: the member name of the primary
    /// model document (e.g. `NameSetTree.3dxml` or `Abaqus_Model.3dxml`).
    pub root: String,
    /// Optional `<Date>` value (ISO date string), `None` when absent.
    pub date: Option<String>,
    /// Optional `<WithAuthoringData>` value, `None` when absent.
    pub with_authoring_data: Option<bool>,
}

/// Errors from parsing a `Manifest.xml`. Typed and non-panicking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The manifest bytes are not valid UTF-8 (spec-violating; observed
    /// manifests are ASCII XML).
    InvalidUtf8,
    /// A required construct is absent or structurally malformed for this
    /// bounded scanner (e.g. no `<Root>`, an unterminated tag).
    MissingElement {
        /// The name of what was expected.
        name: &'static str,
    },
    /// `<WithAuthoringData>` is present but is not a boolean token
    /// (`true`/`false`, case-sensitive per observed material).
    InvalidAuthoringData {
        /// The raw text actually found.
        got: String,
    },
}

// ---------------------------------------------------------------------------
// Tiny byte-level XML scanner (bounded to the manifest shape)
// ---------------------------------------------------------------------------

fn is_name_terminator(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'>')
}

/// Offset of `needle` at or after `from` (and strictly below `limit`), or
/// `None`.
fn find_after(hay: &[u8], needle: &[u8], from: usize, limit: usize) -> Option<usize> {
    if from >= limit {
        return None;
    }
    let window = &hay[from..limit.min(hay.len())];
    (window.len() >= needle.len())
        .then(|| {
            window
                .windows(needle.len())
                .position(|w| w == needle)
                .map(|p| from + p)
        })
        .flatten()
}

/// Byte range of an element/close-tag name starting at `start` (right after
/// `<` or `</`).
fn read_name(hay: &[u8], start: usize, limit: usize) -> Option<(usize, usize)> {
    let mut end = start;
    while end < limit && end < hay.len() && !is_name_terminator(hay[end]) {
        end += 1;
    }
    (end > start).then_some((start, end))
}

/// Drop an XML namespace prefix (`ns:local` -> `local`) from a name slice.
fn local_of(name: &[u8]) -> &[u8] {
    match name.iter().position(|&b| b == b':') {
        Some(idx) => &name[idx + 1..],
        None => name,
    }
}

/// Offset of a tag's terminating `>` (quote-aware), scanning from `from`
/// (bounded by `limit`).
fn tag_end(hay: &[u8], mut from: usize, limit: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    while from < limit && from < hay.len() {
        let b = hay[from];
        if let Some(q) = quote {
            if b == q {
                quote = None;
            }
        } else if b == b'"' || b == b'\'' {
            quote = Some(b);
        } else if b == b'>' {
            return Some(from);
        }
        from += 1;
    }
    None
}

/// `true` when the open tag whose name ends at `name_end` and whose `>` is at
/// `gt` is self-closing (`/>`).
fn is_self_closing(hay: &[u8], mut j: usize, gt: usize) -> bool {
    while j < gt && matches!(hay[j], b' ' | b'\t' | b'\r' | b'\n') {
        j += 1;
    }
    j < gt && hay[j] == b'/'
}

/// Scan the next element open tag `<NAME ...>` in `hay[from..limit)`.
///
/// Returns `(name_range, tag_after_gt, self_closing)`. Skips XML
/// declarations (`<?...?>`), comments (`<!--...-->`) and standalone `</...>`
/// close tags. `Ok(None)` when no more open tags exist in the range.
fn next_open_tag(
    hay: &[u8],
    mut from: usize,
    limit: usize,
) -> Result<Option<(std::ops::Range<usize>, usize, bool)>, ManifestError> {
    while from < limit {
        let Some(lt) = find_after(hay, b"<", from, limit) else {
            return Ok(None);
        };
        if lt + 1 < limit && lt + 1 < hay.len() && hay[lt + 1] == b'?' {
            let end = find_after(hay, b"?>", lt + 2, limit)
                .ok_or(ManifestError::MissingElement { name: "xml-decl" })?;
            from = end + 2;
            continue;
        }
        if lt + 1 < limit && lt + 1 < hay.len() && hay[lt + 1] == b'!' {
            let term: &[u8] = if hay[lt..].starts_with(b"<!--") {
                b"-->"
            } else {
                b">"
            };
            let end = find_after(hay, term, lt + 2, limit)
                .ok_or(ManifestError::MissingElement { name: "comment" })?;
            from = end + term.len();
            continue;
        }
        if lt + 1 >= hay.len() {
            return Err(ManifestError::MissingElement { name: "tag" });
        }
        if hay[lt + 1] == b'/' {
            // A close tag; skip past its `>` without treating it as an open.
            let gt = tag_end(hay, lt + 2, limit)
                .ok_or(ManifestError::MissingElement { name: "close-tag" })?;
            from = gt + 1;
            continue;
        }
        let (name_start, name_end) = read_name(hay, lt + 1, limit)
            .ok_or(ManifestError::MissingElement { name: "tag-name" })?;
        let gt = tag_end(hay, name_end, limit)
            .ok_or(ManifestError::MissingElement { name: "tag-end" })?;
        let self_closing = is_self_closing(hay, name_end, gt);
        return Ok(Some((name_start..name_end, gt + 1, self_closing)));
    }
    Ok(None)
}

/// Offset of the `<` that opens the MATCHING close tag for the element whose
/// open tag ends at `from` (just past its `>`) and whose local name is `local`,
/// bounded by `limit`. Depth-aware: nested same-name elements count against
/// the depth. `Ok(None)` when no well-formed close is found in range.
fn matching_close_offset(
    hay: &[u8],
    mut from: usize,
    local: &[u8],
    limit: usize,
) -> Result<Option<usize>, ManifestError> {
    let mut depth = 1usize;
    while from < limit {
        let Some(lt) = find_after(hay, b"<", from, limit) else {
            return Ok(None);
        };
        if lt + 1 >= hay.len() {
            return Ok(None);
        }
        let c = hay[lt + 1];
        if c == b'?' || c == b'!' {
            let term: &[u8] = if c == b'?' {
                b"?>"
            } else if hay[lt..].starts_with(b"<!--") {
                b"-->"
            } else {
                b">"
            };
            let end = match find_after(hay, term, lt + 2, limit) {
                Some(p) => p + term.len(),
                None => return Ok(None),
            };
            from = end;
            continue;
        }
        let is_close = c == b'/';
        let (name_start, name_end) = match read_name(hay, lt + 1 + usize::from(is_close), limit) {
            Some(r) => r,
            None => return Ok(None),
        };
        let gt = match tag_end(hay, name_end, limit) {
            Some(gt) => gt,
            None => return Ok(None),
        };
        if local_of(&hay[name_start..name_end]) == local {
            if is_close {
                depth -= 1;
                if depth == 0 {
                    return Ok(Some(lt));
                }
            } else if !is_self_closing(hay, name_end, gt) {
                depth += 1;
            }
        }
        from = gt + 1;
    }
    Ok(None)
}

/// Recursively look for an element with local name `target` inside the byte
/// range `hay[from..limit)` of `xml`, returning its trimmed, entity-decoded
/// text content. Non-target elements are descended into (the `<Manifest>`
/// root wraps the fields), so the search is not limited to the top nesting
/// level. Terminates because every recursion enters a strictly smaller range.
fn element_in_range(
    xml: &str,
    from: usize,
    limit: usize,
    target: &str,
) -> Result<Option<String>, ManifestError> {
    let hay = xml.as_bytes();
    let mut p = from;
    while let Some((name_range, tag_after_gt, self_closing)) = next_open_tag(hay, p, limit)? {
        let local = local_of(&hay[name_range]);
        if local == target.as_bytes() {
            if self_closing {
                return Ok(Some(String::new()));
            }
            let close_lt = matching_close_offset(hay, tag_after_gt, local, limit)?
                .ok_or(ManifestError::MissingElement { name: "close-tag" })?;
            let raw = &xml[tag_after_gt..close_lt];
            return Ok(Some(decode_entities(raw).trim().to_string()));
        }
        if self_closing {
            p = tag_after_gt;
            continue;
        }
        let close_lt = matching_close_offset(hay, tag_after_gt, local, limit)?
            .ok_or(ManifestError::MissingElement { name: "close-tag" })?;
        // Descend: the target may be nested inside a non-target wrapper
        // (that is how every manifest is shaped: fields inside <Manifest>).
        if let Some(hit) = element_in_range(xml, tag_after_gt, close_lt, target)? {
            return Ok(Some(hit));
        }
        let gt = tag_end(hay, close_lt, limit)
            .ok_or(ManifestError::MissingElement { name: "close-tag" })?;
        p = gt + 1;
    }
    Ok(None)
}

/// Extract the trimmed, entity-decoded text content of the FIRST element with
/// local name `target` anywhere in `xml`, or `Ok(None)` when absent.
fn element_content(xml: &str, target: &str) -> Result<Option<String>, ManifestError> {
    element_in_range(xml, 0, xml.len(), target)
}

/// Decode the five basic XML entities (`&amp; &lt; &gt; &quot; &apos;`).
/// Numeric/unknown entities are passed through untouched (not observed in
/// manifests; harmless).
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let tail = &rest[amp..];
        let (escaped, len) = if tail.starts_with("&amp;") {
            ("&", 5)
        } else if tail.starts_with("&lt;") {
            ("<", 4)
        } else if tail.starts_with("&gt;") {
            (">", 4)
        } else if tail.starts_with("&quot;") {
            ("\"", 6)
        } else if tail.starts_with("&apos;") {
            ("'", 6)
        } else {
            ("&", 1)
        };
        out.push_str(escaped);
        rest = &tail[len..];
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse the container BOM from the decoded `Manifest.xml` bytes.
///
/// Returns the required `<Root>` (primary model document member name) plus
/// the optional `<Date>` and `<WithAuthoringData>`. Rejected with a typed
/// [`ManifestError`] — never a panic — when the input is not UTF-8, `<Root>`
/// is absent, or `<WithAuthoringData>` is not a boolean token.
pub fn parse_manifest(xml: &[u8]) -> Result<Manifest, ManifestError> {
    let text = std::str::from_utf8(xml).map_err(|_| ManifestError::InvalidUtf8)?;
    let root =
        element_content(text, "Root")?.ok_or(ManifestError::MissingElement { name: "Root" })?;
    let date = element_content(text, "Date")?;
    let with_authoring_data = match element_content(text, "WithAuthoringData")? {
        None => None,
        Some(v) => match v.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            other => {
                return Err(ManifestError::InvalidAuthoringData {
                    got: other.to_string(),
                })
            }
        },
    };
    Ok(Manifest {
        root,
        date,
        with_authoring_data,
    })
}
