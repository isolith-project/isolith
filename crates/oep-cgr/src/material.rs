//! P2-M4-T01 — CGR color/material table parse and glTF 2.0 materials export.
//!
//! Derives ONLY from observed byte structure of real CGR files (see
//! `docs/re-notes/cgr-color-material-observation.md`); there is no public
//! Dassault spec for the CGR container. On the R19-era cfysa family
//! (2026-10-15 observation) every parsed sample that carries surface colors
//! does so through records whose head is:
//!
//! ```text
//! offset  size  value
//! +0      4     f32 LE color scalar c, c in (0.5, 1.0]
//! +4      4     marker word A: 00 00 00 01  (u32 LE == 0x01000000)
//! +8      4     marker word B: 00 01 00 00  (u32 LE == 0x100)
//! ```
//!
//! The leading scalar is read as a grayscale diffuse component: `[uncertain]`
//! whether the file stores pure grayscale or an RGB family the scalar
//! quantises; VALVE.cgr constant `0.812072` is consistent with CATIA default
//! part gray (~0.808). The exporter maps it to a glTF 2.0
//! `pbrMetallicRoughness.baseColorFactor` of `[s, s, s, 1.0]` (public Khronos
//! spec, deterministic). Meaning of the record tail fields is NOT asserted
//! here (surface-attribute attribution is the P2-M3 cards' job).
//!
//! Tolerant by design (P2-M1-T04 heritage): the scanner never panics, skips
//! regions that do not fit the observed signature, and bounds-checks all
//! reads so it is safe to run over untrusted bytes (fuzz-friendly).

#![forbid(unsafe_code)]

/// A parsed color/material record head.
///
/// Holds the byte offset where the signature was found and the observed
/// leading f32 scalar (the per-surface color component).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorRecord {
    /// Byte offset of the record head within the scanned buffer.
    pub offset: usize,
    /// The f32 LE scalar read at the record head (diffuse color component).
    pub scalar: f32,
}

/// Marker word A immediately after the color scalar: `00 00 00 01` (u32 LE
/// == 0x01000000 as read from the real file bytes).
const MARKER_A: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
/// Marker word B at +8: `00 01 00 00` (u32 LE == 0x100).
const MARKER_B: [u8; 4] = [0x00, 0x01, 0x00, 0x00];
/// Minimum scalar accepted as a plausible color component (observation
/// bound: records carry scalars in the open interval (0.5, 1.0]); exactly
/// 0.5 is rejected, any value strictly above it is kept.
const MIN_SCALAR: f32 = 0.5;
/// Maximum scalar accepted as a plausible color component.
const MAX_SCALAR: f32 = 1.0 + 1e-6;
/// Record heads closer than this many bytes are coalesced (observed cluster
/// stride is 0xB4 = 180; overlapping windows at the same head dedupe).
const MIN_RECORD_SPACING: usize = 12;

/// Scan `bytes` for the observed color-record signature and return the
/// records found, in file order.
///
/// The scan walks every byte offset and accepts a window as a record head
/// when it matches the observed signature: an f32 LE in `(0.5, 1.0]`
/// followed immediately by `MARKER_A` and `MARKER_B`. Byte-level stepping is
/// required: on real material the record heads are NOT 4-byte-aligned in the
/// file (e.g. VALVE.cgr records at 0x010afb, 0x024193). The fixed CGR header
/// spine (established by P2-M1-T01) cannot match this signature — its f32
/// windows are outside the accepted band — so starting at 0 is safe.
///
/// Guarantees: never panics on any input (including empty/truncated/junk),
/// performs no out-of-bounds reads, and allocates only for the returned
/// vector. Unknown regions are ignored, never erroring (P2-M1-T04).
pub fn scan_color_records(bytes: &[u8]) -> Vec<ColorRecord> {
    let mut out = Vec::new();
    if bytes.len() < 12 {
        return out;
    }
    let mut pos = 0;
    while pos + 12 <= bytes.len() {
        let window = &bytes[pos..pos + 12];
        if is_record_head(window) {
            let scalar = f32::from_le_bytes([window[0], window[1], window[2], window[3]]);
            // Merge with the previous record when this is the same head seen
            // over again (offset moved by < MIN_RECORD_SPACING).
            if let Some(last) = out.last_mut() {
                if pos - last.offset < MIN_RECORD_SPACING {
                    last.offset = pos;
                    last.scalar = scalar;
                    pos += 4;
                    continue;
                }
            }
            out.push(ColorRecord {
                offset: pos,
                scalar,
            });
            pos += 12;
        } else {
            pos += 1;
        }
    }
    out
}

