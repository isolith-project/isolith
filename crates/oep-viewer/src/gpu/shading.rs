//! Studio shading for the GPU viewport (wave D, card UI-D2).
//!
//! This module is the single source of truth for how the GPU viewport turns a
//! scene into a *studio viewport* rather than a flat wireframe:
//!
//! - a fixed two-point light rig — one directional **key** light plus a weaker
//!   **fill** light — whose strengths and directions are baked into the WGSL
//!   shading pass ([`shading_wgsl`] builds the shader source from these exact
//!   Rust constants, so the file and the numbers can never drift);
//! - the feature-**edge list**: a mesh primitive draws its committed boundary
//!   wires (`Primitive.mesh.edges`), a box proxy draws its 12 box edges, and
//!   the [`edge_pass`] therefore draws EXACTLY the boundary edge list — the
//!   numeric proof counts drawn primitives against the fixture's edge count;
//! - **view modes** ([`ViewMode`]): Shaded (smooth normals + edges), Edges
//!   (the edge pass only), Flat (per-face normals), Smooth (per-vertex
//!   normals) — the last two differ only in normal-driven pixels by design;
//! - **transparency** ([`ShadingState::alpha`]): an opaque (1.0) to fully
//!   transparent (0.0) control carried on the vertex colour, applied by the
//!   shaded pipeline's alpha blending so an overlapping body shows through
//!   and its read-back alpha changes.
//!
//! The active state is process-global ([`set_state`] / [`state`]) so the oep-app
//! shading panel can drive the backend at runtime through the shared seam
//! without a trait method (the `ViewportBackend` trait stays untouched on this
//! card). Tests set it directly under a lock and re-render.
//!
//! ## Determinism
//!
//! Everything here is a pure function of the inputs (scene geometry, camera,
//! the state). The same state + scene + camera + size always produces the same
//! pixels on the same host, so the five snapshots' sha256 hashes are
//! reproducible (re-proven by the D1 determinism oracle on the committed tree).

use oep_geo::math::Vec3;

/// The four view modes the GPU viewport can render (selection at runtime).
///
/// `Shaded` is the studio default: smooth normals, key+fill lighting AND the
/// feature-edge overlay. `Edges` draws only the edge pass. `Flat` and `Smooth`
/// render the filled surfaces with per-face and per-vertex normals
/// respectively, so the "smooth vs flat" proof compares exactly these two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Studio look: smooth normals, key+fill lighting, feature edges on top.
    Shaded,
    /// The edge pass only — the boundary edge list, nothing else.
    Edges,
    /// Flat (per-face) normals, key+fill lighting, no edge overlay.
    Flat,
    /// Smooth (per-vertex) normals, key+fill lighting, no edge overlay.
    Smooth,
}

impl ViewMode {
    /// A short stable label (used by the shading panel and the oracle).
    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Shaded => "Shaded",
            ViewMode::Edges => "Edges",
            ViewMode::Flat => "Flat",
            ViewMode::Smooth => "Smooth",
        }
    }

    /// Every mode, for the panel and for tests that sweep all modes.
    pub const ALL: [ViewMode; 4] = [
        ViewMode::Shaded,
        ViewMode::Edges,
        ViewMode::Flat,
        ViewMode::Smooth,
    ];
}

/// The runtime-selectable shading state of the GPU viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadingState {
    /// Which rendering mode is active.
    pub mode: ViewMode,
    /// Transparency in `0..=1` (`1.0` = fully opaque). The 50% proof uses
    /// `0.5`.
    pub alpha: f32,
}

impl Default for ShadingState {
    fn default() -> Self {
        Self {
            mode: ViewMode::Shaded,
            alpha: 1.0,
        }
    }
}

// ---------------------------------------------------------------------------
// The light rig — the single source of truth for the WGSL shading pass
// ---------------------------------------------------------------------------

/// Direction of the key light (view/eye space, unit length not required; the
/// shader normalises). Sun-from-the-left-front: the classic studio key.
pub const KEY_DIR: [f32; 3] = [-0.45, 0.60, 0.65];
/// Direction of the fill light — dimmer, from the opposite low side, lifting
/// the shadows the key leaves.
pub const FILL_DIR: [f32; 3] = [0.55, -0.25, 0.60];
/// Intensity of the key light (fragment multiplies `max(dot(n, key), 0)`).
pub const KEY_STRENGTH: f32 = 0.85;
/// Intensity of the fill light.
pub const FILL_STRENGTH: f32 = 0.35;
/// Flat lower bound of the lighting (so shadow faces stay readable, never
/// pure black).
pub const AMBIENT: f32 = 0.18;

