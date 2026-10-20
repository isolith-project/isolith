//! STEP / ISO 10303-21 B-Rep passthrough writer for the internal mesh
//! (P2-M5-T02).
//!
//! Converts an indexed triangle mesh (`&Mesh` from [`crate::mesh`]) into a
//! STEP exchange file whose B-Rep is a **faceted shell**: one
//! `SHELL_BASED_SURFACE_MODEL` containing, per connected component, a
//! `CLOSED_SHELL` (watertight manifold) or `OPEN_SHELL` (open or non-manifold)
//! of planar `FACE_SURFACE`s — every triangle becomes a `PLANE` bounded by a
//! `POLY_LOOP` of its three `CARTESIAN_POINT`s.
//!
//! ## Honesty contract (this is the point of the card)
//!
//! The output is **marked as tessellated and is NOT claimed to be an exact
//! analytic B-Rep**:
//!
//! * `FILE_DESCRIPTION` in the PART 21 header states the file is a
//!   tessellated mesh passthrough and not an exact analytic B-Rep;
//! * the `PRODUCT` name/description and the `SHAPE_REPRESENTATION`
//!   identifier carry the `tessellated` marker;
//! * the geometry is planar facets only — no `ADVANCED_FACE`, no
//!   `B_SPLINE`/NURBS, no analytic surfaces;
//! * the CLI prints a runtime warning to the same effect
//!   (`crates/oep-cgr/src/bin/oep-cgr2step.rs`).
//!
//! Exact geometry conversion is a later workstream (P4 STEP AP242, P6
//! geometry core); this module only transports the tessellation.
//!
//! ## Numerics / determinism
//!
//! Coordinates are the source tessellation values emitted verbatim (f32
//! shortest round-trip, trailing `.0` appended so every real has a decimal
//! point per PART 21 grammar). Units are assumed millimetre (SI) — CGR
//! carries no unit metadata; see `docs/cgr-to-step.md`. Output is
//! byte-deterministic for a given mesh + [`StepOpts`].
//!
//! ## Failures
//!
//! Typed [`StepError`]s and never a panic. Degenerate (zero-area) triangles
//! are skipped with a counted warning (tolerant export of real tessellated
//! data); a mesh with no usable face is [`StepError::EmptyMesh`].

use std::collections::HashMap;
use std::fmt;
use std::io::Write;

use crate::mesh::Mesh;

/// A triangle whose span is below this length is treated as zero-area
/// (degenerate) and skipped as a facet.
const DEGENERATE_EPS: f32 = 1e-12;

/// Errors produced during STEP export.
#[derive(Debug)]
pub enum StepError {
    /// The mesh has no usable (non-degenerate) face to write.
    EmptyMesh,
    /// A triangle index referenced a vertex that does not exist.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The number of vertices in the mesh.
        len: usize,
    },
    /// A vertex used by a triangle is NaN or infinite.
    NonFiniteVertex(usize),
    /// Underlying I/O failure while writing.
    Io(std::io::Error),
}

/// Variant-level equality (I/O error payloads are ignored).
impl PartialEq for StepError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (StepError::EmptyMesh, StepError::EmptyMesh) => true,
            (
                StepError::IndexOutOfRange { index: a, len: la },
                StepError::IndexOutOfRange { index: b, len: lb },
            ) => a == b && la == lb,
            (StepError::NonFiniteVertex(a), StepError::NonFiniteVertex(b)) => a == b,
            (StepError::Io(_), StepError::Io(_)) => true,
            _ => false,
        }
    }
}

impl fmt::Display for StepError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StepError::EmptyMesh => {
                write!(f, "mesh has no usable (non-degenerate) triangles to write")
            }
            StepError::IndexOutOfRange { index, len } => {
                write!(f, "triangle index {index} out of range (vertices: {len})")
            }
            StepError::NonFiniteVertex(i) => write!(f, "vertex {i} is non-finite"),
            StepError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for StepError {}

impl From<std::io::Error> for StepError {
    fn from(e: std::io::Error) -> Self {
        StepError::Io(e)
    }
}

/// Writer options that appear in the STEP header and product entities.
#[derive(Debug, Clone)]
pub struct StepOpts<'a> {
    /// Product name; placed in the `PRODUCT` id/name fields.
    pub product_name: &'a str,
    /// Source file name for the `FILE_NAME` field (the file being written).
    pub source_name: &'a str,
    /// RFC3339 timestamp for the `FILE_NAME` field; empty string = omit.
    pub timestamp: &'a str,
}

