//! P2-M6-T02 acceptance test (TDD, RED first) — Performance pass: streaming
//! parse for large assemblies (100k+ parts).
//!
//! Written BEFORE `oep_cgr::stream` existed (RED: the crate exports no
//! `stream` module, so this file does not compile against main). The
//! contract, once GREEN, is:
//!
//! 1. `stream::walk_stream(reader, limits, visitor)` is a TRUE streaming
//!    record walker: it reads the depth-first LE `u32 tag + u32 len +
//!    payload` framing incrementally from a `std::io::Read` with bounded
//!    memory (constant buffer + depth stack), never materializing a record
//!    payload and never requiring the whole file in memory. It returns a
//!    `ProbeReport` with a classified `Outcome` and never panics on any
//!    input (the P2-M1-T04 tolerant doctrine applied to streaming).
//! 2. Correctness parity: on identical bytes, `walk_stream` over a chunked
//!    reader (tiny read buffer that still yields the same stream) produces
//!    the SAME outcome kind, node count and max depth as the slice-based
//!    reference walker re-implemented locally from the documented framing
//!    hypothesis (P1-M2-T01 hex notebook; P2-M6-T01 `structure::walk_probe`
//!    contract). A stream that hands out only a few bytes per `read()` must
//!    parse identically to one read whole — that is the streaming property.
//! 3. Large assemblies: a synthesized 100k+ part assembly (each part an
//!    8-byte record header in an unbounded root container) streams through
//!    with every record visited exactly once, `nodes == parts`, within a
//!    bounded wall-clock budget, and the reader never requests a
//!    whole-file slurp (peak read-request size stays inside a small cap) —
//!    the memory-boundedness evidence.
//! 4. Never-panic: garbage, truncated, depth and huge-length inputs all
//!    classify; none panics.
//! 5. Real material (when `OEP_CGR_SAMPLES` supplies the never-committed
//!    corpus): every real `.cgr` streams with zero panics and the SAME node
//!    count as the slice reference on the same bytes; SKIPs cleanly in CI
//!    without the private bins.
//!
//! Clean-room rule honoured (board doctrine): real CGR binaries are never
//! committed. Every synthesized input derives from the observed, public
//! framing facts (LE `u32 tag + u32 len + payload`, 8-byte record header,
//! depth-first, implicit lengths) and deterministic synthetic payloads.

#![deny(missing_docs)]

use oep_cgr::stream::{walk_stream, Limits, Outcome};

/// Fixed assembly size for the "100k+ parts" leg. Deliberately non-round and
/// above 100_000 to prove the 100k+ headroom, kept small enough that the
/// test runs in milliseconds so CI stays fast.
const PARTS: u64 = 256_000;

/// Generous CI-safe wall-clock budget for the large-assembly leg (debug
/// build, shared runner). Real timing is ~milliseconds; this floor only
/// catches pathological quadratic/whole-file behaviour.
const LARGE_ASSEMBLY_BUDGET_SECS: u64 = 30;

/// A `Read` that hands the caller at most `cap` bytes per `read()` call, and
/// records every request size it receives. Forcing tiny chunks proves the
/// walker needs no look-ahead beyond its own small buffer (true streaming),
/// and the request log is the memory-boundedness evidence.
struct ChunkedReader<'a> {
    data: &'a [u8],
    pos: usize,
    cap: usize,
    max_request: usize,
    requests: u64,
}

impl<'a> ChunkedReader<'a> {
    fn new(data: &'a [u8], cap: usize) -> Self {
        Self {
            data,
            pos: 0,
            cap,
            max_request: 0,
            requests: 0,
        }
    }
}

impl std::io::Read for ChunkedReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        self.requests += 1;
        self.max_request = self.max_request.max(buf.len());
        let n = (self.data.len() - self.pos).min(self.cap).min(buf.len());
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Slice-based reference walker re-derived from the P1-M2-T01 framing
/// hypothesis (the same contract `structure::walk_probe` from P2-M6-T01
/// implements, including the per-record length cap). Used to assert streaming
/// parity on identical bytes. Returns
/// `(outcome_kind, nodes, max_depth_reached)`.
fn reference_walk(bytes: &[u8], depth_cap: usize, node_cap: u64) -> (String, u64, usize) {
    let mut nodes = 0u64;
    let mut max_depth = 0usize;
    let mut stack: Vec<(usize, usize, usize)> = vec![(0, bytes.len(), 0)]; // (start,end,depth)
    while let Some((start, end, cdepth)) = stack.pop() {
        let mut cursor = start;
        while end - cursor >= 8 {
            let len = u32::from_le_bytes([
                bytes[cursor + 4],
                bytes[cursor + 5],
                bytes[cursor + 6],
                bytes[cursor + 7],
            ]) as usize;
            let rdepth = cdepth + 1;
            if rdepth > depth_cap {
                return ("depth_limit".into(), nodes, max_depth);
            }
            if len as u64 > oep_cgr::stream::MAX_BLOCK_LEN as u64 {
                return ("length_limit".into(), nodes, max_depth);
            }
            let payload_end = cursor + 8 + len;
            if payload_end > end {
                return ("truncated".into(), nodes, max_depth);
            }
            if nodes >= node_cap {
                return ("block_budget".into(), nodes, max_depth);
            }
            nodes += 1;
            max_depth = max_depth.max(rdepth);
            if payload_end > cursor + 8 {
                stack.push((cursor + 8, payload_end, rdepth));
            }
            cursor = payload_end;
        }
    }
    ("ok".into(), nodes, max_depth)
}

