//! P2-M4-T01 demo driver: scan a real CGR file and export its observed
//! color/material records as a glTF 2.0 `materials` JSON document.
//!
//! Usage: `cargo run -p oep-cgr --example dump_recs -- <dir-with-VALVE.cgr>`
//! Prints the glTF materials JSON to stdout (16 materials for VALVE.cgr on
//! the real corpus). No proprietary bytes are written; only the generated
//! glTF materials document leaves the crate.

fn main() {
    let dir = std::env::args()
        .nth(1)
        .expect("usage: dump_recs <dir containing VALVE.cgr>");
    let path = format!("{dir}/VALVE.cgr");
    let bytes = std::fs::read(&path).expect("read VALVE.cgr");
    let recs = oep_cgr::material::scan_color_records(&bytes);
    let json = oep_cgr::material::to_gltf_materials_json(&recs);
    println!("{}", json);
}
