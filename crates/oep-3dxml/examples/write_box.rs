//! P3-M4-T01 example: write the reference unit box as a 3DXML ASCII package.
//!
//! Run: `cargo run -p oep-3dxml --example write_box -- <out.3dxml>`
//! The output is a complete, deterministic ZIP-archive 3DXML (Manifest.xml,
//! the model doc, and the tessellated ASCII `.3DRep`).

use std::io::Write;

fn main() -> std::process::ExitCode {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "box.3dxml".to_string());
    let positions: Vec<[f32; 3]> = vec![
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ];
    let triangles: Vec<[u32; 3]> = vec![
        [0, 1, 2],
        [0, 3, 1],
        [4, 5, 6],
        [5, 7, 6],
        [2, 6, 4],
        [2, 0, 6],
        [3, 5, 7],
        [3, 1, 5],
        [5, 1, 2],
        [5, 2, 4],
        [0, 7, 3],
        [0, 6, 7],
    ];
    let opts = oep_3dxml::ascii::WriterOpts {
        product_name: "box",
        author: "keelworks",
        timestamp: "2026-10-15",
    };
    let mut file = match std::fs::File::create(&out) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("cannot create {out}: {e}");
            return std::process::ExitCode::FAILURE;
        }
    };
    if let Err(e) = oep_3dxml::ascii::write_package(&positions, &triangles, &opts, &mut file) {
        eprintln!("3DXML write failed: {e}");
        return std::process::ExitCode::FAILURE;
    }
    file.flush().ok();
    println!(
        "wrote {out} ({} vertices / {} triangles)",
        positions.len(),
        triangles.len()
    );
    std::process::ExitCode::SUCCESS
}