/// Counted export summary returned by [`write_step`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepStats {
    /// Planar faces written (one per usable, non-degenerate triangle).
    pub faces: usize,
    /// Triangles skipped because they were degenerate (zero area).
    pub degenerate_skipped: usize,
    /// Number of shells (connected components of the face set).
    pub shells: usize,
    /// Of [`Self::shells`], how many are watertight `CLOSED_SHELL`s.
    pub closed_shells: usize,
    /// Of [`Self::shells`], how many are open/non-manifold `OPEN_SHELL`s.
    pub open_shells: usize,
}

/// Shortest-round-trip PART 21 REAL literal: `{}` then ensure a decimal
/// point exists so the token parses as a real, not an integer.
fn fmt_real(v: f32) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.to_ascii_lowercase().contains('e') {
        s
    } else {
        format!("{s}.")
    }
}

/// Cross product `u x v`.
fn cross(u: [f32; 3], v: [f32; 3]) -> [f32; 3] {
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

/// Unit vector along `v`, or `None` when `v` is ~zero.
fn unit(v: [f32; 3]) -> Option<[f32; 3]> {
    let mag = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if mag.is_finite() && mag > DEGENERATE_EPS {
        Some([v[0] / mag, v[1] / mag, v[2] / mag])
    } else {
        None
    }
}

/// Sequential PART 21 entity-id emitter. Every `emit_*` returns the assigned
/// `#N` for downstream references; ids are assigned strictly in emission
/// order, so output is deterministic for a fixed input.
struct Emitter<'w, W: Write> {
    w: &'w mut W,
    next: u32,
}

impl<W: Write> Emitter<'_, W> {
    fn new(w: &mut W) -> Emitter<'_, W> {
        Emitter { w, next: 1 }
    }

    fn id(&mut self) -> u32 {
        let id = self.next;
        self.next += 1;
        id
    }

    fn line(&mut self, id: u32, body: &str) -> std::io::Result<()> {
        writeln!(self.w, "#{id}={body};")
    }

    fn cartesian_point(&mut self, p: [f32; 3]) -> std::io::Result<u32> {
        let id = self.id();
        self.line(
            id,
            &format!(
                "CARTESIAN_POINT('',({},{},{}))",
                fmt_real(p[0]),
                fmt_real(p[1]),
                fmt_real(p[2])
            ),
        )?;
        Ok(id)
    }

    fn direction(&mut self, d: [f32; 3]) -> std::io::Result<u32> {
        let id = self.id();
        self.line(
            id,
            &format!(
                "DIRECTION('',({},{},{}))",
                fmt_real(d[0]),
                fmt_real(d[1]),
                fmt_real(d[2])
            ),
        )?;
        Ok(id)
    }

    fn axis2_placement_3d(&mut self, location: u32, z: u32, x: u32) -> std::io::Result<u32> {
        let id = self.id();
        self.line(id, &format!("AXIS2_PLACEMENT_3D('',#{location},#{z},#{x})"))?;
        Ok(id)
    }

    fn plane(&mut self, placement: u32) -> std::io::Result<u32> {
        let id = self.id();
        self.line(id, &format!("PLANE('',#{placement})"))?;
        Ok(id)
    }

    fn poly_loop(&mut self, points: &[u32; 3]) -> std::io::Result<u32> {
        let id = self.id();
        self.line(
            id,
            &format!(
                "POLY_LOOP('',(#{},#{},#{}))",
                points[0], points[1], points[2]
            ),
        )?;
        Ok(id)
    }

    fn face_outer_bound(&mut self, loop_id: u32) -> std::io::Result<u32> {
        let id = self.id();
        self.line(id, &format!("FACE_OUTER_BOUND('',#{loop_id},.T.)"))?;
        Ok(id)
    }

    fn face_surface(&mut self, bound: u32, plane: u32) -> std::io::Result<u32> {
        let id = self.id();
        self.line(id, &format!("FACE_SURFACE('',(#{bound}),#{plane},.T.)"))?;
        Ok(id)
    }

    fn shell(&mut self, closed: bool, faces: &[u32]) -> std::io::Result<u32> {
        let id = self.id();
        let kind = if closed { "CLOSED_SHELL" } else { "OPEN_SHELL" };
        let list = faces
            .iter()
            .map(|f| format!("#{f}"))
            .collect::<Vec<_>>()
            .join(",");
        self.line(id, &format!("{kind}('',({list}))"))?;
        Ok(id)
    }

    fn units_and_context(&mut self) -> std::io::Result<u32> {
        let len_unit = self.id();
        self.line(
            len_unit,
            "(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))",
        )?;
        let ang_unit = self.id();
        self.line(
            ang_unit,
            "(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))",
        )?;
        let ang_solid = self.id();
        self.line(
            ang_solid,
            "(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())",
        )?;
        let ctx = self.id();
        self.line(
            ctx,
            &format!(
                "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#{len_unit},#{ang_unit},#{ang_solid}))REPRESENTATION_CONTEXT('',''))"
            ),
        )?;
        Ok(ctx)
    }
}

