//! P2-M6-T01 fuzz campaign round 2: structure depth, huge files, truncated
//! tails.
//!
//! A deterministic, self-contained campaign that runs the tolerant record
//! walker ([`crate::structure`]) over synthesized adversarial inputs in the
//! three round-2 classes and asserts the contract: every input yields a
//! classified [`crate::structure::Outcome`] and none panics.
//!
//! Clean-room: every synthesized input is derived from the PUBLIC observed
//! CGR header facts (magic `01 00 00 00`, 0x60-byte zero preamble, landscape
//! u32, 16-byte invariant TEMPLATE) recorded in
//! `docs/re-notes/oep-cgr-header-parse.md`; real CGR binaries are never
//! created or committed. Real material is only *read* (never stored) through
//! [`sample_dirs`] / [`run_campaign_on_samples`].
//!
//! The campaign runs on the pinned STABLE toolchain (no nightly, no
//! libFuzzer required): generators + a seeded in-process loop + a report.
//! A workspace-excluded `cargo-fuzz`/nightly leg for full coverage fuzzing is
//! provided under `crates/oep-cgr/fuzz/` (round-1 heritage, P2-M1-T05).

use crate::structure::{walk_probe, Limits, Outcome};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Observed CGR magic, `01 00 00 00` at offset 0 (published in the header
/// RE note; u32 LE reads as 1).
const MAGIC: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

/// Observed invariant 16-byte template at 0x68..0x78, byte-for-byte identical
/// across all eleven corpus samples (published in the header RE note).
const TEMPLATE: [u8; 16] = [
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
];

/// Observed total CGR header-spine length (0x00..0x78), published fact.
const HEADER_LEN: usize = 0x78;

/// Observed landscape literal for the R19-era cfysa set (u32 LE ==
/// 0x10CD3600), held as the canonical sample's landscape value.
const LANDSCAPE: u32 = 0x10CD_3600;

/// Build the canonical 0x78-byte CGR header spine from observed-public facts
/// only (no proprietary bytes). Used as the seed for huge-tail and
/// truncation inputs.
pub fn canonical_header() -> Vec<u8> {
    let mut b = vec![0u8; HEADER_LEN];
    b[0..4].copy_from_slice(&MAGIC);
    b[0x64..0x68].copy_from_slice(&LANDSCAPE.to_le_bytes());
    b[0x68..0x78].copy_from_slice(&TEMPLATE);
    b
}

/// Build a deterministic `len`-byte synthetic CGR "sample": the canonical
/// header plus a deterministic pattern payload (fails neutral on clamp).
pub fn canonical_sample(len: usize) -> Vec<u8> {
    let mut b = canonical_header();
    let need = len.saturating_sub(b.len());
    for i in 0..need {
        b.push(((i * 37) ^ 0x2C) as u8);
    }
    b
}

/// Build a nested record chain `depth` records deep, 8 bytes of record header
/// per level (`tag = level`, `len = payload len`). The walker's depth probe
/// input. Building is O(depth) time and O(depth) memory.
pub fn nested_records(depth: usize) -> Vec<u8> {
    let mut inner = Vec::new();
    for level in 0..depth {
        let tag = u32::try_from(level).unwrap_or(0x0B_AD_B0_0B);
        let len = u32::try_from(inner.len()).unwrap_or(u32::MAX);
        let mut rec = Vec::with_capacity(8 + inner.len());
        rec.extend_from_slice(&tag.to_le_bytes());
        rec.extend_from_slice(&len.to_le_bytes());
        rec.extend_from_slice(&inner);
        inner = rec;
    }
    inner
}

/// Campaign configuration. `Default` is sized for CI speed; the standalone
/// runner uses heavier values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignConfig {
    /// PRNG seed; a fixed seed yields a byte-for-byte identical report.
    pub seed: u64,
    /// Number of mutation (round-1 heritage) iterations.
    pub iters: u64,
    /// Size of the synthetic zero tail used by the huge-file leg.
    pub huge_tail_bytes: usize,
    /// Step used when sweeping truncation points through an input.
    pub truncation_step: usize,
    /// Deepest nested record built by the structure-depth leg.
    pub depth_sweep_max: usize,
}

impl Default for CampaignConfig {
    fn default() -> Self {
        Self {
            seed: 0x4B_45_45_4C_F0_0D_52_02,
            iters: 5_000,
            huge_tail_bytes: 8 * 1024 * 1024,
            truncation_step: 7,
            depth_sweep_max: 300,
        }
    }
}

