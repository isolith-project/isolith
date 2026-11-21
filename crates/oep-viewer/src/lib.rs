//! P6-M5-T02: preview viewer (desktop app shell) for `oep-geo` models.
//!
//! This crate is the interactive surface of the P6 renderer seam (see
//! `workstreams/geometry-core/viewer/renderer-backend-wgpu-adr.md`, "Clean interface"): callers build a
//! [`Scene`] from a real [`oep_geo::Document`], drive an orbit [`Camera`],
//! pick geometry with screen-space rays ([`pick`]), slice the view with
//! world-space [`clip`] planes, and ask a [`render`] target for pixels. The
//! reference implementation here is a deterministic CPU software renderer (the
//! ADR's documented escape hatch, section 9); the WGSL/wgpu backend plugs in
//! behind the same seam without touching callers.
//!
//! What "shows an oep-geo model" means at P6-M1: bodies carry only an
//! axis-aligned bounding box (the B-Rep/geometry kernel slots arrive at
//! P6-M2/P6-M3), so the viewer renders each body as its bounding-box proxy
//! placed by the instance transform chain. Selection returns the full
//! identity: instance chain + part id + body id.
//!
//! All math is `oep_geo::math` (vendored f64), all scalars `f64`. The crate
//! is std-only and deterministic, so the acceptance contract runs on the
//! GPU-less CI container and still proves camera, picking, clipping and
//! rendering on the committed golden model.
//!
//! Since UI-A5 the crate also hosts the viewport seam: [`backend::ViewportBackend`]
//! is what the UI panel drives, and [`backend::CpuBackend`] is its
//! deterministic CPU implementation over the same render path.
#![deny(missing_docs)]

pub mod backend;
pub mod camera;
pub mod clip;
pub mod overlay;
pub mod pick;
pub mod render;
pub mod scene;
/// Theme tokens for the accessibility/theme pass (P7-M5-T02).
pub mod theme;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P6-VIEWER";

pub use backend::{CpuBackend, ViewportBackend};
pub use camera::Camera;
pub use clip::{ClipPlane, Side};
pub use oep_geo::math::Vec3;
pub use overlay::{
    axis_arms, draw_label_chip, draw_text, fill_rect, grid_segments, nice_spacing, text_width,
};
pub use pick::{pick, pick_ray, Ray, Selection};
pub use render::{render, render_present, PresentFrame, PresentOptions, RgbaImage};
pub use scene::{build_scene, Aabb, MeshData, Primitive, Scene};