/// Build a synthetic large-assembly byte stream: `n` part records, each an
/// 8-byte header (`tag` LE, `len = 0` LE) in the root container. Empty
/// payload means each part contributes exactly one record node.
fn synthetic_assembly(parts: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((parts * 8) as usize);
    let tag: u32 = 0xBAD_CAFE;
    for _ in 0..parts {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes()); // len = 0
    }
    bytes
}

/// Build a small hand-crafted mix for parity checking: flat records, a
/// nested record chain, and a truncated container — all from synthetic data.
fn parity_samples() -> Vec<Vec<u8>> {
    let mut flat = Vec::new();
    for i in 0..16u32 {
        flat.extend_from_slice(&i.to_le_bytes());
        flat.extend_from_slice(&4u32.to_le_bytes());
        flat.extend_from_slice(&[0u8, 1, 2, 3]);
    }
    // nested: outer( len covers inner( len covers innermost ) )
    let mut inner = Vec::new();
    inner.extend_from_slice(&9u32.to_le_bytes());
    inner.extend_from_slice(&0u32.to_le_bytes());
    let mut outer = Vec::new();
    outer.extend_from_slice(&7u32.to_le_bytes());
    outer.extend_from_slice(&(inner.len() as u32).to_le_bytes());
    outer.extend_from_slice(&inner);
    let mut nested = Vec::new();
    nested.extend_from_slice(&5u32.to_le_bytes());
    nested.extend_from_slice(&(outer.len() as u32).to_le_bytes());
    nested.extend_from_slice(&outer);

    // truncated container: declares a payload longer than present.
    let mut truncated = Vec::new();
    truncated.extend_from_slice(&3u32.to_le_bytes());
    truncated.extend_from_slice(&(100u32).to_le_bytes());
    truncated.extend_from_slice(&[0u8; 4]);

    // depth chain 300 (past the 256 cap).
    let mut chain = Vec::new();
    for level in 0..300u32 {
        let len = u32::try_from(chain.len()).unwrap_or(0);
        let mut rec = Vec::with_capacity(8 + chain.len());
        rec.extend_from_slice(&level.to_le_bytes());
        rec.extend_from_slice(&len.to_le_bytes());
        rec.extend_from_slice(&chain);
        chain = rec;
    }

    // garbage + empty.
    vec![
        flat,
        nested,
        truncated,
        chain,
        vec![0xFF; 1024],
        Vec::new(),
        vec![1, 2, 3],
    ]
}

// ---------------------------------------------------------------------------
// 1. Streaming parity with the slice reference on identical bytes.
// ---------------------------------------------------------------------------

