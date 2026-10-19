//! P2-M1-T05 fuzz round 1: deterministic campaign over the CGR header
//! parser, zero panics on corpus.
//!
//! The cargo-fuzz (libFuzzer) harness lives under `crates/oep-cgr/fuzz/`
//! (target `fuzz_targets/header_parse.rs`) and needs a NIGHTLY toolchain to
//! build. CI and commits run on the pinned STABLE channel
//! (`rust-toolchain.toml`), so the enforced zero-panic contract is expressed
//! here as a deterministic, self-contained campaign: generators + a seeded
//! in-process loop + a [`CampaignReport`]. LibFuzzer on nightly then
//! continues from the same seed corpus on every push — the two legs share
//! the exact same typed-error, never-panic contract.
//!
//! Subject under fuzz: [`crate::header::parse_header`] (and its full typed
//! error set `TooShort` / `BadMagic` / `NonZeroPreamble` /
//! `TemplateMismatch`). By construction it is slice-based, allocation-free
//! past the small header struct, and returns a typed error for every
//! observed corruption class — the campaign proves it holds.
//!
//! Clean-room: every synthesized input is derived from the PUBLIC observed
//! CGR header facts (magic `01 00 00 00`, 0x60-byte zero preamble, landscape
//! u32 at 0x64, 16-byte invariant TEMPLATE at 0x68..0x78) recorded in
//! `docs/re-notes/oep-cgr-header-parse.md`; real CGR binaries are never
//! created or committed. Real material is only *read* (never stored)
//! through [`sample_dirs`] / [`run_campaign_on_samples`].

use crate::header::{parse_header, CgrError, HEADER_LEN, MAGIC, TEMPLATE};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Observed per-sample "landscape" u32 at header offset 0x64 for the
/// R19-era cfysa set (LE == 0x10CD3600). Semantics `[uncertain]`; held as
/// the canonical sample's landscape value.
const LANDSCAPE: u32 = 0x10CD_3600;

/// Build the canonical 0x78-byte CGR header spine from observed-public facts
/// only (no proprietary bytes). Used as the seed for the mutation,
/// truncation, and corrupt-class inputs.
pub fn canonical_header() -> Vec<u8> {
    let mut b = vec![0u8; HEADER_LEN];
    b[0..4].copy_from_slice(&MAGIC);
    b[0x64..0x68].copy_from_slice(&LANDSCAPE.to_le_bytes());
    b[0x68..0x78].copy_from_slice(&TEMPLATE);
    b
}

/// Build a deterministic `len`-byte synthetic CGR-ish "sample": the
/// canonical header plus a deterministic pattern payload (the parser keeps
/// the payload unparsed; the tail is fuzz realism for truncation/mutation).
pub fn canonical_sample(len: usize) -> Vec<u8> {
    let mut b = canonical_header();
    let need = len.saturating_sub(b.len());
    for i in 0..need {
        b.push(((i * 37) ^ 0x2C) as u8);
    }
    b
}

/// Campaign configuration. `Default` is sized for CI speed; the standalone
/// runner uses heavier values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CampaignConfig {
    /// PRNG seed; a fixed seed yields a byte-for-byte identical report.
    pub seed: u64,
    /// Number of mutation (seeded byte-flip) iterations.
    pub iters: u64,
    /// Step used when sweeping truncation points through an input.
    pub truncation_step: usize,
    /// Largest size exercised by the arbitrary-bytes sweep (0..=max).
    pub arbitrary_max: usize,
}

impl Default for CampaignConfig {
    fn default() -> Self {
        Self {
            seed: 0x4B_45_45_4C_52_31_5F_05,
            iters: 5_000,
            truncation_step: 7,
            arbitrary_max: 4096,
        }
    }
}

/// Aggregate result of one campaign run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CampaignReport {
    /// Total inputs parsed (deterministic legs + real material).
    pub inputs: u64,
    /// Inputs on which the parser PANICKED. The contract is exactly 0.
    pub panics: u64,
    /// Description of the first panic (class + input length), if any.
    pub first_panic: Option<String>,
    /// Outcome-kind histogram (`ok` / `too_short` / `bad_magic` /
    /// `nonzero_preamble` / `template_mismatch`) -> count.
    pub outcomes: BTreeMap<&'static str, u64>,
    /// Input counts per generator class
    /// (`canonical/truncated/mutation/tamper/arbitrary/real`).
    pub class_counts: BTreeMap<&'static str, u64>,
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