/// Is a 12-byte window an observed color-record head?
fn is_record_head(w: &[u8]) -> bool {
    let scalar = f32::from_le_bytes([w[0], w[1], w[2], w[3]]);
    // Reject NaN / infinities and out-of-range values first (a raw f32 from
    // file bytes can be any bit pattern).
    let scalar_in_range = scalar.is_finite() && scalar > MIN_SCALAR && scalar <= MAX_SCALAR;
    scalar_in_range && w[4..8] == MARKER_A && w[8..12] == MARKER_B
}

/// Emit a glTF 2.0 JSON `materials` array from parsed color records.
///
/// Each record becomes one material in file order with
/// `pbrMetallicRoughness.baseColorFactor = [s, s, s, 1.0]` (the observed
/// scalar mapped to a grayscale diffuse; alpha opaque), `metallicFactor 0.0`
/// and `roughnessFactor 1.0`, named `cgr-material-N`. Field names and value
/// types follow the public Khronos glTF 2.0 specification; the structural
/// contract is locked by `tests/golden_material.rs`.
///
/// Costs: the single returned `String` (the JSON document). No I/O.
pub fn to_gltf_materials_json(records: &[ColorRecord]) -> String {
    let mut s = String::with_capacity(64 + records.len() * 128);
    s.push_str("{\"materials\":[");
    for (i, rec) in records.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let c = rec.scalar as f64;
        s.push_str(&format!(
            "{{\"name\":\"cgr-material-{i}\",\"pbrMetallicRoughness\":{{\"baseColorFactor\":[{c:.6},{c:.6},{c:.6},1.0],\"metallicFactor\":0.0,\"roughnessFactor\":1.0}}}}"
        ));
    }
    s.push_str("]}");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_one_record() {
        let mut b = vec![0u8; 0x100];
        b[0x20..0x24].copy_from_slice(&0.812072f32.to_le_bytes());
        b[0x24..0x28].copy_from_slice(&MARKER_A);
        b[0x28..0x2c].copy_from_slice(&MARKER_B);
        let r = scan_color_records(&b);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].offset, 0x20);
        assert!((r[0].scalar - 0.812072).abs() < 1e-4);
    }

    #[test]
    fn rejects_edge_scalars() {
        let mk = |scalar: f32| {
            let mut b = vec![0u8; 0x80];
            b[0x20..0x24].copy_from_slice(&scalar.to_le_bytes());
            b[0x24..0x28].copy_from_slice(&MARKER_A);
            b[0x28..0x2c].copy_from_slice(&MARKER_B);
            scan_color_records(&b)
        };
        // 0.5 exactly is NOT accepted (observation was open interval).
        assert!(mk(0.5).is_empty());
        // 0.0 and 2.0 are out of the observed band.
        assert!(mk(0.0).is_empty());
        assert!(mk(2.0).is_empty());
        // NaN / infinity are rejected without panicking.
        assert!(mk(f32::NAN).is_empty());
        assert!(mk(f32::INFINITY).is_empty());
        // Just inside the band is accepted.
        assert_eq!(mk(0.6).len(), 1);
    }

    #[test]
    fn json_is_structurally_valid() {
        let recs = [
            ColorRecord {
                offset: 0x20,
                scalar: 0.812072,
            },
            ColorRecord {
                offset: 0x100,
                scalar: 0.9,
            },
        ];
        let j = to_gltf_materials_json(&recs);
        assert!(j.starts_with("{\"materials\":["));
        assert!(j.ends_with("]}"), "document closes object+array: {j}");
    }
}