#[test]
fn streaming_parity_with_slice_reference() {
    let limits = Limits::default();
    let samples = parity_samples();
    for (i, bytes) in samples.iter().enumerate() {
        // Slice reference (what `structure::walk_probe` guarantees).
        let (ref_kind, ref_nodes, ref_depth) = reference_walk(bytes, 256, 1_000_000);
        // Streaming over a 3-byte-per-read chunk (forces many tiny reads) and
        // over a whole-buffer cursor — both must agree with the reference.
        for cap in [3usize, 8, 256] {
            let mut rdr = ChunkedReader::new(bytes, cap);
            let rep = walk_stream(&mut rdr, &limits, |_tag, _len, _off| true);
            assert_eq!(
                rep.outcome.kind(),
                ref_kind,
                "sample {i} cap {cap}: outcome kind mismatch (ref {ref_kind})"
            );
            assert_eq!(
                rep.nodes, ref_nodes,
                "sample {i} cap {cap}: node count mismatch (ref {ref_nodes})"
            );
            assert_eq!(
                rep.max_depth_reached, ref_depth,
                "sample {i} cap {cap}: depth mismatch (ref {ref_depth})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Large assembly: every record visited exactly once, in bounded time.
// ---------------------------------------------------------------------------

#[test]
fn large_assembly_100k_visits_every_part_once() {
    let limits = Limits::default();
    let bytes = synthetic_assembly(PARTS);
    let mut rdr = ChunkedReader::new(&bytes, 64); // 64-byte chunks: true streaming

    let start = std::time::Instant::now();
    let rep = walk_stream(&mut rdr, &limits, |_tag, _len, _off| true);
    let elapsed = start.elapsed();

    assert_eq!(
        rep.outcome.kind(),
        "ok",
        "healthy assembly must classify ok"
    );
    assert_eq!(rep.nodes, PARTS, "every part visited exactly once");
    assert_eq!(rep.max_depth_reached, 1, "parts sit at root depth 1");
    assert!(
        elapsed.as_secs() < LARGE_ASSEMBLY_BUDGET_SECS,
        "large assembly took {elapsed:?}, over the {LARGE_ASSEMBLY_BUDGET_SECS}s budget"
    );
}

/// The memory-boundedness evidence: even when the reader only hands out a
/// few bytes at a time, the walker's peak single read-request stays inside a
/// small constant cap (never a whole-file slurp), and it parses fine.
#[test]
fn streaming_memory_is_bounded_not_whole_file() {
    let limits = Limits::default();
    let bytes = synthetic_assembly(PARTS);
    let mut rdr = ChunkedReader::new(&bytes, 7);
    let rep = walk_stream(&mut rdr, &limits, |_tag, _len, _off| true);
    assert_eq!(rep.nodes, PARTS);
    // Peak request must be bounded: the header read is 8 bytes; a
    // whole-file slurp (read_to_end) would request everything at once.
    assert!(
        rdr.max_request <= 64 * 1024,
        "walker requested {}-byte reads; expected a small constant cap",
        rdr.max_request
    );
    assert!(rdr.requests > 0, "stream was actually read");
}

// ---------------------------------------------------------------------------
// 3. Tolerant / never-panic on adversarial synthetic inputs.
// ---------------------------------------------------------------------------

#[test]
fn streaming_never_panics_and_classifies() {
    let limits = Limits::default();
    // Depth beyond cap -> depth_limit, block budget -> block_budget, and
    // random bytes of many sizes -> *something* classified, zero panics.
    let mut chain = Vec::new();
    for level in 0..500u32 {
        let len = u32::try_from(chain.len()).unwrap_or(0);
        let mut rec = Vec::with_capacity(8 + chain.len());
        rec.extend_from_slice(&level.to_le_bytes());
        rec.extend_from_slice(&len.to_le_bytes());
        rec.extend_from_slice(&chain);
        chain = rec;
    }
    for bytes in [chain, vec![0u8; 4_000_000], vec![0xFF; 65_537]] {
        let mut rdr = ChunkedReader::new(&bytes, 31);
        let rep = walk_stream(&mut rdr, &limits, |_tag, _len, _off| true);
        let kind = rep.outcome.kind();
        assert!(
            matches!(
                rep.outcome,
                Outcome::Ok
                    | Outcome::Truncated { .. }
                    | Outcome::LengthLimitExceeded { .. }
                    | Outcome::DepthLimitExceeded { .. }
                    | Outcome::BlockBudgetExceeded { .. }
            ),
            "unclassified outcome {kind}"
        );
    }
}

#[test]
fn streaming_block_budget_trips_and_stops() {
    // A huge flat assembly past the node cap must stop at the cap (bounded
    // CPU), not parse forever.
    let cap: u64 = 1_000;
    let bytes = synthetic_assembly(1_000_000);
    let limits = Limits {
        max_blocks: cap,
        ..Limits::default()
    };
    let mut rdr = ChunkedReader::new(&bytes, 4096);
    let rep = walk_stream(&mut rdr, &limits, |_tag, _len, _off| true);
    assert_eq!(rep.outcome.kind(), "block_budget");
    assert!(
        rep.nodes <= cap,
        "nodes {} must not exceed the cap {cap}",
        rep.nodes
    );
}

// ---------------------------------------------------------------------------
// 4. Real material (private corpus via OEP_CGR_SAMPLES; SKIP without it).
// ---------------------------------------------------------------------------

#[test]
fn real_material_streams_with_zero_panics_and_matches_slice() {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        for chunk in env.split([':', ',']) {
            let c = chunk.trim();
            if !c.is_empty() {
                dirs.push(std::path::PathBuf::from(c));
            }
        }
    }
    dirs.push(std::path::PathBuf::from("samples"));
    dirs.push(std::path::PathBuf::from("../samples"));

    let limits = Limits::default();
    let mut files = 0usize;
    for d in &dirs {
        if !d.is_dir() {
            continue;
        }
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("cgr") {
                continue;
            }
            files += 1;
            let bytes = std::fs::read(&path).expect("read sample");
            let (ref_kind, ref_nodes, _ref_depth) = reference_walk(&bytes, 256, 1_000_000);
            let mut rdr = ChunkedReader::new(&bytes, 16);
            let rep = walk_stream(&mut rdr, &limits, |_t, _l, _o| true);
            assert_eq!(
                rep.outcome.kind(),
                ref_kind,
                "{} streaming outcome != slice reference",
                path.display()
            );
            assert_eq!(
                rep.nodes,
                ref_nodes,
                "{} streaming node count != slice reference",
                path.display()
            );
            println!(
                "PASS streaming {}({} B) nodes={}",
                path.display(),
                bytes.len(),
                rep.nodes
            );
        }
    }
    if files == 0 {
        println!("SKIP real-material leg (no OEP_CGR_SAMPLES; CI stays green)");
        return;
    }
    println!("PASS real-material streaming on {files} file(s), zero panics");
}