/// Aggregate result of one campaign run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CampaignReport {
    /// Total inputs walked (deterministic legs + real material).
    pub inputs: u64,
    /// Inputs on which the walker PANICKED. The contract is exactly 0.
    pub panics: u64,
    /// Description of the first panic (class + input length), if any.
    pub first_panic: Option<String>,
    /// Outcome-kind histogram (`Outcome::kind()` -> count).
    pub outcomes: BTreeMap<&'static str, u64>,
    /// Input counts per generator class (`depth/huge/truncated/mutation/
    /// canonical/real`).
    pub class_counts: BTreeMap<&'static str, u64>,
    /// True if any input tripped the depth cap.
    pub saw_depth_limit: bool,
    /// True if any input tripped the per-record length cap.
    pub saw_length_limit: bool,
    /// True if any input truncated mid-structure.
    pub saw_truncated: bool,
    /// Peak nodes visited on a single input (bounds evidence).
    pub max_nodes: u64,
    /// Real `.cgr` files scanned by [`run_campaign_on_samples`].
    pub files_scanned: u64,
}

/// Locations to search for real CGR samples: `$OEP_CGR_SAMPLES`
/// (colon/comma/semicolon separated), then `samples`, then `../samples`.
pub fn sample_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        for chunk in env.split([':', ',', ';']) {
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                dirs.push(PathBuf::from(chunk));
            }
        }
    }
    dirs.push(PathBuf::from("samples"));
    dirs.push(PathBuf::from("../samples"));
    dirs
}

/// Run the full deterministic round-2 campaign (all legs) under `cfg`.
///
/// Returns a [`CampaignReport`]; the caller decides pass/fail
/// (`panics == 0` is the invariant). A fixed seed reproduces an identical
/// report.
pub fn run_campaign(cfg: &CampaignConfig) -> CampaignReport {
    let mut report = CampaignReport::default();
    let limits = Limits::default();
    let mut rng = XorShift64::new(cfg.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));

    // canonical baselines
    let hdr = canonical_header();
    run_one(&mut report, "canonical", &hdr, &limits);
    run_one(&mut report, "canonical", &canonical_sample(1024), &limits);
    run_one(&mut report, "canonical", &canonical_sample(65536), &limits);

    // structure-depth leg
    for d in 0..=cfg.depth_sweep_max {
        run_one(&mut report, "depth", &nested_records(d), &limits);
    }

    // huge-file leg
    let mut bomb = Vec::new();
    bomb.extend_from_slice(&1u32.to_le_bytes());
    bomb.extend_from_slice(&u32::MAX.to_le_bytes());
    run_one(&mut report, "huge", &bomb, &limits);
    let mut tail = canonical_header();
    tail.extend(std::iter::repeat_n(0u8, cfg.huge_tail_bytes));
    run_one(&mut report, "huge", &tail, &limits);
    // Raw zero container (not header-anchored): a (tag=0,len=0) record
    // stream that trips the block budget directly when huge_tail_bytes makes
    // it exceed MAX_BLOCKS — exercises the BlockBudgetExceeded path inside
    // the campaign itself, not only in the acceptance probes.
    run_one(
        &mut report,
        "huge",
        &vec![0u8; cfg.huge_tail_bytes.max(0x1000)],
        &limits,
    );
    run_one(
        &mut report,
        "huge",
        &canonical_sample(cfg.huge_tail_bytes.max(0x1000)),
        &limits,
    );

    // truncated-tail leg (sweep + sharp tail window)
    let bases = [
        canonical_header(),
        nested_records(16),
        canonical_sample(8192),
    ];
    let step = cfg.truncation_step.max(1);
    for base in &bases {
        let full = base.len();
        for k in (0..=full).step_by(step) {
            run_one(&mut report, "truncated", &base[..k], &limits);
        }
        let start = full.saturating_sub(64);
        for k in start..=full {
            run_one(&mut report, "truncated", &base[..k], &limits);
        }
    }

    // mutation leg (round-1 heritage: seeded byte mutation of real-adjacent
    // canonical bytes)
    let m1 = canonical_sample(2048);
    let m2 = nested_records(24);
    for _ in 0..cfg.iters {
        let base = if rng.next_u64() & 1 == 0 { &m1 } else { &m2 };
        let mut bytes = base.clone();
        mutate(&mut bytes, &mut rng);
        run_one(&mut report, "mutation", &bytes, &limits);
    }

    report
}

/// Run the deterministic campaign AND walk every real `.cgr` reachable in
/// `dirs`, reading at most [`REAL_FILE_READ_CAP`] bytes per file (a larger
/// real file is exercised as a truncated tail, consistent with the legacy).
/// Real bytes are never stored or committed; only classification counts.
pub fn run_campaign_on_samples(cfg: &CampaignConfig, dirs: &[PathBuf]) -> CampaignReport {
    let mut report = run_campaign(cfg);
    let limits = Limits::default();
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("cgr") {
                continue;
            }
            report.files_scanned += 1;
            let cap = REAL_FILE_READ_CAP;
            match read_capped(&path, cap) {
                Ok(bytes) => run_one(&mut report, "real", &bytes, &limits),
                Err(_) => {
                    // IO failure on an optional sample: not a parser event,
                    // skip without counting as a panic.
                    report.files_scanned -= 1;
                }
            }
        }
    }
    report
}