/// Run the full deterministic round-1 campaign (all legs) under `cfg`.
///
/// Returns a [`CampaignReport`]; the caller decides pass/fail
/// (`panics == 0` is the invariant). A fixed seed reproduces an identical
/// report.
pub fn run_campaign(cfg: &CampaignConfig) -> CampaignReport {
    let mut report = CampaignReport::default();
    let mut rng = XorShift64::new(cfg.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));

    // canonical baselines
    run_one(&mut report, "canonical", &canonical_header());
    run_one(&mut report, "canonical", &canonical_sample(1024));
    run_one(&mut report, "canonical", &canonical_sample(65536));

    // explicit corruption-class generators (deterministic coverage of the
    // full typed error set — not left to PRNG luck)
    let mut bad_magic = canonical_header();
    bad_magic[0] ^= 0xFF;
    run_one(&mut report, "tamper", &bad_magic);
    let mut bad_preamble = canonical_header();
    bad_preamble[0x40] = 1;
    run_one(&mut report, "tamper", &bad_preamble);
    let mut bad_template = canonical_header();
    bad_template[0x68] ^= 1;
    run_one(&mut report, "tamper", &bad_template);
    run_one(&mut report, "tamper", &canonical_header()[..1]); // too_short

    // truncated-tail leg (sweep + sharp tail window)
    let step = cfg.truncation_step.max(1);
    for base in [canonical_header(), canonical_sample(8192)] {
        let full = base.len();
        for k in (0..=full).step_by(step) {
            run_one(&mut report, "truncated", &base[..k]);
        }
        let start = full.saturating_sub(64);
        for k in start..=full {
            run_one(&mut report, "truncated", &base[..k]);
        }
    }

    // arbitrary-bytes sweep: fully unconstrained content across every size
    // up to `arbitrary_max`
    for n in 0..=cfg.arbitrary_max {
        let bytes: Vec<u8> = (0..n).map(|i| ((i * 31) ^ n) as u8).collect();
        run_one(&mut report, "arbitrary", &bytes);
    }

    // mutation leg: seeded byte mutation of real-adjacent canonical bytes
    let m1 = canonical_sample(2048);
    let m2 = canonical_header();
    for _ in 0..cfg.iters {
        let base = if rng.next_u64() & 1 == 0 { &m1 } else { &m2 };
        let mut bytes = base.clone();
        mutate(&mut bytes, &mut rng);
        run_one(&mut report, "mutation", &bytes);
    }

    report
}

/// Run the deterministic campaign AND parse the header of every real `.cgr`
/// reachable in `dirs`, reading at most [`REAL_FILE_READ_CAP`] bytes per
/// file. Real bytes are never stored or committed; only header classification
/// counts.
pub fn run_campaign_on_samples(cfg: &CampaignConfig, dirs: &[PathBuf]) -> CampaignReport {
    let mut report = run_campaign(cfg);
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
            match read_capped(&path, REAL_FILE_READ_CAP) {
                Ok(bytes) => run_one(&mut report, "real", &bytes),
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
/// stand-in bounded while the header (0x78 bytes) is what the parser reads.
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

/// Parse one input, updating `report`. The parse is wrapped in
/// `catch_unwind`: a panic on any untrusted input IS a campaign failure and
/// is counted in `panics` (the contract requires 0).
fn run_one(report: &mut CampaignReport, class: &'static str, bytes: &[u8]) {
    report.inputs += 1;
    *report.class_counts.entry(class).or_insert(0) += 1;
    let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_header(bytes)));
    match attempt {
        Err(_) => {
            report.panics += 1;
            if report.first_panic.is_none() {
                report.first_panic = Some(format!("class={class} len={}", bytes.len()));
            }
        }
        Ok(Ok(_)) => *report.outcomes.entry("ok").or_insert(0) += 1,
        Ok(Err(CgrError::TooShort { .. })) => {
            *report.outcomes.entry("too_short").or_insert(0) += 1;
        }
        Ok(Err(CgrError::BadMagic { .. })) => {
            *report.outcomes.entry("bad_magic").or_insert(0) += 1;
        }
        Ok(Err(CgrError::NonZeroPreamble { .. })) => {
            *report.outcomes.entry("nonzero_preamble").or_insert(0) += 1;
        }
        Ok(Err(CgrError::TemplateMismatch { .. })) => {
            *report.outcomes.entry("template_mismatch").or_insert(0) += 1;
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
