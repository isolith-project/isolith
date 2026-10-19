//! P2-M6-T02 standalone streaming-parse performance runner for large
//! assemblies (100k+ parts).
//!
//! Usage (from the repo root):
//!
//! ```text
//! cargo run --release -p oep-cgr --example stream_bench [PARTS] [DIR]
//! ```
//!
//! - `PARTS` (default 1_000_000): size of the synthesized assembly that the
//!   streaming walker must fully visit.
//! - `DIR` (optional): a directory of real `.cgr` files to stream once
//!   through the walker, reporting per-file nodes, bytes and wall time.
//!
//! Exits 0 when every walk is clean (fully consumed `Ok` on the synthetic
//! assembly; zero panics and node parity with the per-file slice reference on
//! real material). The synthetic leg prints records/sec — the headline
//! performance number this card proves.

use oep_cgr::stream::{walk_stream, Limits, Outcome};

/// Default assembly size: comfortably above the 100k+ parts acceptance bar.
const DEFAULT_PARTS: u64 = 1_000_000;

/// Slice-based reference (same framing + caps contract as
/// `structure::walk_probe`), used to cross-check streaming parity on real
/// files. Returns `(outcome_kind, node_count)`.
fn reference_walk(bytes: &[u8]) -> (String, u64) {
    let mut nodes = 0u64;
    let mut stack: Vec<(usize, usize)> = vec![(0, bytes.len())]; // (start,end)
    while let Some((start, end)) = stack.pop() {
        let mut cursor = start;
        while end - cursor >= 8 {
            let len = u32::from_le_bytes([
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]) as usize;
            if len as u64 > oep_cgr::stream::MAX_BLOCK_LEN as u64 {
                return ("length_limit".into(), nodes);
            }
            let payload_end = cursor + 8 + len;
            if payload_end > end {
                return ("truncated".into(), nodes);
            }
            nodes += 1;
            if payload_end > cursor + 8 {
                stack.push((cursor + 8, payload_end));
            }
            cursor = payload_end;
        }
    }
    ("ok".into(), nodes)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let parts = args
        .get(1)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_PARTS);
    let dir = args
        .get(if args.get(1).is_some_and(|s| s.parse::<u64>().is_ok()) {
            2
        } else {
            1
        })
        .map(String::as_str);

    let limits = Limits::default();

    // --- synthetic large-assembly leg (the performance contract) ---
    let mut assembly = Vec::with_capacity((parts * 8) as usize);
    for _ in 0..parts {
        assembly.extend_from_slice(&0x0BAD_CAFEu32.to_le_bytes());
        assembly.extend_from_slice(&0u32.to_le_bytes()); // len = 0
    }

    let start = std::time::Instant::now();
    let rep = walk_stream(
        &mut std::io::Cursor::new(&mut assembly),
        &limits,
        |_t, _l, _o| true,
    );
    let elapsed = start.elapsed();
    assert_eq!(rep.outcome, Outcome::Ok, "synthetic assembly must parse Ok");
    assert_eq!(rep.nodes, parts, "every part visited exactly once");
    let rate = if elapsed.as_secs_f64() > 0.0 {
        parts as f64 / elapsed.as_secs_f64()
    } else {
        f64::INFINITY
    };
    println!(
        "synthetic assembly: {parts} parts in {:.3?} => {:.0} records/sec (bounded memory, slice-free)",
        elapsed, rate
    );

    // --- optional real-material leg ---
    if let Some(dir) = dir {
        let entries = std::fs::read_dir(dir).expect("read dir");
        let mut files = 0usize;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("cgr") {
                continue;
            }
            files += 1;
            let bytes = std::fs::read(&path).expect("read sample");
            let (ref_kind, ref_nodes) = reference_walk(&bytes);
            let t0 = std::time::Instant::now();
            let rep = walk_stream(&mut std::io::Cursor::new(&bytes), &limits, |_t, _l, _o| {
                true
            });
            let dt = t0.elapsed();
            assert_eq!(
                rep.outcome.kind(),
                ref_kind,
                "{} streaming != slice reference kind",
                path.display()
            );
            assert_eq!(
                rep.nodes,
                ref_nodes,
                "{} streaming != slice ref",
                path.display()
            );
            println!(
                "real {:20} {:>8} B  nodes={:>3}  kind={:<13}  {:.3?}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                bytes.len(),
                rep.nodes,
                rep.outcome.kind(),
                dt
            );
        }
        println!("real-material: {files} file(s) streamed, all ok, node parity with slice ref");
    }

    println!("STREAM BENCH: PASS");
}
