//! STEP / ISO 10303-21 faceted B-Rep passthrough export (P6-M4-T01).
//!
//! Writes the internal tessellation mesh as an AP214 (automotive_design)
//! PART 21 file containing a `SHELL_BASED_SURFACE_MODEL`: one planar
//! `FACE_SURFACE` per triangle (a `PLANE` bounded by a `POLY_LOOP` of its
//! three `CARTESIAN_POINT`s), grouped into a `CLOSED_SHELL` / `OPEN_SHELL`
//! per connected component.
//!
//! ## Honesty contract (this is the point of the card)
//!
//! The result is a **tessellated (faceted) passthrough and is NOT claimed as
//! an exact analytic B-Rep**. The header and product description state this
//! explicitly, per the P2-M5-T02 / P4-lane contract. The STEP *export* lane is
//! owned by P4 (`crates/oep-step`); this P6 exporter provides the faceted
//! passthrough now and is byte-locked to the independent encoder
//! [`crates/oep-geo/tools/gen_golden_box_step.py`].
//!
//! ## Numerics / determinism
//!
//! Coordinates are emitted in shortest-round-trip `f32` form widened to `f64`
//! with a decimal point forced ("1." / "0." / "-0.7071067690849304") so every
//! token is a PART 21 REAL -- identical to the independent encoder. Connected
//! components are found by BFS over shared undirected edges; shell member
//! order follows the same deterministic traversal, so output is byte-for-byte
//! stable for a given mesh + options.
//!
//! ## Failures
//!
//! Typed errors (`StepError == ExportError`) and never panics: index bounds,
//! non-finite vertices, degenerate triangles (zero-area), empty mesh, I/O.

use std::io::Write;

use super::base::{cross, unit, ExportError, DEGENERATE_EPS};

/// Options for [`write_step`] -- carried so the golden byte contract is
/// explicit and caller-overridable.
#[derive(Debug, Clone)]
pub struct StepOpts {
    /// `PRODUCT` name (golden: `"box"`).
    pub product_name: String,
    /// File name recorded in `FILE_NAME` (golden: `"box-reference.step"`).
    pub file_name: String,
    /// `oep-geo` version string recorded in `FILE_NAME`.
    pub version: String,
}

impl Default for StepOpts {
    fn default() -> Self {
        Self {
            product_name: "box".to_string(),
            file_name: "box-reference.step".to_string(),
            version: "0.1.0".to_string(),
        }
    }
}

/// Result counts from a [`write_step`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepStats {
    /// Number of usable (non-degenerate) faces written.
    pub faces: usize,
    /// Number of shells written.
    pub shells: usize,
    /// Of [`Self::shells`], how many are watertight `CLOSED_SHELL`s.
    pub closed_shells: usize,
}

/// Format one `f32` as a PART 21 REAL: shortest round-trip via `f64`, forcing
/// a decimal point so the token is unambiguously a REAL.
fn fmt_real(v: f32) -> String {
    let f = v as f64;
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e16 {
        // Integral (including -0.0, which must NOT keep its sign on output:
        // the independent encoder emits "0." for it).
        format!("{}.", f as i64)
    } else {
        let s = format!("{f}");
        if s.contains('.') || s.contains('e') || s.contains('E') {
            s
        } else {
            format!("{s}.")
        }
    }
}

/// Emitter that hands out entity ids and tracks shell membership.
struct Emit {
    next_id: u32,
    out: String,
}

impl Emit {
    fn new() -> Self {
        Emit {
            next_id: 1,
            out: String::new(),
        }
    }
    fn id(&mut self) -> u32 {
        let i = self.next_id;
        self.next_id += 1;
        i
    }
    fn line(&mut self, id: u32, body: &str) {
        self.out.push_str(&format!("#{id}={body};\n"));
    }
}

/// A triangle that passed validation: its source index, unit normal and a
/// unit vector along its first edge (used for the plane's z/x axes).
struct Meta {
    tri_index: usize,
    normal: [f32; 3],
    x_axis: [f32; 3],
}

