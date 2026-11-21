//! The viewport render seam (editor parity program, wave A, card UI-A5).
//!
//! [`ViewportBackend`] is the trait the UI viewport panel drives: resize the
//! render target, read and write the orbit camera, and ask for a stable hash
//! of the current rendered frame. [`CpuBackend`] implements the seam with the
//! crate's existing deterministic software renderer ([`crate::render`]),
//! unchanged, so every committed golden stays byte-valid. Wave D swaps in a
//! GPU implementation behind the same trait without touching the panel.
//!
//! Determinism contract: for identical `(scene, camera, size, present
//! options)` the backend produces byte-identical [`RgbaImage`] frames, so
//! `frame_hash()` is a reproducible oracle for "the pixels did not change".

use std::time::{Duration, Instant};

use crate::camera::Camera;
use crate::clip::ClipPlane;
use crate::render::{render_present, PresentOptions, RgbaImage};
use crate::scene::{build_scene, Scene};
use oep_geo::{Body, BodyKind, BoundingBox, Document, Part};

/// Stable, dependency-free FNV-1a 64 hash of a rendered frame.
///
/// Hashes the width and height blocks first, then the RGBA byte stream, in
/// that order, so the hash is a complete fingerprint of the pixels. This is the
/// seam's determinism oracle: identical frames hash identically, and any pixel
/// change (size, camera, scene, options) changes the hash.
pub fn frame_hash(img: &RgbaImage) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    let mut feed = |bytes: &[u8]| {
        for &byte in bytes {
            h ^= u64::from(byte);
            h = h.wrapping_mul(PRIME);
        }
    };
    feed(&img.width.to_be_bytes());
    feed(&img.height.to_be_bytes());
    feed(&img.data);
    h
}

/// The viewport seam.
///
/// A backend owns a render target size, a camera, and the last rendered frame.
/// The UI panel talks only to this trait, so Wave D can substitute a GPU
/// backend behind the same surface: navigation changes the camera, the panel
/// asks for the frame to draw, and `frame_hash()` stays the determinism oracle
/// across backends.
pub trait ViewportBackend {
    /// Resize the render target (in pixels); re-rasterizes with the new size.
    fn resize(&mut self, width: u32, height: u32);

    /// Current orbit camera.
    fn camera(&self) -> Camera;

    /// Replace the camera; re-rasterizes with the new view.
    fn set_camera(&mut self, camera: Camera);

    /// The last rendered frame (draw this into the panel).
    fn frame(&self) -> &RgbaImage;

    /// Stable hash of the last rendered frame.
    fn frame_hash(&self) -> u64;

    /// Wall time of the last rasterize, for the status bar frame-time field.
    fn frame_time_secs(&self) -> f64;
}

/// Deterministic CPU viewport backend.
///
/// Wraps the existing [`render_present`] path byte-for-byte: it rasterizes the
/// same scene/camera/size/options the renderer already handled, so it never
/// changes the pixels a committed golden recorded.
#[derive(Clone, Debug)]
pub struct CpuBackend {
    scene: Scene,
    camera: Camera,
    clips: Vec<ClipPlane>,
    width: u32,
    height: u32,
    present: PresentOptions,
    frame: RgbaImage,
    hash: u64,
    frame_time: Duration,
}

impl CpuBackend {
    /// Wrap `scene` with a camera framed to its bounds (or `Camera::default`
    /// for an empty scene) and rasterize once at the renderer's default size.
    pub fn new(scene: Scene) -> Self {
        let camera = if let Some(bounds) = &scene.bounds {
            Camera::framed(bounds)
        } else {
            Camera::default()
        };
        let mut backend = Self {
            scene,
            camera,
            clips: Vec::new(),
            width: crate::render::DEFAULT_WIDTH,
            height: crate::render::DEFAULT_HEIGHT,
            present: PresentOptions {
                grid: true,
                axes: true,
                labels: Vec::new(),
            },
            frame: RgbaImage {
                width: 0,
                height: 0,
                data: Vec::new(),
            },
            hash: 0,
            frame_time: Duration::ZERO,
        };
        backend.rasterize();
        backend
    }

    /// The committed demo scene: the canonical 80x40x10 solid from the
    /// editor-sketch-extrude demo, built exactly as the real viewer owns it.
    pub fn demo_scene() -> Scene {
        let mut doc = Document::new("mm");
        doc.push_part(Part::new("p1", "part-one").with_body("b1"));
        doc.push_body(
            Body::new("b1", "body-one", BodyKind::Solid)
                .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [80.0, 40.0, 10.0])),
        );
        build_scene(&doc)
    }

    /// A backend already showing the committed demo scene.
    pub fn demo() -> Self {
        Self::new(Self::demo_scene())
    }

    fn rasterize(&mut self) {
        let start = Instant::now();
        let present = render_present(
            &self.scene,
            &self.camera,
            &self.clips,
            self.width,
            self.height,
            &self.present,
        );
        self.frame_time = start.elapsed();
        self.frame = present.img;
        self.hash = frame_hash(&self.frame);
    }
}