/// Resolve a triangle's three vertex positions, bounds-checked.
fn triangle_vertices(mesh: &Mesh, tri: &[u32; 3]) -> Result<[[f32; 3]; 3], StepError> {
    let verts = mesh.vertices();
    let mut out = [[0.0f32; 3]; 3];
    for (k, &idx) in tri.iter().enumerate() {
        let i = idx as usize;
        if i >= verts.len() {
            return Err(StepError::IndexOutOfRange {
                index: idx,
                len: verts.len(),
            });
        }
        if !verts[i].iter().all(|c| c.is_finite()) {
            return Err(StepError::NonFiniteVertex(i));
        }
        out[k] = verts[i];
    }
    Ok(out)
}

/// Write `mesh` as a faceted, tessellation-marked STEP B-Rep into `w`.
///
/// See the module docs for the honesty contract. Returns export [`StepStats`]
/// on success. Never panics (see the no-panic mini-fuzz in the acceptance
/// test).
pub fn write_step<W: Write>(
    mesh: &Mesh,
    opts: &StepOpts<'_>,
    w: &mut W,
) -> Result<StepStats, StepError> {
    let triangles = mesh.triangles();

    // Pass 1: classify triangles into usable (plane computable) vs degenerate.
    // Index/finite checks are hard errors (defensive; meshes produced by
    // `extract` are already structurally sound); degenerate faces are skipped,
    // counted, and reported — the tolerant-export path.
    struct TriMeta {
        tri_idx: usize,
        normal: [f32; 3],
        x_axis: [f32; 3],
    }
    let mut metas: Vec<TriMeta> = Vec::new();
    let mut degenerate_skipped = 0usize;
    for (idx, tri) in triangles.iter().enumerate() {
        let pts = triangle_vertices(mesh, tri)?;
        let u = [
            pts[1][0] - pts[0][0],
            pts[1][1] - pts[0][1],
            pts[1][2] - pts[0][2],
        ];
        let v = [
            pts[2][0] - pts[0][0],
            pts[2][1] - pts[0][1],
            pts[2][2] - pts[0][2],
        ];
        match (unit(cross(u, v)), unit(u)) {
            (Some(normal), Some(x_axis)) => {
                metas.push(TriMeta {
                    tri_idx: idx,
                    normal,
                    x_axis,
                });
            }
            _ => degenerate_skipped += 1,
        }
    }
    if metas.is_empty() {
        return Err(StepError::EmptyMesh);
    }

    // Undirected-edge -> usage count and -> incident triangles, both over the
    // *usable* face set, for shells and closedness.
    let mut edge_to_tris: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
    let mut tri_edges: Vec<[(u64, u64); 3]> = Vec::with_capacity(metas.len());
    for (meta_idx, m) in metas.iter().enumerate() {
        let tri = &triangles[m.tri_idx];
        let mut edges = [(0u64, 0u64); 3];
        for (k, &(a, b)) in [(tri[0], tri[1]), (tri[1], tri[2]), (tri[2], tri[0])]
            .iter()
            .enumerate()
        {
            let key = (a.min(b) as u64, a.max(b) as u64);
            edges[k] = key;
            edge_to_tris.entry(key).or_default().push(meta_idx);
        }
        tri_edges.push(edges);
    }

    // Components (BFS over shared edges).
    let n = metas.len();
    let mut comp_of = vec![usize::MAX; n];
    let mut components: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if comp_of[start] != usize::MAX {
            continue;
        }
        let cid = components.len();
        let mut stack = vec![start];
        comp_of[start] = cid;
        let mut members = Vec::new();
        while let Some(t) = stack.pop() {
            members.push(t);
            for e in &tri_edges[t] {
                for &nbr in edge_to_tris.get(e).into_iter().flatten() {
                    if comp_of[nbr] == usize::MAX {
                        comp_of[nbr] = cid;
                        stack.push(nbr);
                    }
                }
            }
        }
        components.push(members);
    }

    // --- HEADER ---
    let ts: String = if opts.timestamp.is_empty() {
        String::new()
    } else {
        opts.timestamp.replace('\'', "''")
    };
    writeln!(w, "ISO-10303-21;")?;
    writeln!(w, "HEADER;")?;
    writeln!(
        w,
        "FILE_DESCRIPTION(('oep-cgr2step: tessellated mesh passthrough. Faceted (planar) B-Rep derived from the source CGR tessellation. This is a TESSELLATED representation and is NOT an exact analytic B-Rep.'),'2;1');"
    )?;
    writeln!(
        w,
        "FILE_NAME('{}','{ts}',('Isolith OEP'),(''),'oep-cgr2step {}','Isolith OEP','');",
        opts.source_name.replace('\'', "''"),
        env!("CARGO_PKG_VERSION")
    )?;
    writeln!(
        w,
        "FILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));"
    )?;
    writeln!(w, "ENDSEC;")?;
    writeln!(w, "DATA;")?;

    let mut em = Emitter::new(w);

    // Product / definition cluster.
    let app_ctx = em.id();
    em.line(app_ctx, "APPLICATION_CONTEXT('geometric model')")?;
    let apd = em.id();
    em.line(
        apd,
        &format!(
            "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',1999,#{app_ctx})"
        ),
    )?;
    let prod_ctx = em.id();
    em.line(
        prod_ctx,
        &format!("PRODUCT_CONTEXT('',#{apd},'mechanical')"),
    )?;
    let product = em.id();
    let product_name = opts.product_name.replace('\'', "''");
    em.line(
        product,
        &format!(
            "PRODUCT('{product_name}','{product_name}','oep-cgr2step: tessellated faceted B-Rep passthrough. NOT an exact analytic B-Rep.',(#{prod_ctx}))"
        ),
    )?;
    let pdf = em.id();
    em.line(
        pdf,
        &format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"),
    )?;
    let pdc = em.id();
    em.line(
        pdc,
        &format!("PRODUCT_DEFINITION_CONTEXT('part definition',#{apd},'design')"),
    )?;
    let pd = em.id();
    em.line(
        pd,
        &format!("PRODUCT_DEFINITION('design','',#{pdf},#{pdc})"),
    )?;
    let pds = em.id();
    em.line(
        pds,
        &format!("PRODUCT_DEFINITION_SHAPE('','tessellated Mesh',#{pd})"),
    )?;

    // One planar face per usable triangle.
    let mut tri_face_id = vec![0u32; n];
    for (k, m) in metas.iter().enumerate() {
        let pts = triangle_vertices(mesh, &triangles[m.tri_idx])?;
        let p0 = em.cartesian_point(pts[0])?;
        let p1 = em.cartesian_point(pts[1])?;
        let p2 = em.cartesian_point(pts[2])?;
        let z = em.direction(m.normal)?;
        let x = em.direction(m.x_axis)?;
        let place = em.axis2_placement_3d(p0, z, x)?;
        let plane = em.plane(place)?;
        let ploop = em.poly_loop(&[p0, p1, p2])?;
        let fob = em.face_outer_bound(ploop)?;
        let face = em.face_surface(fob, plane)?;
        tri_face_id[k] = face;
    }

    // One shell per connected component.
    let mut shell_ids = Vec::new();
    let mut closed_count = 0usize;
    for members in &components {
        let closed = members.iter().all(|&m| {
            tri_edges[m]
                .iter()
                .all(|e| edge_to_tris.get(e).is_some_and(|v| v.len() == 2))
        });
        let face_list: Vec<u32> = members.iter().map(|&m| tri_face_id[m]).collect();
        let shell_id = em.shell(closed, &face_list)?;
        shell_ids.push(shell_id);
        if closed {
            closed_count += 1;
        }
    }
    let open_count = shell_ids.len() - closed_count;

    let shell_list = shell_ids
        .iter()
        .map(|s| format!("#{s}"))
        .collect::<Vec<_>>()
        .join(",");
    let sbsm = em.id();
    em.line(
        sbsm,
        &format!("SHELL_BASED_SURFACE_MODEL('oep-cgr tessellated mesh',({shell_list}))"),
    )?;

    // Units + context, then the shape representation referencing them.
    let ctx = em.units_and_context()?;
    let shape_rep = em.id();
    em.line(
        shape_rep,
        &format!(
            "SHAPE_REPRESENTATION('tessellated faceted B-Rep (NOT exact)','ShellBasedSurfaceModel',(#{sbsm}),#{ctx})"
        ),
    )?;
    let rel = em.id();
    em.line(
        rel,
        &format!("SHAPE_REPRESENTATION_RELATIONSHIP('','',#{pds},#{shape_rep})"),
    )?;

    writeln!(w, "ENDSEC;")?;
    writeln!(w, "END-ISO-10303-21;")?;

    Ok(StepStats {
        faces: n,
        degenerate_skipped,
        shells: shell_ids.len(),
        closed_shells: closed_count,
        open_shells: open_count,
    })
}
