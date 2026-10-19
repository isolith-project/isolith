//! P3-M1-T02 acceptance test (TDD, RED first) — parse the `<Model_3dxml>`
//! representation: nodes (`Reference3D`), references (`ReferenceRep` /
//! `InstanceRep`) and instances (`Instance3D`).
//!
//! Written BEFORE the `oep_3dxml::rep` module existed (RED: this file does not
//! compile against the skeleton crate). The contract, once GREEN, is:
//!
//! 1. `parse_model` on a `<Model_3dxml>` model document returns the product
//!    structure as typed records: `Header`, `ProductStructure { root_id,
//!    nodes (Reference3D), references (ReferenceRep), instances (Instance3D),
//!    instance_reps (InstanceRep) }`.
//! 2. `ReferenceRep` carries `format` (observed `TESSELLATED`),
//!    optional `version` and the `associatedFile` URN, whose member name is
//!    parsed off the `urn:3DXML:` prefix.
//! 3. `Instance3D` carries the `<IsAggregatedBy>`, `<IsInstanceOf>` references
//!    and the 12-double `<RelativeMatrix>` (row-major 4x3 transform).
//! 4. Malformed input (junk bytes, truncated XML, non-XML, wrong matrix
//!    length) is rejected with a typed error, never a panic (fuzz-friendly,
//!    cf. P2-M1-T05 discipline).
//! 5. Against REAL material, when a golden sample is supplied (env
//!    `OEP_3DXML_SAMPLES`, or the repo's license-clean corpus at
//!    `corpus/data/3dxml`, or `samples` next to the crate), the parsed
//!    node/instance/rep counts and header facts must match the observed
//!    golden-facts map below; otherwise the test prints SKIP and passes so CI
//!    stays green before the corpus PR merges.
//!
//! Clean-room rule honoured here: every asserted fact comes from observed
//! bytes of license-clean MIT/Apache-2.0 corpus files (P1-M1-T02) and the
//! P1-M2-T04 hex notebook (`docs/re-notes/3dxml-hex-notebook.md` §4.1).
//! Synthetic fixtures are built from the public `3DXML.xsd` element names only.

use std::path::PathBuf;

use oep_3dxml::rep::{
    parse_model, Header, Instance3D, ProductStructure, Reference3D, ReferenceRep, RepError,
};

// ---------------------------------------------------------------------------
// Observed golden facts (verified 2026-10-15 on the license-clean corpus):
// file -> (schema version, #Reference3D, #ReferenceRep, #Instance3D,
// #InstanceRep, ProductStructure root id, first associatedFile member,
// first instance id/name).
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Golden {
    schema: &'static str,
    nodes: usize,
    refs: usize,
    instances: usize,
    inst_reps: usize,
    root_id: &'static str,
    first_rep_member: &'static str,
}

