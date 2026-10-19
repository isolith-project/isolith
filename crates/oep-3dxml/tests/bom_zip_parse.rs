//! P3-M1-T01 acceptance test (TDD, RED first) — parse outer ZIP + BOM of a
//! real 3DXML file.
//!
//! Written BEFORE the `oep_3dxml::bom` / `container` / `manifest` modules
//! existed (RED: this file does not compile against the skeleton crate).
//! The contract, once GREEN, is:
//!
//! 1. `detect` classifies a 3DXML container from its first bytes: a real
//!    ZIP-archive 3DXML starts `50 4B 03 04` (PK\x03\x04), a standalone
//!    ASCII model document starts `3C 3F 78 6D` (`<?xm`), anything else is
//!    Unknown. Never panics.
//! 2. `parse` on a ZIP-archive 3DXML returns the member inventory (outer ZIP
//!    parse) AND the decoded `Manifest.xml` (the container BOM: `<Root>`
//!    primary model document, optional `<Date>`, `<WithAuthoringData>`), and
//!    verifies the declared `<Root>` names a member that actually exists in
//!    the archive.
//! 3. `parse` on a standalone ASCII model document reports `StandaloneXml`
//!    without an archive (its model-doc XML parse is P3-M1-T02).
//! 4. Malformed input (junk bytes, truncated ZIP, empty) is rejected with a
//!    typed error, never a panic (fuzz-friendly, cf. P2-M1-T05 discipline).
//! 5. Against REAL material, when a golden sample is supplied (env
//!    `OEP_3DXML_SAMPLES`, or the repo's license-clean corpus at
//!    `corpus/data/3dxml`, or `samples` next to the crate), the parsed entry
//!    counts and Manifest `<Root>` values must match the observed
//!    golden-facts map below; otherwise the test prints SKIP and passes so CI
//!    stays green before the P1 corpus PR (task/3dxml-samples-1015a) merges.
//!
//! Clean-room rule honoured here: every asserted fact comes from observed
//! bytes of license-clean MIT/Apache-2.0 corpus files (P1-M1-T02) and the
//! P1-M2-T04 hex notebook (`docs/re-notes/3dxml-hex-notebook.md`). Nothing
//! proprietary is committed. The synthetic ZIP is built from public ZIP
//! structure facts only.

use std::io::Write;
use std::path::PathBuf;

use oep_3dxml::bom::{parse, Bom, BomError};
use oep_3dxml::container::{detect, parse_archive, Archive, ContainerKind, Entry};
use oep_3dxml::manifest::{parse_manifest, Manifest};

// ---------------------------------------------------------------------------
// Observed golden facts (verified 2026-10-15 on the license-clean corpus):
// file -> (size, entry count, Manifest <Root>, expected container kind).
// All ZIP members are DEFLATE (method 8), manifests ASCII XML, LF or CRLF.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Golden {
    size: u64,
    entries: usize,
    root: &'static str,
    kind: ContainerKind,
}

const GOLDEN: &[(&str, Golden)] = &[
    (
        "blocks.3dxml",
        Golden {
            size: 2583,
            entries: 3,
            root: "Abaqus_Model.3dxml",
            kind: ContainerKind::ZipArchive,
        },
    ),
    (
        "cube1.3dxml",
        Golden {
            size: 4834,
            entries: 7,
            root: "Abaqus_Model.3dxml",
            kind: ContainerKind::ZipArchive,
        },
    ),
    (
        "machinist.3DXML",
        Golden {
            size: 38_849,
            entries: 9,
            root: "NameSetTree.3dxml",
            kind: ContainerKind::ZipArchive,
        },
    ),
    (
        "cycloidal.3DXML",
        Golden {
            size: 742_428,
            entries: 28,
            root: "NameSetTree.3dxml",
            kind: ContainerKind::ZipArchive,
        },
    ),
    (
        "FM1_satellite.3DXML",
        Golden {
            size: 1_763_813,
            entries: 69,
            root: "NameSetTree.3dxml",
            kind: ContainerKind::ZipArchive,
        },
    ),
    (
        "quad.3dxml",
        Golden {
            size: 14_317,
            entries: 0,
            root: "",
            kind: ContainerKind::StandaloneXml,
        },
    ),
];

