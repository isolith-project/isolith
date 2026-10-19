//! P3-M3-T01 acceptance test (TDD, RED first) — the binary `.3DRep` branch:
//! the native-V5 `V5_CFV3` container header.
//!
//! Written BEFORE `oep_3dxml::v5rep` existed (RED: this file does not compile
//! against the skeleton crate). The contract, once GREEN, is:
//!
//! 1. `parse_header` accepts the observed `V5_CFV3` header prefix (0x00..0x53)
//!    and returns the exact observed fields (big-endian lengths, count).
//! 2. Malformed headers (bad magic, truncated, marker/prelude/token/follower
//!    drift) are rejected with a typed error, never a panic. Whole-file
//!    validation also rejects a length-sum mismatch and a missing `CB__END`
//!    tail.
//! 3. Against REAL material, when reference-only binary `.3DRep` members are
//!    supplied, every one passes header parse + whole-file invariants (sum +
//!    tail); an ASCII `.3DRep` from the open corpus is rejected (branch
//!    discrimination intact). With no sample the real-material legs print a
//!    tolerant SKIP so CI stays green (the proprietary binary is never
//!    committed).
//!
//! Scope boundary: this card parses RAW `.3DRep` bytes only. Un-ZIPping the
//! outer `.3dxml` archive is P3-M1-T01's job (the ZIP/BOM card); the real
//! material here is supplied as extracted files, so this test stays pure-std
//! with NO external crates (the workspace rule; crates.io is not reachable
//! from this host).
//!
//! Clean-room rule honoured here: no proprietary `.3DRep` bytes are committed.
//! The deterministic tests construct input ONLY from observed format facts
//! already published in `docs/re-notes/3dxml-binary-3drep-format.md`
//! (magic `V5_CFV3\0`, BE length-sum invariant, `FF x8` marker, `_\r\n\0`
//! prelude, `CATIA_V5 CB0001\0` token, `CB__END\0` tail). The real-material
//! legs read actual binary from a private observation location:
//!   - `OEP_3DXML_BINARY_REPS`: colon/comma-separated dirs of `.3DRep` files
//!     (or individual files) for the binary branch;
//!   - `OEP_3DXML_ASCII_REPS`: colon/comma-separated dirs/files of ASCII
//!     (XML) `.3DRep` members for the discrimination leg.

use oep_3dxml::v5rep::{check_container, has_magic, parse_header, V5Error, HEADER_LEN};

/// Observed golden values — reference-only Dassault gallery sample
/// `press_rink_axe_fta.3dxml`, member `R125.3DRep` (P1-M2-T04 hex notebook,
/// verified on 18 real parts, 2026-10-15).
const GOLDEN_MEMBER: &str = "R125.3DRep";
const GOLDEN_SIZE: usize = 7014;
const GOLDEN_LEN_A: u32 = 6268;
const GOLDEN_LEN_B: u32 = 746;
const GOLDEN_COUNT: u16 = 1000;

/// Build a deterministic, fully-valid `V5_CFV3` container from OBSERVED facts
/// only (no proprietary payload): header prefix with the observed constants,
/// an arbitrary zeroed body, and the `CB__END\0` terminator. The two lengths
/// are chosen so their sum equals the constructed file size.
fn synthetic_reference() -> Vec<u8> {
    let body: usize = 0x200;
    let total = HEADER_LEN + body + 8; // prefix + body + CB__END tail
    let mut b = vec![0u8; total];
    b[0x00..0x08].copy_from_slice(b"V5_CFV3\x00");
    b[0x08..0x0C].copy_from_slice(&((total - 8) as u32).to_be_bytes()); // len A
    b[0x0C..0x10].copy_from_slice(&8u32.to_be_bytes()); // len B (tail, incl. CB__END)
    b[0x10..0x18].fill(0xFF); // FF x8 marker
    b[0x3C..0x40].copy_from_slice(&[0x5F, 0x0D, 0x0A, 0x00]); // _ CR LF NUL
    b[0x40..0x50].copy_from_slice(b"CATIA_V5 CB0001\x00"); // version token
    b[0x50] = b'N'; // token follower
    b[0x51..0x53].copy_from_slice(&1000u16.to_be_bytes()); // count
    b[total - 8..].copy_from_slice(b"CB__END\x00");
    b
}

#[test]
fn parse_accepts_synthetic_reference_shape() {
    let b = synthetic_reference();
    let h = parse_header(&b).expect("observed V5_CFV3 header shape must parse");
    assert_eq!(
        h.count, GOLDEN_COUNT,
        "count is u16 BE at 0x51, observed 1000"
    );
    assert_eq!(
        h.len_a + h.len_b,
        b.len() as u32,
        "len sum must equal file size (observed invariant)"
    );
    assert_eq!(h.len_b, 8, "synthetic tail length (CB__END only)");
    assert_eq!(h.len_a, (b.len() - 8) as u32);
}