/// Cap applied when reading a real sample for the real-material leg (32 MiB).
/// Real corpus files are ~36 KiB..192 KiB; the cap keeps an adversarial
/// stand-in bounded without losing the truncated-tail exercise.
pub const REAL_FILE_READ_CAP: usize = 32 * 1024 * 1024;

/// Read up to `cap` bytes of `path` (deterministic truncation semantics).
fn read_capped(path: &std::path::Path, cap: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = Vec::with_capacity(cap.min(1 << 20));
    let mut chunk = [0u8; 64 * 1024];
    loop {
        if buf.len() >= cap {
            break;
        }
        let want = (cap - buf.len()).min(chunk.len());
        let n = f.read(&mut chunk[..want])?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    Ok(buf)
}

/// Walk one input, updating `report`. The walk is wrapped in
/// `catch_unwind`: a panic on any untrusted input IS a campaign failure and
/// is counted in `panics` (the contract requires 0).
fn run_one(report: &mut CampaignReport, class: &'static str, bytes: &[u8], limits: &Limits) {
    report.inputs += 1;
    *report.class_counts.entry(class).or_insert(0) += 1;
    let attempt =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| walk_probe(bytes, limits)));
    match attempt {
        Err(_) => {
            report.panics += 1;
            if report.first_panic.is_none() {
                report.first_panic = Some(format!(
                    "class={class} len={} seed_rng=see-config",
                    bytes.len()
                ));
            }
        }
        Ok(rep) => {
            *report.outcomes.entry(rep.outcome.kind()).or_insert(0) += 1;
            report.max_nodes = report.max_nodes.max(rep.nodes);
            match rep.outcome {
                Outcome::DepthLimitExceeded { .. } => report.saw_depth_limit = true,
                Outcome::LengthLimitExceeded { .. } => report.saw_length_limit = true,
                Outcome::Truncated { .. } => report.saw_truncated = true,
                _ => {}
            }
        }
    }
}

/// Apply 1..=4 random byte mutations to `bytes` (flip/set/truncate/insert/
/// duplicate/delete). Bounded so inputs stay small in the mutation leg.
fn mutate(bytes: &mut Vec<u8>, rng: &mut XorShift64) {
    let ops = 1 + rng.next_usize(4);
    for _ in 0..ops {
        match rng.next_usize(6) {
            0 => {
                if !bytes.is_empty() {
                    let i = rng.next_usize(bytes.len());
                    bytes[i] ^= 1u8 << rng.next_usize(8);
                }
            }
            1 => {
                if !bytes.is_empty() {
                    let i = rng.next_usize(bytes.len());
                    bytes[i] = rng.next_byte();
                }
            }
            2 => {
                if !bytes.is_empty() {
                    let k = rng.next_usize(bytes.len() + 1);
                    bytes.truncate(k);
                }
            }
            3 => {
                if bytes.len() < 1 << 20 {
                    let i = rng.next_usize(bytes.len() + 1);
                    bytes.insert(i, rng.next_byte());
                }
            }
            4 => {
                // duplicate a random slice onto the end (bounded).
                if !bytes.is_empty() && bytes.len() < 1 << 20 {
                    let a = rng.next_usize(bytes.len());
                    let b = rng.next_usize(bytes.len() - a) + a;
                    let copy = bytes[a..b].to_vec();
                    bytes.extend_from_slice(&copy);
                }
            }
            _ => {
                if !bytes.is_empty() {
                    let i = rng.next_usize(bytes.len());
                    bytes.remove(i);
                }
            }
        }
    }
}

/// Deterministic xorshift64* PRNG. Inline (no crate dependency) so the
/// campaign adds zero new dependencies and stays reproducible forever.
struct XorShift64(u64);

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn next_usize(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next_u64() % n as u64) as usize
        }
    }

    fn next_byte(&mut self) -> u8 {
        (self.next_u64() >> 56) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_layout_matches_observed_facts() {
        let h = canonical_header();
        assert_eq!(h.len(), HEADER_LEN);
        assert_eq!(&h[0..4], &MAGIC);
        assert_eq!(&h[0x68..0x78], &TEMPLATE);
        assert_eq!(
            u32::from_le_bytes([h[0x64], h[0x65], h[0x66], h[0x67]]),
            LANDSCAPE
        );
    }

    #[test]
    fn nested_size_is_8_per_level() {
        assert_eq!(nested_records(3).len(), 24);
        assert_eq!(nested_records(0).len(), 0);
    }

    #[test]
    fn truncation_sweep_is_bounded() {
        let cfg = CampaignConfig {
            seed: 7,
            iters: 50,
            huge_tail_bytes: 1024,
            truncation_step: 3,
            depth_sweep_max: 16,
        };
        let rep = run_campaign(&cfg);
        assert_eq!(rep.panics, 0);
        assert!(rep.inputs > 100);
    }
}