/// Locations searched for real samples: `$OEP_3DXML_SAMPLES` (colon/comma
/// separated), else `samples`/`../samples` next to the crate, else the repo
/// corpus `corpus/data/3dxml` (resolved from CARGO_MANIFEST_DIR).
fn sample_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("OEP_3DXML_SAMPLES") {
        for chunk in env.split([':', ',']) {
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                dirs.push(PathBuf::from(chunk));
            }
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dirs.push(manifest_dir.join("samples"));
    dirs.push(manifest_dir.join("../samples"));
    dirs.push(manifest_dir.join("../../corpus/data/3dxml"));
    dirs
}

fn find_samples() -> Vec<(PathBuf, &'static str, Golden)> {
    let mut out = Vec::new();
    for &(name, golden) in GOLDEN {
        let golden: Golden = golden;
        for dir in sample_dirs() {
            let probe = dir.join(name);
            if probe.is_file() {
                out.push((probe, name, golden));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Synthetic real-shaped input (public structure facts only; no private bytes)
// ---------------------------------------------------------------------------

/// Build a minimal ZIP-archive 3DXML: Manifest.xml declaring `root` plus an
/// empty model-doc member with that name. Deflate-compressed like the real
/// corpus.
fn synthetic_3dxml_zip(root: &str) -> Vec<u8> {
    let manifest = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<Manifest xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"Manifest.xsd\"><Root>{root}</Root></Manifest>\r\n"
    );
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut w = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        w.start_file("Manifest.xml", opts).unwrap();
        w.write_all(manifest.as_bytes()).unwrap();
        w.start_file(root, opts).unwrap();
        w.write_all(b"<Model_3dxml/>").unwrap();
        w.finish().unwrap();
    }
    buf.into_inner()
}

// ---------------------------------------------------------------------------
// Detection (branch entry point, per hex notebook section 6)
// ---------------------------------------------------------------------------

#[test]
fn detects_zip_archive_from_pk_magic() {
    assert_eq!(
        detect(&[0x50, 0x4B, 0x03, 0x04, 0x2A, 0x2A]),
        ContainerKind::ZipArchive
    );
}

#[test]
fn detects_standalone_xml_from_xml_declaration() {
    assert_eq!(
        detect(b"<?xml version=\"1.0\" encoding=\"utf-8\" ?>"),
        ContainerKind::StandaloneXml
    );
    assert_eq!(detect(b"<?xml"), ContainerKind::StandaloneXml);
}

#[test]
fn unknown_for_junk_and_empty_inputs_never_panics() {
    assert_eq!(detect(b""), ContainerKind::Unknown);
    assert_eq!(detect(b"\x00\x00\x00\x00"), ContainerKind::Unknown);
    assert_eq!(detect(b"PK"), ContainerKind::Unknown); // too short for full magic
    assert_eq!(
        detect(b"not a zip or xml at all........"),
        ContainerKind::Unknown
    );
}

// ---------------------------------------------------------------------------
// Manifest (BOM) extractor
// ---------------------------------------------------------------------------

#[test]
fn manifest_parses_ablative_form() {
    let m = parse_manifest(br#"<?xml version="1.0" encoding="utf-8" ?>
<Manifest xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="Manifest.xsd">
<Root>Abaqus_Model.3dxml</Root>
</Manifest>"#)
    .expect("ablative-style manifest must parse");
    assert_eq!(m.root, "Abaqus_Model.3dxml");
    assert!(m.date.is_none());
    assert!(m.with_authoring_data.is_none());
}

#[test]
fn manifest_parses_catia_v5_form_with_metadata() {
    let m = parse_manifest(br#"<?xml version="1.0" encoding="UTF-8"?>
<Manifest xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="Manifest.xsd"><Root>NameSetTree.3dxml</Root><Date>2017-12-14</Date><WithAuthoringData>false</WithAuthoringData></Manifest>
"#)
    .expect("CATIA-family manifest must parse");
    assert_eq!(m.root, "NameSetTree.3dxml");
    assert_eq!(m.date.as_deref(), Some("2017-12-14"));
    assert_eq!(m.with_authoring_data, Some(false));
}

#[test]
fn manifest_rejects_missing_root() {
    let r = parse_manifest(br#"<?xml version="1.0"?><Manifest><Date>2020-01-01</Date></Manifest>"#);
    assert!(matches!(
        r,
        Err(oep_3dxml::manifest::ManifestError::MissingElement { .. })
    ));
}

// ---------------------------------------------------------------------------
// Outer ZIP parse + BOM coherence (synthetic)
// ---------------------------------------------------------------------------

#[test]
fn parses_synthetic_zip_bom_end_to_end() {
    let bytes = synthetic_3dxml_zip("Abaqus_Model.3dxml");
    match parse(&bytes) {
        Ok(bom) => {
            assert_eq!(bom.kind, ContainerKind::ZipArchive);
            let archive = bom.archive.as_ref().expect("ZIP archive must be present");
            assert_eq!(archive.entries.len(), 2, "Manifest.xml + one model doc");
            assert!(archive.entries.iter().any(|e| e.name == "Manifest.xml"));
            assert!(archive
                .entries
                .iter()
                .any(|e| e.name == "Abaqus_Model.3dxml"));
            let manifest = bom.manifest.as_ref().expect("manifest must be present");
            assert_eq!(manifest.root, "Abaqus_Model.3dxml");
            // BOM coherence: the declared root must name an actual member.
            assert!(archive.entries.iter().any(|e| e.name == manifest.root));
        }
        other => panic!("expected Ok(Bom), got {:?}", other),
    }
}

#[test]
fn rejects_junk_not_a_zip_and_never_panics() {
    let junk = b"this is not a zip archive and not xml either, just prose.........";
    match parse(junk) {
        Err(BomError::UnknownContainer { .. }) => {}
        other => panic!("expected UnknownContainer, got {:?}", other),
    }
}

#[test]
fn rejects_empty_input() {
    match parse(b"") {
        Err(BomError::TooShort { .. } | BomError::UnknownContainer { .. }) => {}
        other => panic!("expected TooShort/UnknownContainer, got {:?}", other),
    }
}

#[test]
fn archive_shell_rejects_truncated_zip() {
    // A real-looking PK header followed by too few bytes to hold a local header.
    let truncated = [0x50, 0x4B, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00];
    let r = parse_archive(&truncated);
    assert!(
        r.is_err(),
        "truncated ZIP must be a typed error, got Ok({:?})",
        r
    );
}

// ---------------------------------------------------------------------------
// Real material leg. Fails LOUD on ANY disagreement with the golden map;
// prints SKIP and passes when no sample is supplied, so CI without the
// corpus stays green. All golden facts were observed on 2026-10-15.
// ---------------------------------------------------------------------------

#[test]
fn real_material_zip_and_manifest() {
    let samples = find_samples();
    if samples.is_empty() {
        eprintln!(
            "SKIP  no real 3DXML sample found (set OEP_3DXML_SAMPLES=<dir> or use the repo corpus); tolerant CI-safe skip"
        );
        return;
    }

    let mut verified = 0usize;
    for (path, name, golden) in samples {
        eprintln!(
            "info  verifying container+BOM parse on real material: {}",
            path.display()
        );

        let bytes = std::fs::read(&path).expect("read golden sample");
        assert_eq!(
            bytes.len() as u64,
            golden.size,
            "golden size anchor for {name} (see docs/re-notes/oep-3dxml-container-bom.md)"
        );
        assert_eq!(detect(&bytes), golden.kind, "detect kind anchor for {name}");

        if golden.kind == ContainerKind::StandaloneXml {
            eprintln!("PASS  {name}: standalone ASCII model document detected (no ZIP)");
            verified += 1;
            continue;
        }

        let bom: Bom = match parse(&bytes) {
            Ok(b) => b,
            Err(e) => panic!("real golden sample {name} must parse, got {e:?}"),
        };
        let archive: &Archive = bom.archive.as_ref().expect("ZIP archive present");
        assert_eq!(
            archive.entries.len(),
            golden.entries,
            "archive entry-count anchor for {name}"
        );
        assert!(
            archive
                .entries
                .iter()
                .any(|e: &Entry| e.name == "Manifest.xml"),
            "{name}: a ZIP-archive 3DXML must contain Manifest.xml"
        );
        let manifest: &Manifest = bom.manifest.as_ref().expect("manifest present");
        assert_eq!(
            manifest.root, golden.root,
            "Manifest <Root> anchor for {name}"
        );
        assert!(
            archive.entries.iter().any(|e| e.name == manifest.root),
            "BOM coherence: <Root> {} must name a member present in {name}",
            manifest.root
        );
        eprintln!(
            "PASS  {name}: {} entries, root={}",
            archive.entries.len(),
            manifest.root
        );
        verified += 1;
    }
    assert!(
        verified >= 1,
        "expected to verify at least one real sample, verified {verified}"
    );
}
