//! P9-M5-T02 -- the P9 GA gate.
//!
//! The capstone acceptance of the P9 P2P compute lane: run a **100-job
//! render/convert wave across 3+ real nodes**, zero manual intervention,
//! every output **hash-verified in the P8 vault**.
//!
//! This crate wires the *real* wave together -- the same way the P8-M6-T03
//! gate wired the real working-set lock / revision state / audit log into the
//! served PDM contract, this gate wires the real lane into one runnable wave:
//!
//! - the **real scheduler** ([`oep_lane`]) owns order / priorities / leases /
//!   acks / GPU placement across the node pool;
//! - the **real convert workers** ([`oep_worker`]: `cgr2gltf`, `step2mesh`)
//!   and the **real render worker** ([`oep_render_worker`]) do the work;
//! - the **real P8 vault** ([`oep_pdm`]) is the shared input currency and the
//!   output verification target -- every artifact lands back in the vault and
//!   the vault's own `FileRef.blob_sha256` must equal the artifact sha-256
//!   (the P9-M5-T02 GA gate contract the convert lane already encodes).
//!
//! The wave runs *on* the real fleet: the gate driver (binary `oep-ga-gate`)
//! runs on the coordinator host, dispatches through the real scheduler, and
//! hands each dispatched job to its placement node by invoking the uniform
//! node agent (binary `oep-ga-node`) on that node -- locally for the
//! coordinator, over SSH for the remote GB10 fleet members. Every node agent
//! resolves its input from the SAME shared vault, runs the real worker
//! function, stores the output back to the vault, and hash-verifies it.
//!
//! ## Honesty boundaries
//!
//! - The wave claims nothing it does not do: each job's row records which
//!   physical node executed it, which input ref it consumed, and the
//!   artifact + vault content addresses; the gate then re-opens **every**
//!   output from the vault and re-hashes to confirm (independent of the
//!   worker's own check).
//! - A node that cannot be reached or a vault that cannot serve is a
//!   classified SKIP (tolerant doctrine), never a fabricated PASS.
//! - The render worker today uses the deterministic CPU reference backend
//!   (the P6-M5-T01 ADR escape hatch), so render jobs are tagged
//!   GPU-*preferred* -- they take a GPU node when one is free and fall back to
//!   a CPU node cleanly; they are never announced as GPU-*required*.
#![deny(missing_docs)]

/// Render-to-vault worker function (the render half of the wave): mirrors the
/// convert lane's `run_job` contract so render outputs are P9-M5-T02-verified.
pub mod render;

/// Node fleet model (real worker nodes + placement resources).
pub mod fleet;

/// Wave manifest + seed (the 100 render/convert jobs + corpus).
pub mod manifest;

/// The GA gate report (self-consistent JSON + Markdown wave proof).
pub mod report;