/// The WGSL source for the shaded (triangle) pass, assembled from the light
/// rig constants above so the shader can never drift from them.
///
/// The static `shaders/shading.wgsl` file carries the structs and the
/// `__KEY_DIR_X__`-style placeholders; this function substitutes the values,
/// returning a ready-to-compile WGSL module string.
pub fn shading_wgsl() -> String {
    include_str!("shaders/shading.wgsl")
        .replace("__KEY_DIR_X__", &fmt(KEY_DIR[0]))
        .replace("__KEY_DIR_Y__", &fmt(KEY_DIR[1]))
        .replace("__KEY_DIR_Z__", &fmt(KEY_DIR[2]))
        .replace("__FILL_DIR_X__", &fmt(FILL_DIR[0]))
        .replace("__FILL_DIR_Y__", &fmt(FILL_DIR[1]))
        .replace("__FILL_DIR_Z__", &fmt(FILL_DIR[2]))
        .replace("__KEY_STRENGTH__", &fmt(KEY_STRENGTH))
        .replace("__FILL_STRENGTH__", &fmt(FILL_STRENGTH))
        .replace("__AMBIENT__", &fmt(AMBIENT))
}

/// Deterministic float rendering (three decimals) for the WGSL substitution.
fn fmt(v: f32) -> String {
    format!("{v:.3}")
}

// ---------------------------------------------------------------------------
// The feature edge list
// ---------------------------------------------------------------------------

/// The 12 boundary edges of a box proxy, as index pairs into `world_corners`
/// (the `Aabb::corners()` order: 0..3 = the `z = min` ring, 4..7 = the
/// `z = max` ring, verticals join the same x/y corner across rings).
///
/// A box proxy has no `Primitive.mesh`, so this is the proxy's boundary edge
/// list — the same 12 edges a tessellated box exposes.
pub const BOX_EDGES: [[usize; 2]; 12] = [
    // z = min ring
    [0, 1],
    [1, 2],
    [2, 3],
    [3, 0],
    // z = max ring
    [4, 5],
    [5, 6],
    [6, 7],
    [7, 4],
    // verticals
    [0, 4],
    [1, 5],
    [2, 6],
    [3, 7],
];

/// The boundary edge list of `prim`, as index pairs.
///
/// A mesh primitive returns its committed `mesh.edges` (the boundary wires);
/// a box proxy returns the 12 [`BOX_EDGES`]. The edge pass draws exactly this
/// list, so its primitive count equals the fixture's boundary edge count.
pub fn boundary_edges(prim: &crate::scene::Primitive) -> &[[usize; 2]] {
    match &prim.mesh {
        Some(mesh) => &mesh.edges,
        None => &BOX_EDGES,
    }
}

// ---------------------------------------------------------------------------
// Per-vertex normals: flat (per-face) vs smooth (per-vertex averaged)
// ---------------------------------------------------------------------------

/// World-space normal of a polygon (Newell's method, same convention as the
/// CPU renderer). Public so the GPU backend can derive flat box-proxy face
/// normals directly.
pub fn newell(poly: &[Vec3]) -> Vec3 {
    let mut n = Vec3::zero();
    for i in 0..poly.len() {
        let cur = poly[i];
        let nxt = poly[(i + 1) % poly.len()];
        n = n.add(&Vec3::new(
            (cur.y - nxt.y) * (cur.z + nxt.z),
            (cur.z - nxt.z) * (cur.x + nxt.x),
            (cur.x - nxt.x) * (cur.y + nxt.y),
        ));
    }
    n.normalize().unwrap_or(Vec3::new(0.0, 0.0, 1.0))
}

/// The flat per-vertex normals for one polygon: every vertex of `poly` gets
/// the same face normal (constant across the face → flat shading).
pub fn flat_normals(poly: &[Vec3]) -> Vec<Vec3> {
    let n = newell(poly);
    vec![n; poly.len()]
}

/// The flat (per-face) normal of one triangle as its three vertex normals —
/// the flat-shading normal for a triangle pass. `tri` must hold at least 3
/// vertices.
pub fn flat_triangle_normals(tri: &[Vec3]) -> [Vec3; 3] {
    let n = if tri.len() >= 3 {
        newell(&tri[..3])
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    };
    [n, n, n]
}

/// Smooth per-vertex normals for a (closed) indexed triangle soup.
///
/// `positions` is indexed by `triangles`; each vertex's normal is the
/// area-weighted average of the normals of every triangle that uses it,
/// renormalised — the classic Gouraud normal. Because it is computed from the
/// *shared-indexed* triangle list, adjacent faces blend across the shared
/// edge, which is what makes smooth shading differ from flat on the same
/// geometry. For a box proxy the caller passes the 8 `world_corners` as
/// `positions` and fanned triangle indices over them.
pub fn smooth_normals(positions: &[Vec3], triangles: &[[u32; 3]]) -> Vec<Vec3> {
    let max_idx = positions.len();
    let mut acc = vec![Vec3::zero(); max_idx];
    for t in triangles {
        let (Some(a), Some(b), Some(c)) = (
            positions.get(t[0] as usize),
            positions.get(t[1] as usize),
            positions.get(t[2] as usize),
        ) else {
            continue;
        };
        // Unnormalised cross product: length = 2 * triangle area.
        let n = b.sub(a).cross(&c.sub(a));
        for i in [t[0] as usize, t[1] as usize, t[2] as usize] {
            if let Some(slot) = acc.get_mut(i) {
                *slot = slot.add(&n);
            }
        }
    }
    acc.into_iter()
        .map(|v| {
            if v.norm_squared() <= 1e-24 {
                Vec3::new(0.0, 0.0, 1.0)
            } else {
                v.normalize().unwrap_or(Vec3::new(0.0, 0.0, 1.0))
            }
        })
        .collect()
}