#[test]
fn check_container_accepts_whole_reference() {
    let b = synthetic_reference();
    let h = check_container(&b).expect("whole reference container must validate");
    assert_eq!(h.len_a + h.len_b, b.len() as u32);
}

#[test]
fn parse_rejects_bad_magic() {
    let mut b = synthetic_reference();
    b[0] = b'X';
    match parse_header(&b) {
        Err(V5Error::BadMagic { got }) => assert_ne!(got.as_slice(), b"V5_CFV3\x00"),
        other => panic!("expected BadMagic, got {:?}", other),
    }
}

#[test]
fn parse_rejects_truncated_input() {
    let b = synthetic_reference();
    match parse_header(&b[..HEADER_LEN - 1]) {
        Err(V5Error::TooShort { need, got }) => {
            assert_eq!(need, HEADER_LEN);
            assert_eq!(got, HEADER_LEN - 1);
        }
        other => panic!("expected TooShort, got {:?}", other),
    }
}

#[test]
fn parse_rejects_marker_drift() {
    let mut b = synthetic_reference();
    b[0x10] = 0x00; // FF x8 marker corrupted
    assert!(matches!(parse_header(&b), Err(V5Error::BadMarker { .. })));
}

#[test]
fn parse_rejects_prelude_drift() {
    let mut b = synthetic_reference();
    b[0x3C] = 0x00; // `_` CR LF NUL corrupted
    assert!(matches!(parse_header(&b), Err(V5Error::BadPrelude { .. })));
}

#[test]
fn parse_rejects_token_drift() {
    let mut b = synthetic_reference();
    b[0x40] = b'X'; // "CATIA_V5 CB0001\0" token corrupted
    assert!(matches!(parse_header(&b), Err(V5Error::BadToken { .. })));
}

#[test]
fn parse_rejects_token_follower_drift() {
    let mut b = synthetic_reference();
    b[0x50] = b'0'; // follower must be 'N'
    assert!(matches!(parse_header(&b), Err(V5Error::BadFollower { .. })));
}

#[test]
fn check_container_rejects_length_sum_mismatch() {
    let mut b = synthetic_reference();
    // Corrupt len B so A + B != file size; header prefix still looks valid.
    b[0x0C..0x10].copy_from_slice(&999u32.to_be_bytes());
    match check_container(&b) {
        Err(V5Error::LengthSumMismatch { a, b: bb, file_len }) => {
            // Original len_b was 8 so a + 8 == file_len; with len_b=999 the
            // invariant a + bb == file_len must now be violated.
            assert_eq!(a + 8, file_len as u32, "a unchanged by the corruption");
            assert_ne!(a + bb, file_len as u32, "corrupted bb breaks the sum");
            assert_eq!(bb, 999, "bb is the corrupted len_b");
        }
        other => panic!("expected LengthSumMismatch, got {:?}", other),
    }
}

#[test]
fn check_container_rejects_missing_tail() {
    let mut b = synthetic_reference();
    let n = b.len();
    b[n - 2] = 0xFF; // corrupt 'N' of CB__END\0
    match check_container(&b) {
        Err(V5Error::BadTail { .. }) => (),
        other => panic!("expected BadTail, got {:?}", other),
    }
    // length sum must still hold for the error to be BadTail (not mismatch):
    let h = parse_header(&b).unwrap();
    assert_eq!(h.len_a + h.len_b, b.len() as u32);
}

#[test]
fn has_magic_sniffs_only() {
    assert!(has_magic(b"V5_CFV3\x00anything-after"));
    assert!(!has_magic(b"V5_CFV3x"));
    assert!(!has_magic(b""));
    assert!(
        !has_magic(b"<?xml version=\"1.0\""),
        "ASCII rep must NOT sniff binary"
    );
}

#[test]
fn never_panics_on_garbage() {
    // Header parsing must never panic on untrusted bytes (fuzz-friendly,
    // P2-M1-T05 pattern).
    let cases: Vec<Vec<u8>> = vec![
        vec![],
        vec![0x56],
        vec![0u8; 8],
        vec![0xFF; 84],
        b"V5_CFV3\x00".to_vec(),
        synthetic_reference()[..68].to_vec(),
        b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>".to_vec(),
    ];
    for c in cases {
        let _ = parse_header(&c); // must return Err or Ok, never panic
        let _ = check_container(&c);
    }
}

// --- real material --------------------------------------------------------

