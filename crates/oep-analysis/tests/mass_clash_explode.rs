//! Real-material acceptance for the analysis crate: the committed clashing
//! fixture. Loads `tests/golden/clash-fixture.oepgeo.json` (+ its material
//! companion), computes mass properties, runs the clash check, and explodes
//! the assembly. Every value is verified against the analytical solid /
//! known geometry within the committed tolerance.
//!
//! The fixture (real material, committed bytes):
//! - base plate: solid 40x30x10 at origin, aluminium 2.70 g/cm³
//! - interfering block: solid 20x10x10, steel 7.85 g/cm³, placed at
//!   (25, 20, 5) — which PENETRATES the base plate.
//!
//! Analytic goldens:
//! - base plate volume = 40*30*10 = 12000 mm³, mass = 2.70 * 12 = 32.4 g,
//!   centroid (20,15,5), Ixx = m/12 (30²+10²)=32.4/12*1000 = 2700, etc.
//! - block volume = 20*10*10 = 2000 mm³, mass = 7.85*2 = 15.7 g.
//! - clash overlap: the block spans x∈[25,45], y∈[20,30], z∈[5,15]; the base
//!   spans x∈[0,40], y∈[0,30], z∈[0,10]; overlap is x∈[25,40] (15),
//!   y∈[20,30] (10), z∈[5,10] (5) → volume 750 mm³, location
//!   ((25+40)/2, (20+30)/2, (5+10)/2) = (32.5, 25, 7.5).

use oep_analysis::clash::detect_in_assembly;
use oep_analysis::explode::explode;
use oep_analysis::mass::{body_mass_properties, mass_tolerance};
use oep_analysis::material::MaterialTable;
use oep_geo::Document;

const FIXTURE_DOC: &str = include_str!("golden/clash-fixture.oepgeo.json");
const FIXTURE_MATERIALS: &str = include_str!("golden/clash-fixture.materials.json");

fn load_doc() -> Document {
    Document::from_json_pretty(FIXTURE_DOC).expect("fixture parses")
}

fn load_materials() -> MaterialTable {
    MaterialTable::from_json_pretty(FIXTURE_MATERIALS).expect("material table parses")
}

fn density_resolver(table: &MaterialTable) -> impl Fn(&str) -> Option<f64> + '_ {
    move |body_id| table.density_g_cm3_for_body(body_id)
}

#[test]
fn base_plate_mass_properties_match_analytical() {
    let doc = load_doc();
    let materials = load_materials();
    let props = body_mass_properties(&doc, &density_resolver(&materials), "oep-body-base").unwrap();

    // Volume 12000 mm³ and mass 32.4 g.
    assert!(mass_tolerance().approx_eq(props.volume_mm3, 12000.0));
    assert!(mass_tolerance().approx_eq(props.mass_g, 32.4));
    // Centroid (20, 15, 5).
    assert!(mass_tolerance().points_eq(props.centroid, [20.0, 15.0, 5.0]));
    // Ixx = m/12 (30²+10²) = 2.7 * 1000 = 2700 g·mm².
    assert!(mass_tolerance().approx_eq(props.inertia[0], 2700.0));
    // Iyy = m/12 (40²+10²) = 2.7 * 1700 = 4590.
    assert!(mass_tolerance().approx_eq(props.inertia[1], 4590.0));
    // Izz = m/12 (40²+30²) = 2.7 * 2500 = 6750.
    assert!(mass_tolerance().approx_eq(props.inertia[2], 6750.0));
}

#[test]
fn block_mass_properties_match_analytical() {
    let doc = load_doc();
    let materials = load_materials();
    let props =
        body_mass_properties(&doc, &density_resolver(&materials), "oep-body-block").unwrap();
    assert!(mass_tolerance().approx_eq(props.volume_mm3, 2000.0));
    assert!(mass_tolerance().approx_eq(props.mass_g, 15.7));
    assert!(mass_tolerance().points_eq(props.centroid, [10.0, 5.0, 5.0]));
}

