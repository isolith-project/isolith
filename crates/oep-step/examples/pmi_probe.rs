//! P4-M2-T03 `pmi_probe` — PMI fidelity audit probe.
//!
//! Reads one or more ISO 10303-21 STEP physical files through the wrapped OSS
//! STEP library (step-io, per the P4-M1 step-library ADR) and emits one JSON
//! object per file carrying the evidence needed for the GD&T-types x
//! reference-apps fidelity audit:
//!
//! - `entities` — ground-truth PMI inventory straight from the raw parsed
//!   model's per-entity-type arenas (what the file physically contains: each
//!   AP242 tolerance subtype, datums, tolerance zones, plus-minus tolerances,
//!   shape aspects);
//! - `surfaced` — what the library's PMI navigation layer (`Scene`) actually
//!   surfaces: geometric tolerances classified by characteristic plus datum
//!   and dimension counts;
//! - `dropped` — count of STEP entities the library dropped during read (the
//!   honest "not preserved" signal, never hidden).
//!
//! The probe is a leaf auditing tool: it makes no claims about correctness of
//! any datum or value, it only reports what is present in the file and what the
//! reference reader surfaced. Verdicts (PASS/FAIL/SKIP) are produced by the
//! Python audit runner, not by this probe.

use serde_json::{json, Map, Value};
use std::path::Path;
use step_io::generated::model::StepModel;

fn main() {
    let mut out: Vec<Value> = Vec::new();
    for arg in std::env::args().skip(1) {
        out.push(probe_file(&arg));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&Value::Array(out)).unwrap()
    );
}

/// Resolve a PMI-relevant entity arena size by short key. The entry lists the
/// exact `StepModel` field (with `_arena` suffix) next to the key emitted in
/// the JSON (without). Unknown keys -> 0 (the entity type is simply absent
/// from the model).
fn arena_len(model: &StepModel, key: &str) -> usize {
    match key {
        // tolerance subtypes / relatives
        "position_tolerance" => model.position_tolerance_arena.items.len(),
        "flatness_tolerance" => model.flatness_tolerance_arena.items.len(),
        "straightness_tolerance" => model.straightness_tolerance_arena.items.len(),
        "roundness_tolerance" => model.roundness_tolerance_arena.items.len(),
        "cylindricity_tolerance" => model.cylindricity_tolerance_arena.items.len(),
        "line_profile_tolerance" => model.line_profile_tolerance_arena.items.len(),
        "surface_profile_tolerance" => model.surface_profile_tolerance_arena.items.len(),
        "angularity_tolerance" => model.angularity_tolerance_arena.items.len(),
        "parallelism_tolerance" => model.parallelism_tolerance_arena.items.len(),
        "perpendicularity_tolerance" => model.perpendicularity_tolerance_arena.items.len(),
        "concentricity_tolerance" => model.concentricity_tolerance_arena.items.len(),
        "coaxiality_tolerance" => model.coaxiality_tolerance_arena.items.len(),
        "symmetry_tolerance" => model.symmetry_tolerance_arena.items.len(),
        "circular_runout_tolerance" => model.circular_runout_tolerance_arena.items.len(),
        "total_runout_tolerance" => model.total_runout_tolerance_arena.items.len(),
        "geometric_tolerance" => model.geometric_tolerance_arena.items.len(),
        "modified_geometric_tolerance" => model.modified_geometric_tolerance_arena.items.len(),
        "unequally_disposed_geometric_tolerance" => model
            .unequally_disposed_geometric_tolerance_arena
            .items
            .len(),
        "geometric_tolerance_with_datum_reference" => model
            .geometric_tolerance_with_datum_reference_arena
            .items
            .len(),
        "geometric_tolerance_with_modifiers" => {
            model.geometric_tolerance_with_modifiers_arena.items.len()
        }
        "plus_minus_tolerance" => model.plus_minus_tolerance_arena.items.len(),
        "tolerance_zone" => model.tolerance_zone_arena.items.len(),
        "tolerance_zone_definition" => model.tolerance_zone_definition_arena.items.len(),
        "tolerance_value" => model.tolerance_value_arena.items.len(),
        // datums
        "datum" => model.datum_arena.items.len(),
        "common_datum" => model.common_datum_arena.items.len(),
        "datum_reference" => model.datum_reference_arena.items.len(),
        "datum_system" => model.datum_system_arena.items.len(),
        "datum_feature" => model.datum_feature_arena.items.len(),
        "datum_target" => model.datum_target_arena.items.len(),
        // shape anchors
        "shape_aspect" => model.shape_aspect_arena.items.len(),
        "derived_shape_aspect" => model.derived_shape_aspect_arena.items.len(),
        "composite_shape_aspect" => model.composite_shape_aspect_arena.items.len(),
        "shape_aspect_relationship" => model.shape_aspect_relationship_arena.items.len(),
        "shape_aspect_associativity" => model.shape_aspect_associativity_arena.items.len(),
        // dimensional
        "dimensional_location" => model.dimensional_location_arena.items.len(),
        "dimensional_size" => model.dimensional_size_arena.items.len(),
        "angular_location" => model.angular_location_arena.items.len(),
        "angular_size" => model.angular_size_arena.items.len(),
        _ => 0,
    }
}

fn bump(counts: &mut Map<String, Value>, key: &str) {
    let n = counts.get(key).and_then(|v| v.as_u64()).unwrap_or(0) + 1;
    counts.insert(key.to_string(), json!(n));
}