/// Collect raw `.3DRep` paths from a colon/comma-separated env var that may
/// name directories (scanned recursively for `*.3DRep`/`*.3drep`) or files.
fn reps_from_env(env: &str) -> Vec<std::path::PathBuf> {
    use std::collections::VecDeque;
    let mut out = Vec::new();
    let Ok(v) = std::env::var(env) else {
        return out;
    };
    let mut queue: VecDeque<std::path::PathBuf> = VecDeque::new();
    for chunk in v.split([':', ',']) {
        let chunk = chunk.trim();
        if !chunk.is_empty() {
            queue.push_back(std::path::PathBuf::from(chunk));
        }
    }
    while let Some(p) = queue.pop_front() {
        if p.is_dir() {
            if let Ok(rd) = std::fs::read_dir(&p) {
                for e in rd.flatten() {
                    let ep = e.path();
                    if ep.is_dir() {
                        queue.push_back(ep);
                    } else if ep
                        .extension()
                        .map(|s| s == "3DRep" || s == "3drep")
                        .unwrap_or(false)
                    {
                        out.push(ep);
                    }
                }
            }
        } else if p.is_file() {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Real-material leg 1: every supplied binary `.3DRep` member must pass header
/// parse + whole-file invariants and carry the observed anchors.
#[test]
fn binary_branch_real_material_all_members() {
    let reps = reps_from_env("OEP_3DXML_BINARY_REPS");
    if reps.is_empty() {
        eprintln!("SKIP  no real binary .3DRep material (set OEP_3DXML_BINARY_REPS); CI-safe skip");
        return;
    }
    eprintln!("info  verifying {} binary .3DRep member(s)", reps.len());
    let mut verified = 0usize;
    for p in &reps {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e));
        let name = p
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let h = parse_header(&bytes)
            .unwrap_or_else(|e| panic!("{}: header parse failed: {:?}", name, e));
        check_container(&bytes)
            .unwrap_or_else(|e| panic!("{}: whole-container validation failed: {:?}", name, e));
        assert_eq!(
            h.len_a + h.len_b,
            bytes.len() as u32,
            "{}: length-sum invariant (observed 18/18)",
            name
        );
        assert_eq!(h.count, GOLDEN_COUNT, "{}: count u16BE@0x51", name);
        assert!(bytes.starts_with(b"V5_CFV3\x00"), "{}: magic", name);
        assert!(bytes.ends_with(b"CB__END\x00"), "{}: CB__END tail", name);
        verified += 1;
    }
    eprintln!(
        "PASS  {} binary member(s) verified (magic, length sum, marker, prelude, token, count, tail)",
        verified
    );
}

/// Real-material leg 2: the exact golden member fields match the notebook.
#[test]
fn binary_branch_real_material_golden_member() {
    let reps = reps_from_env("OEP_3DXML_BINARY_REPS");
    let found = reps
        .iter()
        .find(|p| p.file_name().map(|s| s == GOLDEN_MEMBER).unwrap_or(false));
    let Some(p) = found else {
        eprintln!(
            "SKIP  golden member '{}' not in supplied material; CI-safe skip",
            GOLDEN_MEMBER
        );
        return;
    };
    let bytes = std::fs::read(p).unwrap();
    eprintln!(
        "info  verifying golden member on real material: {}",
        p.display()
    );
    assert_eq!(
        bytes.len(),
        GOLDEN_SIZE,
        "size anchor for {}",
        GOLDEN_MEMBER
    );
    let h = parse_header(&bytes).expect("golden member header must parse");
    assert_eq!(h.len_a, GOLDEN_LEN_A);
    assert_eq!(h.len_b, GOLDEN_LEN_B);
    assert_eq!(h.count, GOLDEN_COUNT);
    eprintln!(
        "PASS  golden member parsed: len_a={} len_b={} count={} ({} bytes)",
        h.len_a,
        h.len_b,
        h.count,
        bytes.len()
    );
}

/// Real-material leg 3: an ASCII `.3DRep` (open corpus member) must NOT match
/// the binary branch — content-based discrimination, never the extension.
#[test]
fn ascii_rep_is_rejected_as_binary() {
    let reps = reps_from_env("OEP_3DXML_ASCII_REPS");
    if reps.is_empty() {
        eprintln!("SKIP  no ASCII .3DRep corpus (set OEP_3DXML_ASCII_REPS); CI-safe skip");
        return;
    }
    let mut checked = 0usize;
    for p in &reps {
        let bytes = std::fs::read(p).unwrap_or_else(|e| panic!("read {}: {}", p.display(), e));
        let name = p
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        assert!(
            !has_magic(&bytes),
            "{}: ASCII rep must not sniff V5_CFV3 (byte 0 = 0x3C '<')",
            name
        );
        assert!(
            bytes.starts_with(b"<?xml") || bytes.starts_with(b"<"),
            "{}: expected an ASCII XML rep in this leg (got {:02x})",
            name,
            bytes.first().copied().unwrap_or(0)
        );
        match parse_header(&bytes) {
            Err(_) => (),
            Ok(_) => panic!("{}: ASCII rep must fail binary header parse", name),
        }
        checked += 1;
    }
    eprintln!(
        "PASS  {} ASCII rep(s) rejected as binary (branch discrimination ok)",
        checked
    );
}