#[test]
fn known_clashing_pair_is_flagged() {
    let doc = load_doc();
    let clashes = detect_in_assembly(&doc, "oep-assembly-fixture").unwrap();
    assert_eq!(clashes.len(), 1, "exactly one clashing pair expected");
    let c = &clashes[0];
    // Overlap volume 750 mm³.
    assert!(mass_tolerance().approx_eq(c.volume, 750.0));
    // Location of the penetration (overlap box centroid).
    assert!(mass_tolerance().points_eq(c.location, [32.5, 25.0, 7.5]));
    // The pair involves both parts.
    let chain_ids: Vec<&str> = c
        .chain_a
        .iter()
        .chain(c.chain_b.iter())
        .map(|s| s.as_str())
        .collect();
    assert!(chain_ids.contains(&"i-base") && chain_ids.contains(&"i-block"));
}

#[test]
fn exploded_view_is_deterministic_at_constant_pull() {
    let doc = load_doc();
    let a = explode(&doc, "oep-assembly-fixture", 25.0).unwrap();
    let b = explode(&doc, "oep-assembly-fixture", 25.0).unwrap();
    assert_eq!(
        a, b,
        "explode must be a pure function of (doc, assembly, pull)"
    );
    assert_eq!(a.len(), 2);
    // Both parts are direct children → depth 1 → offset magnitude = pull.
    for p in &a {
        assert_eq!(p.depth, 1);
    }
}

#[test]
fn exploded_view_offset_scales_with_pull() {
    let doc = load_doc();
    let small = explode(&doc, "oep-assembly-fixture", 10.0).unwrap();
    let large = explode(&doc, "oep-assembly-fixture", 20.0).unwrap();
    // Doubling the pull distance doubles every signed offset (linearity).
    for (s, l) in small.iter().zip(large.iter()) {
        for i in 0..3 {
            assert!(mass_tolerance().approx_eq(l.offset[i], 2.0 * s.offset[i]));
        }
    }
}

#[test]
fn exploded_document_renders_and_is_deterministic() {
    let doc = load_doc();
    let placements = explode(&doc, "oep-assembly-fixture", 25.0).unwrap();
    let exploded =
        oep_analysis::explode::apply_placements(&doc, "oep-assembly-fixture", &placements).unwrap();

    // The exploded document is structurally valid (the viewer scene
    // builder accepts any valid oep-geo document — renderability).
    assert!(
        exploded.validate().is_empty(),
        "exploded doc must stay canonical"
    );

    // Deterministic bytes: same input + pull → identical serialization.
    let again = oep_analysis::explode::apply_placements(
        &load_doc(),
        "oep-assembly-fixture",
        &explode(&load_doc(), "oep-assembly-fixture", 25.0).unwrap(),
    )
    .unwrap();
    assert_eq!(
        exploded.to_json_pretty().unwrap(),
        again.to_json_pretty().unwrap()
    );

    // The exploded placements actually separated the pair: the instance
    // transforms differ from the source document.
    let source_block = doc
        .assemblies
        .iter()
        .find(|a| a.id == "oep-assembly-fixture")
        .and_then(|a| a.instances.iter().find(|i| i.id == "i-block"))
        .unwrap()
        .transform
        .clone();
    let exploded_block = exploded
        .assemblies
        .iter()
        .find(|a| a.id == "oep-assembly-fixture")
        .and_then(|a| a.instances.iter().find(|i| i.id == "i-block"))
        .unwrap()
        .transform
        .clone();
    assert_ne!(source_block, exploded_block);
}

#[test]
fn clash_report_is_deterministic() {
    let a = detect_in_assembly(&load_doc(), "oep-assembly-fixture").unwrap();
    let b = detect_in_assembly(&load_doc(), "oep-assembly-fixture").unwrap();
    assert_eq!(a, b);
}
