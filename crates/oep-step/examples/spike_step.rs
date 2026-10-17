//! Temporary spike (P9-M3-T02): parse arbitrary STEP files from argv.
fn main() {
    for f in std::env::args().skip(1) {
        let bytes = match std::fs::read(&f) {
            Ok(b) => b,
            Err(e) => {
                println!("{f}: READ_ERR {e}");
                continue;
            }
        };
        match oep_step::parse(&bytes) {
            Ok(out) => println!(
                "{f}: OK name={} verts={} tris={} shells={}",
                out.facets.product_name,
                out.facets.vertices.len(),
                out.facets.triangles.len(),
                out.facets.shells.len()
            ),
            Err(e) => println!("{f}: ERR {e}"),
        }
    }
}
