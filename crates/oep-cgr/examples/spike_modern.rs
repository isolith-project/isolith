//! Temporary spike (P9-M3-T02): modern CGR decode on real material.
use oep_cgr::modern::extract_modern;

fn main() {
    let files = [
        "/home/r1a/.hermes/kanban/boards/keelworks/workspaces/t_4c51a021/real-corpus/nist_stc_06_asme1_V56R2023.CATPart",
        "/home/r1a/.hermes/kanban/boards/keelworks/workspaces/t_4c51a021/real-corpus/VALVE.cgr",
    ];
    for f in files {
        let bytes = match std::fs::read(f) {
            Ok(b) => b,
            Err(e) => {
                println!("{f}: READ_ERR {e}");
                continue;
            }
        };
        match extract_modern(&bytes) {
            Ok((counts, m)) => println!(
                "{f}: OK mesh index_count={} vertex_count={} triangle_count={} max_index={}",
                counts.index_count,
                counts.vertex_count,
                counts.triangle_count,
                counts.max_index
            ),
            Err(e) => println!("{f}: {:?}", e),
        }
    }
}