const GOLDEN: &[(&str, Golden)] = &[
    (
        "blocks.3dxml",
        Golden {
            schema: "4.0",
            nodes: 2,
            refs: 1,
            instances: 1,
            inst_reps: 1,
            root_id: "1",
            first_rep_member: "Abaqus_Geometry.3DRep",
        },
    ),
    (
        "cube1.3dxml",
        Golden {
            schema: "4.0",
            nodes: 2,
            refs: 1,
            instances: 1,
            inst_reps: 1,
            root_id: "1",
            first_rep_member: "Abaqus_Geometry.3DRep",
        },
    ),
    (
        "machinist.3DXML",
        Golden {
            schema: "4.2",
            nodes: 1,
            refs: 1,
            instances: 0,
            inst_reps: 1,
            root_id: "1",
            first_rep_member: "TessPart_2.3DRep",
        },
    ),
    (
        "quad.3dxml",
        Golden {
            schema: "4.3",
            nodes: 7,
            refs: 4,
            instances: 8,
            inst_reps: 4,
            root_id: "1",
            first_rep_member: "Chassis.3DRep",
        },
    ),
    (
        "cycloidal.3DXML",
        Golden {
            schema: "4.2",
            nodes: 17,
            refs: 13,
            instances: 67,
            inst_reps: 13,
            root_id: "1",
            first_rep_member: "TessPart_5.3DRep",
        },
    ),
    (
        "FM1_satellite.3DXML",
        Golden {
            schema: "4.2",
            nodes: 72,
            refs: 36,
            instances: 94,
            inst_reps: 36,
            root_id: "1",
            first_rep_member: "TessPart_3.3DRep",
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

/// Open a real 3DXML file and return the primary model-document bytes:
/// ZIP-archive 3DXMLs follow `Manifest.xml` `<Root>` (like P3-M1-T01);
/// standalone ASCII model documents are read as-is.
fn load_model_document_bytes(path: &std::path::Path) -> Vec<u8> {
    let bytes = std::fs::read(path).expect("read golden sample");
    if bytes.starts_with(b"PK\x03\x04") {
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("open zip");
        let manifest = zip.by_name("Manifest.xml").expect("manifest member");
        let text = std::io::read_to_string(manifest).expect("read manifest");
        let root = text
            .lines()
            .find(|l| l.contains("<Root>"))
            .and_then(|l| l.split("<Root>").nth(1))
            .and_then(|l| l.split("</Root>").next())
            .expect("Root in manifest");
        let mut member = zip.by_name(root.trim()).expect("model doc member");
        let mut out = Vec::new();
        std::io::Read::read_to_end(&mut member, &mut out).expect("read model doc");
        out
    } else {
        bytes
    }
}

// ---------------------------------------------------------------------------
// Synthetic real-shaped input (public element names only; no private bytes)
// ---------------------------------------------------------------------------

/// Minimal Abaqus-style model document: header + one product with a rep.
fn abaqus_style_model() -> &'static [u8] {
    br#"<?xml version="1.0" encoding="utf-8" ?>
<Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xlink="http://www.w3.org/1999/xlink" xsi:schemaLocation="http://www.3ds.com/xsd/3DXML">
	<Header>
		<SchemaVersion>4.0</SchemaVersion>
		<Title>T</Title>
		<Author>A</Author>
		<Generator>G</Generator>
		<Created>2023-01-01</Created>
	</Header>
	<ProductStructure root="1">
		<Reference3D xsi:type="Reference3DType" id="1" name="Root"/>
		<Reference3D xsi:type="Reference3DType" id="2" name="Geo"/>
		<Instance3D xsi:type="Instance3DType" id="3" name="Geo">
		<IsAggregatedBy>1</IsAggregatedBy>
		<IsInstanceOf>2</IsInstanceOf>
		<RelativeMatrix>0.000000 1.000000 0.000000 0.000000 0.000000 1.000000 1.000000 0.000000 0.000000 0.000000 0.000000 0.000000</RelativeMatrix>
		</Instance3D>
		<ReferenceRep xsi:type="ReferenceRepType" id="5" name="Rep" format="TESSELLATED" version="1.1" associatedFile="urn:3DXML:Abaqus_Geometry.3DRep"/>
		<InstanceRep xsi:type="InstanceRepType" id="4" name="InstRep">
			<IsAggregatedBy>2</IsAggregatedBy>
			<IsInstanceOf>5</IsInstanceOf>
		</InstanceRep>
	</ProductStructure>
	</Model_3dxml>
"#
}

// ---------------------------------------------------------------------------
// Synthetic (public element shapes), the TDD core
// ---------------------------------------------------------------------------

#[test]
fn parses_header_and_counts_from_synthetic_model() {
    let m = parse_model(abaqus_style_model()).expect("synthetic model must parse");
    assert_eq!(m.header.schema_version.as_deref(), Some("4.0"));
    assert_eq!(m.header.generator.as_deref(), Some("G"));

    let ps: &ProductStructure = &m.structure;
    assert_eq!(ps.root_id.as_deref(), Some("1"));
    assert_eq!(ps.nodes.len(), 2, "two Reference3D");
    assert_eq!(ps.references.len(), 1, "one ReferenceRep");
    assert_eq!(ps.instances.len(), 1, "one Instance3D");
    assert_eq!(ps.instance_reps.len(), 1, "one InstanceRep");
}

#[test]
fn node_records_carry_id_and_name() {
    let m = parse_model(abaqus_style_model()).expect("parse");
    let node: &Reference3D = m.structure.nodes.iter().find(|n| n.id == "2").unwrap();
    assert_eq!(node.name, "Geo");
}

#[test]
fn rep_reference_parses_urn_member_name() {
    let m = parse_model(abaqus_style_model()).expect("parse");
    let rep: &ReferenceRep = &m.structure.references[0];
    assert_eq!(rep.format.as_deref(), Some("TESSELLATED"));
    assert_eq!(rep.version.as_deref(), Some("1.1"));
    assert_eq!(
        rep.associated_file.as_deref(),
        Some("urn:3DXML:Abaqus_Geometry.3DRep")
    );
    assert_eq!(rep.member_name(), Some("Abaqus_Geometry.3DRep".to_string()));
}

#[test]
fn instance_records_carry_links_and_matrix() {
    let m = parse_model(abaqus_style_model()).expect("parse");
    let inst: &Instance3D = &m.structure.instances[0];
    assert_eq!(inst.is_aggregated_by.as_deref(), Some("1"));
    assert_eq!(inst.is_instance_of.as_deref(), Some("2"));
    let mat = inst
        .relative_matrix
        .as_ref()
        .expect("RelativeMatrix present");
    assert_eq!(mat.len(), 12);
    assert_eq!(mat[0], 0.0);
    assert_eq!(mat[1], 1.0);
}

// ---------------------------------------------------------------------------
// Tolerant failure: typed errors, never a panic
// ---------------------------------------------------------------------------

#[test]
fn rejects_junk_and_never_panics() {
    let junk = b"this is not XML at all, just prose................................";
    match parse_model(junk) {
        Err(RepError::Xml { .. }) | Err(RepError::InvalidUtf8) => {}
        other => panic!("expected Xml/InvalidUtf8, got {:?}", other.map(|_| ())),
    }
}

#[test]
fn rejects_truncated_model_document_and_never_panics() {
    let truncated =
        b"<?xml version=\"1.0\"?><Model_3dxml><ProductStructure root=\"1\"><Reference3D";
    let r = parse_model(truncated);
    assert!(
        matches!(r, Err(RepError::Xml { .. })),
        "truncated XML must be a typed error, got {r:?}"
    );
}

#[test]
fn rejects_wrong_matrix_length_with_typed_error() {
    let bad = br#"<?xml version="1.0"?>
<Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML">
<Header><SchemaVersion>4.0</SchemaVersion></Header>
<ProductStructure root="1">
<Reference3D id="1" name="R"/>
<Instance3D id="2" name="I"><IsAggregatedBy>1</IsAggregatedBy><IsInstanceOf>1</IsInstanceOf>
<RelativeMatrix>1 0 0 0 1 0</RelativeMatrix></Instance3D>
</ProductStructure></Model_3dxml>"#;
    let r = parse_model(bad);
    assert!(
        matches!(r, Err(RepError::Matrix { .. })),
        "6-element matrix must be a typed Matrix error, got {r:?}"
    );
}

#[test]
fn empty_model_without_product_structure_is_tolerated() {
    // A `<Model_3dxml>` with no ProductStructure at all is a valid (degraded)
    // document from the parser's point of view: empty structure, no panic.
    let empty = br#"<?xml version="1.0"?><Model_3dxml xmlns="http://www.3ds.com/xsd/3DXML"/>"#;
    let m = parse_model(empty).expect("parse tolerantly");
    assert_eq!(m.structure.nodes.len(), 0);
    assert_eq!(m.structure.instances.len(), 0);
}

// ---------------------------------------------------------------------------
// Header accessors used by the real-material leg
// ---------------------------------------------------------------------------

#[allow(dead_code)]
fn _assert_header_fields(h: &Header) {
    let _ = [
        &h.schema_version,
        &h.title,
        &h.author,
        &h.generator,
        &h.created,
    ];
}

// ---------------------------------------------------------------------------
// Real material leg. Fails LOUD on ANY disagreement with the golden map;
// prints SKIP and passes when no sample is supplied, so CI without the
// corpus stays green. All golden facts were observed on 2026-10-15.
// ---------------------------------------------------------------------------

#[test]
fn real_material_rep_parse() {
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
            "info  verifying rep parse on real material: {}",
            path.display()
        );

        let model_bytes = load_model_document_bytes(&path);
        let m = match parse_model(&model_bytes) {
            Ok(m) => m,
            Err(e) => panic!("real golden sample {name} must parse, got {e:?}"),
        };

        assert_eq!(
            m.header.schema_version.as_deref(),
            Some(golden.schema),
            "schema-version anchor for {name}"
        );
        let ps: &ProductStructure = &m.structure;
        assert_eq!(
            ps.root_id.as_deref(),
            Some(golden.root_id),
            "root anchor {name}"
        );
        assert_eq!(ps.nodes.len(), golden.nodes, "Reference3D count for {name}");
        assert_eq!(
            ps.references.len(),
            golden.refs,
            "ReferenceRep count for {name}"
        );
        assert_eq!(
            ps.instances.len(),
            golden.instances,
            "Instance3D count for {name}"
        );
        assert_eq!(
            ps.instance_reps.len(),
            golden.inst_reps,
            "InstanceRep count for {name}"
        );

        if !ps.references.is_empty() {
            assert_eq!(
                ps.references[0].member_name().as_deref(),
                Some(golden.first_rep_member),
                "first associatedFile member anchor for {name}"
            );
        }

        // Every Reference3D has id + name; every instance has a 12-double
        // matrix when present in the source (machinist has zero instances so
        // the matrix loop is vacuous there).
        for n in &ps.nodes {
            assert!(!n.id.is_empty() && !n.name.is_empty(), "node fields {name}");
        }
        for i in &ps.instances {
            if let Some(ref mat) = i.relative_matrix {
                assert_eq!(mat.len(), 12, "matrix length for {name}");
            }
            assert!(
                i.is_aggregated_by.is_some() && i.is_instance_of.is_some(),
                "instance links for {name}"
            );
        }

        eprintln!(
            "PASS  {name}: schema={} nodes={} refs={} instances={} instreps={}",
            m.header.schema_version.as_deref().unwrap_or("?"),
            ps.nodes.len(),
            ps.references.len(),
            ps.instances.len(),
            ps.instance_reps.len()
        );
        verified += 1;
    }
    assert!(
        verified >= 1,
        "expected to verify at least one real sample, verified {verified}"
    );
}
