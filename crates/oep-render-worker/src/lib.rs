//! P9-M3-T01: headless oep-geo render worker.
//!
//! The P9 compute lane's first worker function (master plan P9-M3-T01):
//! a headless renderer that turns a real `oep-geo` model into an image
//! artifact, built on the P6-M5-T01 renderer decision (prereq t_cfbc5f03,
//! ADR `P6-M5/renderer-backend-wgpu-adr.md`). Per the ADR's clean seam, the
//! worker consumes the [`oep_viewer`] scene/render surface and never sees
//! wgpu/WGSL; today that surface is the deterministic CPU reference
//! rasterizer (the ADR's documented escape hatch), so this worker renders
//! real pixels on any host, GPU or not, which is exactly what a regression-
//! safe CI gate needs.
//!
//! The worker function is [`worker::render_document`]: build the scene from a
//! loaded [`oep_geo::Document`], frame an orbit camera, render at the
//! requested resolution and fidelity (supersample factor), and return an
//! [`RgbaImage`](oep_viewer::render::RgbaImage) ready to write as PNG. The
//! CLI binary (`crates/oep-render-worker/src/main.rs`) wraps it as the
//! `oep-render-worker` command, parameterized exactly as the P9-M1 job spec
//! declares for the `render` executor: `--resolution WxH` and `--fidelity`.
//!
//! Registration: the worker registers with the P9 scheduler through the
//! [`registry`] module, which resolves the P9-M1 executor vocabulary
//! (`kind = "render"`, `function = "oep-geo-render"`) and is the seam the
//! scheduler calls to map a job's `executor.function` onto this worker.
//! `--describe` prints the same registration as JSON, so scheduler discovery
//! is CI-checkable without linking the (in-flight) scheduler crate.
#![deny(missing_docs)]

pub mod options;
pub mod registry;
pub mod worker;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-WORKER";

pub use options::{Fidelity, RenderOptions};
pub use registry::{
    get, registry, WorkerRegistration, EXECUTOR_KIND_RENDER, FIDELITY_LEVELS,
    WORKER_FUNCTION_RENDER,
};
pub use worker::{box_downsample, render_document, RenderResult};