impl Default for CpuBackend {
    fn default() -> Self {
        Self::demo()
    }
}

impl ViewportBackend for CpuBackend {
    fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.rasterize();
    }

    fn camera(&self) -> Camera {
        self.camera
    }

    fn set_camera(&mut self, camera: Camera) {
        self.camera = camera;
        self.rasterize();
    }

    fn frame(&self) -> &RgbaImage {
        &self.frame
    }

    fn frame_hash(&self) -> u64 {
        self.hash
    }

    fn frame_time_secs(&self) -> f64 {
        self.frame_time.as_secs_f64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two identical backends hash identically, and the demo scene differs
    /// from the empty scene (so the hash actually carries content).
    #[test]
    fn demo_render_hashes_deterministically() {
        let a = CpuBackend::demo();
        let b = CpuBackend::demo();
        assert_eq!(a.frame_hash(), b.frame_hash());
        assert_eq!(a.camera(), b.camera());

        let empty = CpuBackend::new(Scene::default());
        assert_ne!(a.frame_hash(), empty.frame_hash());
    }

    /// The seam hands back byte-identical pixels to a direct render call: the
    /// trait added no transformation to the committed render path.
    #[test]
    fn seam_does_not_alter_output() {
        let backend = CpuBackend::demo();
        let (width, height) = (backend.width, backend.height);
        let direct = render_present(
            &backend.scene,
            &backend.camera,
            &backend.clips,
            width,
            height,
            &backend.present,
        );
        assert_eq!(backend.frame(), &direct.img);
        assert_eq!(backend.frame_hash(), frame_hash(&direct.img));
    }

    /// Orbit changes yaw/pitch by exactly the numeric deltas, and re-renders
    /// (the frame hash must move with the camera).
    #[test]
    fn orbit_changes_camera_numerically_and_frame_hash() {
        let mut backend = CpuBackend::demo();
        let before_hash = backend.frame_hash();
        let before = backend.camera();

        let mut cam = before;
        cam.orbit(0.4, -0.15);
        backend.set_camera(cam);

        let after = backend.camera();
        assert!((after.yaw - (before.yaw + 0.4)).abs() < 1e-9);
        assert!((after.pitch - (before.pitch - 0.15)).abs() < 1e-9);
        assert_ne!(after, before);
        assert_ne!(backend.frame_hash(), before_hash);
    }

    /// Zoom changes distance multiplicatively (respecting the camera's own
    /// `near*2` clamp, so a mild factor keeps the exact ratio) and re-renders.
    #[test]
    fn zoom_changes_distance_numerically_and_frame_hash() {
        let mut backend = CpuBackend::demo();
        let before_hash = backend.frame_hash();
        let before = backend.camera();

        let mut cam = before;
        cam.zoom(0.95);
        backend.set_camera(cam);

        let after = backend.camera();
        assert!((after.distance - before.distance * 0.95).abs() < 1e-9);
        assert_ne!(after.distance, before.distance);
        assert_ne!(backend.frame_hash(), before_hash);
    }

    /// Pan moves the orbit target by the requested world deltas and re-renders.
    #[test]
    fn pan_changes_target_numerically_and_frame_hash() {
        let mut backend = CpuBackend::demo();
        let before_hash = backend.frame_hash();
        let before = backend.camera();

        let target_before = before.target;
        let mut cam = before;
        cam.pan(5.0, 10.0);
        backend.set_camera(cam);

        let after = backend.camera();
        assert_ne!(after.target, target_before);
        assert_ne!(backend.frame_hash(), before_hash);
    }

    /// Resize changes the target size and therefore the hash.
    #[test]
    fn resize_changes_size_and_hash() {
        let mut backend = CpuBackend::demo();
        let before_hash = backend.frame_hash();
        backend.resize(320, 200);
        assert_eq!((backend.frame().width, backend.frame().height), (320, 200));
        assert_ne!(backend.frame_hash(), before_hash);
    }

    /// The camera stored by `set_camera` is exactly what `camera()` returns.
    #[test]
    fn camera_round_trips() {
        let mut backend = CpuBackend::demo();
        let mut cam = backend.camera();
        cam.orbit(0.1, 0.2);
        cam.zoom(0.9);
        backend.set_camera(cam);
        assert_eq!(backend.camera(), cam);
    }

    /// frame_time_secs() records a non-negative rasterize duration.
    #[test]
    fn frame_time_is_non_negative() {
        let backend = CpuBackend::demo();
        assert!(backend.frame_time_secs() >= 0.0);
    }
}
