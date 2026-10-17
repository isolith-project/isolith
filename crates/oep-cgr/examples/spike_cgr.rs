//! Temporary spike (P9-M3-T02): does oep_cgr::mesh::extract handle real CGR?
use oep_cgr::mesh::{extract, MeshError};

fn main() {
    let files = ["/home/r1a/Downloads/VALVE.cgr"];
    for f in files {
        let bytes = std::fs::read(f).expect("read cgr");
        match extract(&bytes) {
            Ok(m) => println!(
                "{f}: OK {} vertices {} triangles",
                m.vertices().len(),
                m.triangles().len()
            ),
            Err(e) => match e {
                MeshError::NotCgrPreamble => println!("{f}: NOT_CGR {e}"),
                other => println!("{f}: SKIP {other}"),
            },
        }
    }
}