/// Compute `Meta` for every usable triangle (skipping degenerate ones).
fn usable_metas(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Result<Vec<Meta>, ExportError> {
    let mut out = Vec::with_capacity(triangles.len());
    for (ti, tri) in triangles.iter().enumerate() {
        let p = super::base::triangle_positions(positions, tri)?;
        let u = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
        let v = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
        let n = cross(u, v);
        let mag = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if mag <= DEGENERATE_EPS {
            continue; // degenerate -> tolerant skip, mirrors the encoder
        }
        let normal = [n[0] / mag, n[1] / mag, n[2] / mag];
        let Some(x_axis) = unit(u) else {
            continue;
        };
        out.push(Meta {
            tri_index: ti,
            normal,
            x_axis,
        });
    }
    if out.is_empty() {
        return Err(ExportError::EmptyMesh);
    }
    Ok(out)
}

/// Find connected components of the face graph (shared undirected edges),
/// deterministic DFS over the triangle list. Returns components as lists of
/// *meta indices* in traversal (first-seen pop) order, matching the encoder.
fn components(metas: &[Meta], triangles: &[[u32; 3]]) -> Vec<Vec<usize>> {
    let n = metas.len();
    // undirected edge -> meta indices sharing it (insertion order)
    let mut edge_to_metas: std::collections::HashMap<(u32, u32), Vec<usize>> =
        std::collections::HashMap::new();
    let mut tri_edges: Vec<Vec<(u32, u32)>> = Vec::with_capacity(n);
    for (mi, m) in metas.iter().enumerate() {
        let (a, b, c) = (
            triangles[m.tri_index][0],
            triangles[m.tri_index][1],
            triangles[m.tri_index][2],
        );
        let mut edges = Vec::with_capacity(3);
        for (u, v) in [(a, b), (b, c), (c, a)] {
            let key = if u < v { (u, v) } else { (v, u) };
            edges.push(key);
            edge_to_metas.entry(key).or_default().push(mi);
        }
        tri_edges.push(edges);
    }

    let mut comp_of = vec![usize::MAX; n];
    let mut components = Vec::new();
    for start in 0..n {
        if comp_of[start] != usize::MAX {
            continue;
        }
        let cid = components.len();
        comp_of[start] = cid;
        let mut stack = vec![start];
        let mut members = Vec::new();
        while let Some(t) = stack.pop() {
            members.push(t);
            for e in &tri_edges[t] {
                if let Some(nbs) = edge_to_metas.get(e) {
                    for &nb in nbs {
                        if comp_of[nb] == usize::MAX {
                            comp_of[nb] = cid;
                            stack.push(nb);
                        }
                    }
                }
            }
        }
        components.push(members);
    }
    components
}

/// Write the mesh as an AP214 PART 21 faceted B-Rep (honest tessellated
/// passthrough). Returns [`StepStats`] on success.
pub fn write_step<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    opts: &StepOpts,
    w: &mut W,
) -> Result<StepStats, ExportError> {
    let metas = usable_metas(positions, triangles)?;
    let comps = components(&metas, triangles);

    let mut em = Emit::new();
    let emit = |em: &mut Emit, body: String| {
        let id = em.id();
        em.line(id, &body);
        id
    };

    // ---- Header -----------------------------------------------------------
    em.out.push_str("ISO-10303-21;\nHEADER;\n");
    em.out.push_str(
        "FILE_DESCRIPTION(('oep-geo: tessellated mesh passthrough. Faceted \
         (planar) B-Rep derived from source tessellation. This is a \
         TESSELLATED representation and is NOT an exact analytic B-Rep.'),\
         '2;1');\n",
    );
    em.out.push_str(&format!(
        "FILE_NAME('{}','',('Isolith OEP'),(''),'oep-geo {}','Isolith OEP','');\n",
        opts.file_name, opts.version
    ));
    em.out
        .push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\n");
    em.out.push_str("ENDSEC;\nDATA;\n");

    // ---- Product cluster (deterministic id order, as in the encoder) ------
    let app_ctx = emit(
        &mut em,
        "APPLICATION_CONTEXT('geometric model')".to_string(),
    );
    let apd = emit(
        &mut em,
        format!(
            "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',1999,#{app_ctx})"
        ),
    );
    let prod_ctx = emit(&mut em, format!("PRODUCT_CONTEXT('',#{apd},'mechanical')"));
    let product = emit(
        &mut em,
        format!(
            "PRODUCT('{}','{}','oep-geo: tessellated faceted B-Rep passthrough. NOT an exact analytic B-Rep.',(#{prod_ctx}))",
            opts.product_name, opts.product_name
        ),
    );
    let pdf2 = emit(
        &mut em,
        format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"),
    );
    let pdc = emit(
        &mut em,
        format!("PRODUCT_DEFINITION_CONTEXT('part definition',#{apd},'design')"),
    );
    let pd = emit(
        &mut em,
        format!("PRODUCT_DEFINITION('design','',#{pdf2},#{pdc})"),
    );
    let pds = emit(
        &mut em,
        format!("PRODUCT_DEFINITION_SHAPE('','tessellated mesh',#{pd})"),
    );

    // ---- One planar face per usable triangle ------------------------------
    let mut tri_face_id = vec![0u32; metas.len()];
    for (mi, m) in metas.iter().enumerate() {
        let (a, b, c) = (
            triangles[m.tri_index][0] as usize,
            triangles[m.tri_index][1] as usize,
            triangles[m.tri_index][2] as usize,
        );
        let pa = positions[a];
        let pb = positions[b];
        let pc = positions[c];
        let cp0 = emit(
            &mut em,
            format!(
                "CARTESIAN_POINT('',({},{},{}))",
                fmt_real(pa[0]),
                fmt_real(pa[1]),
                fmt_real(pa[2])
            ),
        );
        let cp1 = emit(
            &mut em,
            format!(
                "CARTESIAN_POINT('',({},{},{}))",
                fmt_real(pb[0]),
                fmt_real(pb[1]),
                fmt_real(pb[2])
            ),
        );
        let cp2 = emit(
            &mut em,
            format!(
                "CARTESIAN_POINT('',({},{},{}))",
                fmt_real(pc[0]),
                fmt_real(pc[1]),
                fmt_real(pc[2])
            ),
        );
        let zid = emit(
            &mut em,
            format!(
                "DIRECTION('',({},{},{}))",
                fmt_real(m.normal[0]),
                fmt_real(m.normal[1]),
                fmt_real(m.normal[2])
            ),
        );
        let xid = emit(
            &mut em,
            format!(
                "DIRECTION('',({},{},{}))",
                fmt_real(m.x_axis[0]),
                fmt_real(m.x_axis[1]),
                fmt_real(m.x_axis[2])
            ),
        );
        let place = emit(
            &mut em,
            format!("AXIS2_PLACEMENT_3D('',#{cp0},#{zid},#{xid})"),
        );
        let plane = emit(&mut em, format!("PLANE('',#{place})"));
        let ploop = emit(&mut em, format!("POLY_LOOP('',(#{cp0},#{cp1},#{cp2}))"));
        let fob = emit(&mut em, format!("FACE_OUTER_BOUND('',#{ploop},.T.)"));
        let face = emit(&mut em, format!("FACE_SURFACE('',(#{fob}),#{plane},.T.)"));
        tri_face_id[mi] = face;
    }

    // ---- One shell per component ------------------------------------------
    let mut shell_ids = Vec::with_capacity(comps.len());
    let mut closed_count = 0usize;
    for members in &comps {
        let mut percent_closed = true;
        let mut edge_counts: std::collections::HashMap<(u32, u32), usize> =
            std::collections::HashMap::new();
        for &mi in members {
            let m = &metas[mi];
            let (a, b, c) = (
                triangles[m.tri_index][0],
                triangles[m.tri_index][1],
                triangles[m.tri_index][2],
            );
            for (u, v) in [(a, b), (b, c), (c, a)] {
                let key = if u < v { (u, v) } else { (v, u) };
                *edge_counts.entry(key).or_insert(0) += 1;
            }
        }
        for &count in edge_counts.values() {
            if count != 2 {
                percent_closed = false;
                break;
            }
        }
        let kind = if percent_closed {
            "CLOSED_SHELL"
        } else {
            "OPEN_SHELL"
        };
        if percent_closed {
            closed_count += 1;
        }
        let face_list = members
            .iter()
            .map(|&mi| format!("#{}", tri_face_id[mi]))
            .collect::<Vec<_>>()
            .join(",");
        let shell = emit(&mut em, format!("{kind}('',({face_list}))"));
        shell_ids.push(shell);
    }

    // ---- Surface model + units + shape representation ----------------------
    let shell_list = shell_ids
        .iter()
        .map(|s| format!("#{s}"))
        .collect::<Vec<_>>()
        .join(",");
    let sbsm = emit(
        &mut em,
        format!("SHELL_BASED_SURFACE_MODEL('oep-geo tessellated mesh',({shell_list}))"),
    );
    let len_unit = emit(
        &mut em,
        "(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))".to_string(),
    );
    let ang_unit = emit(
        &mut em,
        "(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))".to_string(),
    );
    let ang_solid = emit(
        &mut em,
        "(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())".to_string(),
    );
    let ctx = emit(
        &mut em,
        format!(
            "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#{len_unit},#{ang_unit},#{ang_solid}))REPRESENTATION_CONTEXT('',''))"
        ),
    );
    let shape_rep = emit(
        &mut em,
        format!(
            "SHAPE_REPRESENTATION('tessellated faceted B-Rep (NOT exact)','ShellBasedSurfaceModel',(#{sbsm}),#{ctx})"
        ),
    );
    let _rel = emit(
        &mut em,
        format!("SHAPE_REPRESENTATION_RELATIONSHIP('','',#{pds},#{shape_rep})"),
    );

    em.out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    w.write_all(em.out.as_bytes())?;

    Ok(StepStats {
        faces: metas.len(),
        shells: comps.len(),
        closed_shells: closed_count,
    })
}
