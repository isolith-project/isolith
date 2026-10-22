//! oep-analysis CLI (editor-spine analysis, decision #19 pull-forward).
//!
//! Agent/automation lane for the mass-properties / clash / exploded-view
//! analysis crate. The editor surface (oep-shell + oep-viewer, being built as
//! the interactive editor by INTERACTIVE-SHELL) drives the same library
//! functions; this binary is the machine-callable face that automation and
//! tests use. It is deliberately small and deterministic.
//!
//! Usage:
//!   oep-analysis-cli <doc.oepgeo.json> --material <materials.json> [--asm ID]
//!       [--pull DISTANCE] [--explode-out OUT.oepgeo.json]
//!
//! With no options other than the two positional files, it prints a JSON
//! report: per-solid-body mass properties, every clashed pair, and the
//! exploded placements at the given pull distance (default 0). With
//! `--explode-out`, it also writes the exploded document (renderable by the
//! existing viewer scene pipeline).
//!
//! Exit codes: 0 = all good; 2 = a strictly non-finite pull distance; 3 =
//! assembly not found; 4 = file/JSON error.

use oep_analysis::clash;
use oep_analysis::explode;
use oep_analysis::mass;
use oep_analysis::material::MaterialTable;
use oep_geo::Document;
use std::io::Write;

const EXIT_OK: i32 = 0;
const EXIT_PULL: i32 = 2;
const EXIT_ASM: i32 = 3;
const EXIT_FILE: i32 = 4;

fn fail(code: i32, msg: &str) -> ! {
    eprintln!("oep-analysis-cli: {msg}");
    std::process::exit(code);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut doc_path: Option<String> = None;
    let mut material_path: Option<String> = None;
    let mut asm: Option<String> = None;
    let mut pull: f64 = 0.0;
    let mut explode_out: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--material" => {
                i += 1;
                material_path = Some(args[i].clone());
            }
            "--asm" => {
                i += 1;
                asm = Some(args[i].clone());
            }
            "--pull" => {
                i += 1;
                pull = args[i]
                    .parse()
                    .unwrap_or_else(|_| fail(EXIT_PULL, "pull distance must be a number"));
            }
            "--explode-out" => {
                i += 1;
                explode_out = Some(args[i].clone());
            }
            other if other.starts_with("--") => {
                fail(EXIT_FILE, &format!("unknown option {other:?}"));
            }
            other => {
                if doc_path.is_none() {
                    doc_path = Some(other.to_string());
                } else {
                    fail(EXIT_FILE, "too many positional args");
                }
            }
        }
        i += 1;
    }

    let doc_path = doc_path.unwrap_or_else(|| fail(EXIT_FILE, "missing <doc.oepgeo.json> arg"));
    let doc_text = std::fs::read_to_string(&doc_path)
        .unwrap_or_else(|e| fail(EXIT_FILE, &format!("cannot read {doc_path}: {e}")));
    let doc: Document = Document::from_json_pretty(&doc_text)
        .unwrap_or_else(|e| fail(EXIT_FILE, &format!("cannot parse {doc_path}: {e}")));

    let materials = match &material_path {
        Some(p) => {
            let text = std::fs::read_to_string(p)
                .unwrap_or_else(|e| fail(EXIT_FILE, &format!("cannot read {p}: {e}")));
            MaterialTable::from_json_pretty(&text)
                .unwrap_or_else(|e| fail(EXIT_FILE, &format!("cannot parse {p}: {e}")))
        }
        None => MaterialTable::new("mm"),
    };

    // The assembly to analyze: first assembly when unspecified.
    let asm = asm.unwrap_or_else(|| {
        doc.assemblies
            .first()
            .map(|a| a.id.clone())
            .unwrap_or_else(|| fail(EXIT_ASM, "document has no assemblies; pass --asm"))
    });

    // Mass properties for every solid body (density from material metadata).
    let density_resolver = |body_id: &str| materials.density_g_cm3_for_body(body_id);
    let mut mass_report: Vec<serde_json::Value> = Vec::new();
    for body in &doc.bodies {
        match mass::body_mass_properties(&doc, &density_resolver, &body.id) {
            Ok(props) => mass_report.push(serde_json::json!({
                "body": body.id,
                "volume_mm3": props.volume_mm3,
                "mass_g": props.mass_g,
                "centroid": props.centroid,
                "inertia" : props.inertia,
            })),
            Err(e) => mass_report.push(serde_json::json!({
                "body": body.id,
                "error": format!("{e}"),
            })),
        }
    }

    // Clash detection.
    let clashes = match clash::detect_in_assembly(&doc, &asm) {
        Ok(c) => c
            .iter()
            .map(|c| {
                serde_json::json!({
                    "a": { "chain": c.chain_a, "part": c.part_a, "body": c.body_a },
                    "b": { "chain": c.chain_b, "part": c.part_b, "body": c.body_b },
                    "overlap": { "min": c.overlap.min, "max": c.overlap.max },
                    "location": c.location,
                    "volume": c.volume,
                })
            })
            .collect::<Vec<_>>(),
        Err(clash::ClashError::AssemblyNotFound(id)) => {
            fail(EXIT_ASM, &format!("assembly {id:?} not found"))
        }
        Err(e) => fail(EXIT_ASM, &format!("clash check failed: {e:?}")),
    };

    // Exploded view at the requested pull distance.
    let placements = match explode::explode(&doc, &asm, pull) {
        Ok(p) => p,
        Err(explode::ExplodeError::AssemblyNotFound(id)) => {
            fail(EXIT_ASM, &format!("assembly {id:?} not found"))
        }
        Err(explode::ExplodeError::NonFinitePull(v)) => {
            fail(EXIT_PULL, &format!("non-finite pull distance {v}"))
        }
        Err(e) => fail(EXIT_ASM, &format!("explode failed: {e:?}")),
    };
    let placements_json = placements
        .iter()
        .map(|p| {
            serde_json::json!({
                "instance": p.instance,
                "depth": p.depth,
                "direction": p.direction,
                "offset": p.offset,
                "transform": p.transform.components(),
            })
        })
        .collect::<Vec<_>>();

    let report = serde_json::json!({
        "format": "oep-analysis-report",
        "version": 1,
        "assembly": asm,
        "pull_distance": pull,
        "mass_properties": mass_report,
        "clashes": clashes,
        "exploded": placements_json,
    });

    let mut out = serde_json::to_string_pretty(&report).expect("report serializes");
    out.push('\n');
    let stdout = std::io::stdout();
    let mut w = stdout.lock();
    w.write_all(out.as_bytes()).expect("stdout writable");

    if let Some(out_path) = &explode_out {
        let exploded = explode::apply_placements(&doc, &asm, &placements);
        match exploded {
            Ok(d) => {
                let text = d
                    .to_json_pretty()
                    .map_err(|e| fail(EXIT_FILE, &format!("cannot serialize exploded doc: {e}")))
                    .expect("serialize");
                std::fs::write(out_path, text)
                    .unwrap_or_else(|e| fail(EXIT_FILE, &format!("cannot write {out_path}: {e}")));
            }
            Err(e) => fail(EXIT_ASM, &format!("explode-out failed: {e:?}")),
        }
    }

    std::process::exit(EXIT_OK);
}