/// Surface the model's geometric tolerances classified by `ToleranceKind`,
/// deduplicated by global `EntityKey` (a complex tolerance may be reachable
/// both as a `ComplexUnit` and through its leaf facet).
fn surfaced_tolerances(scene: &step_io::scene::Scene<'_>) -> Map<String, Value> {
    let mut counts: Map<String, Value> = Map::new();
    let mut other = 0usize;
    let mut seen: Vec<step_io::generated::model::EntityKey> = Vec::new();
    for t in scene.tolerances() {
        let k = t.key();
        if seen.contains(&k) {
            continue;
        }
        seen.push(k);
        match t.kind() {
            step_io::scene::ToleranceKind::Position => bump(&mut counts, "Position"),
            step_io::scene::ToleranceKind::Flatness => bump(&mut counts, "Flatness"),
            step_io::scene::ToleranceKind::Straightness => bump(&mut counts, "Straightness"),
            step_io::scene::ToleranceKind::Roundness => bump(&mut counts, "Roundness"),
            step_io::scene::ToleranceKind::Cylindricity => bump(&mut counts, "Cylindricity"),
            step_io::scene::ToleranceKind::LineProfile => bump(&mut counts, "LineProfile"),
            step_io::scene::ToleranceKind::SurfaceProfile => bump(&mut counts, "SurfaceProfile"),
            step_io::scene::ToleranceKind::Angularity => bump(&mut counts, "Angularity"),
            step_io::scene::ToleranceKind::Parallelism => bump(&mut counts, "Parallelism"),
            step_io::scene::ToleranceKind::Perpendicularity => {
                bump(&mut counts, "Perpendicularity")
            }
            step_io::scene::ToleranceKind::Concentricity => bump(&mut counts, "Concentricity"),
            step_io::scene::ToleranceKind::Coaxiality => bump(&mut counts, "Coaxiality"),
            step_io::scene::ToleranceKind::Symmetry => bump(&mut counts, "Symmetry"),
            step_io::scene::ToleranceKind::CircularRunout => bump(&mut counts, "CircularRunout"),
            step_io::scene::ToleranceKind::TotalRunout => bump(&mut counts, "TotalRunout"),
            step_io::scene::ToleranceKind::Other => other += 1,
        }
    }
    if other > 0 {
        counts.insert("_unclassified_other".to_string(), json!(other));
    }
    counts
}

fn probe_file(path_str: &str) -> Value {
    let path = Path::new(path_str);
    let file = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path_str.to_string());

    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            return json!({
                "file": file, "path": path_str, "parse_ok": false,
                "parse_error": format!("io: {e}"),
            });
        }
    };

    if !bytes.starts_with(b"ISO-10303-21;") {
        return json!({
            "file": file, "path": path_str, "parse_ok": false,
            "parse_error": "not an ISO-10303-21 STEP physical file",
        });
    }

    let (model, report) = match step_io::read(&bytes) {
        Ok(pair) => pair,
        Err(e) => {
            return json!({
                "file": file, "path": path_str, "parse_ok": false,
                "parse_error": format!("step-io parse: {e}"),
            });
        }
    };

    let schema = format!("{:?}", model.header().schema);
    let scene = model.scene();

    // Dropped-entity evidence: which entity types the reader dropped, counted
    // by their STEP type key (e.g. CIRCULAR_RUNOUT_TOLERANCE). This is the
    // honest "present in file but not preserved by this reader" signal.
    let dropped_total = report.dropped.len();
    let mut dropped_by_type: Map<String, Value> = Map::new();
    for (_id, reason) in &report.dropped {
        bump(&mut dropped_by_type, reason.key.as_str());
    }

    let entity_keys = [
        "position_tolerance",
        "flatness_tolerance",
        "straightness_tolerance",
        "roundness_tolerance",
        "cylindricity_tolerance",
        "line_profile_tolerance",
        "surface_profile_tolerance",
        "angularity_tolerance",
        "parallelism_tolerance",
        "perpendicularity_tolerance",
        "concentricity_tolerance",
        "coaxiality_tolerance",
        "symmetry_tolerance",
        "circular_runout_tolerance",
        "total_runout_tolerance",
        "geometric_tolerance",
        "modified_geometric_tolerance",
        "unequally_disposed_geometric_tolerance",
        "geometric_tolerance_with_datum_reference",
        "geometric_tolerance_with_modifiers",
        "plus_minus_tolerance",
        "tolerance_zone",
        "tolerance_zone_definition",
        "tolerance_value",
        "datum",
        "common_datum",
        "datum_reference",
        "datum_system",
        "datum_feature",
        "datum_target",
        "shape_aspect",
        "derived_shape_aspect",
        "composite_shape_aspect",
        "shape_aspect_relationship",
        "shape_aspect_associativity",
        "dimensional_location",
        "dimensional_size",
        "angular_location",
        "angular_size",
    ];
    let entities: Map<String, Value> = entity_keys
        .iter()
        .map(|name| (name.to_string(), json!(arena_len(&model, name))))
        .collect();

    let mut dims: Map<String, Value> = Map::new();
    for d in scene.dimensions() {
        let key = match d.kind() {
            step_io::scene::DimensionKind::Size => "Size",
            step_io::scene::DimensionKind::Location => "Location",
            step_io::scene::DimensionKind::AngularSize => "AngularSize",
            step_io::scene::DimensionKind::AngularLocation => "AngularLocation",
        };
        bump(&mut dims, key);
    }

    let tolerances = surfaced_tolerances(&scene);
    let datums = scene.datums().count();

    json!({
        "file": file,
        "path": path_str,
        "parse_ok": true,
        "schema": if schema.is_empty() { Value::Null } else { json!(schema) },
        "input_entities": report.n_in,
        "kept_entities": report.validated,
        "dropped": dropped_total,
        "dropped_by_type": dropped_by_type,
        "entities": entities,
        "surfaced": {
            "tolerances": tolerances,
            "datums": datums,
            "dimensions": dims,
        }
    })
}