/// The face normal of a box proxy quad, in world space.
pub fn box_face_normal(corners: &[Vec3; 8], quad: &[usize; 4]) -> Vec3 {
    let poly: Vec<Vec3> = quad.iter().map(|&i| corners[i]).collect();
    newell(&poly)
}

/// The fanned triangle index list of an AABB-corners box: each of the six
/// quads (in `Aabb::corners()` order — bottom ring 0..3, top ring 4..7, then
/// the four vertical side faces) split into two index triangles. This is the
/// shape `smooth_normals` needs for a box proxy, and the same fan the GPU
/// backend rasterizes per quad.
pub fn box_fan_triangles() -> [[u32; 3]; 12] {
    let quads: [[usize; 4]; 6] = [
        [0, 1, 2, 3], // bottom
        [4, 5, 6, 7], // top
        [0, 1, 5, 4], // +y side
        [3, 2, 6, 7], // -y side
        [0, 3, 7, 4], // -x side
        [1, 2, 6, 5], // +x side
    ];
    let mut out = [[0u32; 3]; 12];
    for (i, q) in quads.iter().enumerate() {
        out[i * 2] = [q[0] as u32, q[1] as u32, q[2] as u32];
        out[i * 2 + 1] = [q[0] as u32, q[2] as u32, q[3] as u32];
    }
    out
}

// ---------------------------------------------------------------------------
// Process-global active state (panel + tests drive it; the backend reads it)
// ---------------------------------------------------------------------------

use std::sync::{Mutex, OnceLock};

static STATE: OnceLock<Mutex<ShadingState>> = OnceLock::new();

fn state_lock() -> &'static Mutex<ShadingState> {
    STATE.get_or_init(|| Mutex::new(ShadingState::default()))
}

/// Read the current process-wide shading state.
pub fn state() -> ShadingState {
    *state_lock().lock().expect("shading state lock")
}

/// Replace the process-wide shading state, returning the previous value.
pub fn set_state(next: ShadingState) -> ShadingState {
    let mut g = state_lock().lock().expect("shading state lock");
    let prev = *g;
    *g = next;
    prev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shading_wgsl_embeds_the_exact_light_rig() {
        let src = shading_wgsl();
        // The substituted numbers must appear verbatim (single source of truth).
        assert!(src.contains("-0.450"), "key x in: {src}");
        assert!(src.contains("0.600"), "key y in: {src}");
        assert!(src.contains("0.850"), "key strength in: {src}");
        assert!(src.contains("0.180"), "ambient in: {src}");
        // No placeholder may survive substitution.
        assert!(!src.contains("__KEY_DIR_X__"));
        assert!(!src.contains("__AMBIENT__"));
    }

    #[test]
    fn smooth_vs_flat_normals_differ_on_a_box() {
        use crate::scene::Aabb;
        use oep_geo::math::Vec3;
        let bb = Aabb::new(Vec3::zero(), Vec3::new(1.0, 1.0, 1.0));
        let corners = bb.corners();
        // The six quad faces of the unit box.
        let quads: [[usize; 4]; 6] = [
            [0, 1, 2, 3],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [3, 2, 6, 7],
            [0, 3, 7, 4],
            [1, 2, 6, 5],
        ];
        let flat: Vec<Vec3> = quads
            .iter()
            .flat_map(|q| flat_normals(&q.iter().map(|&i| corners[i]).collect::<Vec<_>>()))
            .collect();
        // Truth table: 8 corners x 6 faces, then 8 corners of each of 12 tris.
        let tris: Vec<[u32; 3]> = quads
            .iter()
            .flat_map(|q| {
                [
                    [q[0] as u32, q[1] as u32, q[2] as u32],
                    [q[0] as u32, q[2] as u32, q[3] as u32],
                ]
            })
            .collect();
        let smooth = smooth_normals(corners.as_ref(), &tris);
        // A corner of the cube (index 0) has three incident faces; its smooth
        // normal points at (1,1,1)/sqrt(3), never equal to any single face
        // normal. All 8 corners therefore differ between flat and smooth.
        let flat_corner = flat[0];
        let smooth_corner = smooth[0];
        assert_ne!(flat_corner.x, smooth_corner.x);
        assert_ne!(flat_corner.y, smooth_corner.y);
        assert_ne!(flat_corner.z, smooth_corner.z);
        // And every smooth normal is a unit vector.
        for n in &smooth {
            let l = (n.x * n.x + n.y * n.y + n.z * n.z).sqrt();
            assert!((l - 1.0).abs() < 1e-9, "smooth normal not unit: {n:?}");
        }
    }
}
