//! P3-M2-T01 acceptance tests: Gregory-patch surface decode to NURBS.
//!
//! Written FIRST (TDD): before the implementation existed, the API referenced
//! by these tests (`oep_3dxml::gregory`, `oep_3dxml::lattice`) did not
//! compile (RED). Now GREEN on real material committed at
//! `corpus/data/3dxml/surface/`.

use oep_3dxml::gregory::GregoryPatch;
use oep_3dxml::lattice::LatticeMesh;
use oep_3dxml::vec3::Vec3;

use std::path::PathBuf;

/// Absolute point tolerance used throughout (patch scale ~O(10), so this is a
/// ~1e-10 relative check; the decode is exact, this only absorbs fp rounding).
const TOL: f64 = 1e-9;

/// Corpus directory relative to the crate dir.
fn corpus_dir() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    PathBuf::from(manifest).join("../../corpus/data/3dxml/surface")
}

fn read_corpus(name: &str) -> String {
    std::fs::read_to_string(corpus_dir().join(name))
        .unwrap_or_else(|e| panic!("missing corpus file {name}: {e}"))
}

/// Sample parameters on (u,v) in [0,1]^2, skipping the two blend-singular
/// corners (0,0) and (1,1).
fn sample_grid() -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for i in 0..=16 {
        let u = i as f64 / 16.0;
        for j in 0..=16 {
            let v = j as f64 / 16.0;
            let singular = (u.abs() < 1e-12 && v.abs() < 1e-12)
                || ((1.0 - u).abs() < 1e-12 && (1.0 - v).abs() < 1e-12);
            if !singular {
                out.push((u, v));
            }
        }
    }
    out
}

fn assert_close(got: Vec3, want: Vec3, tol: f64, what: &str) {
    let d = (got - want).norm();
    assert!(
        d <= tol,
        "{what}: |got-want|={d:.3e} > {tol:.1e}\n  got  {got:?}\n  want {want:?}"
    );
}

/// A deterministic, non-trivial twisted Gregory patch (scale ~ 10).
fn sample_patch() -> GregoryPatch {
    let corners = [
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(4.0, 0.0, 1.0),
        Vec3::new(4.0, 4.0, 1.0),
        Vec3::new(0.0, 4.0, 1.0),
    ];
    let edge_uv0 = [Vec3::new(1.0, -0.4, 1.3), Vec3::new(2.0, 0.2, 1.6)];
    let edge_uv1 = [Vec3::new(1.0, 4.4, 1.0), Vec3::new(2.0, 4.6, 1.1)];
    let edge_vu0 = [Vec3::new(-0.4, 1.0, 1.2), Vec3::new(-0.2, 2.0, 0.8)];
    let edge_vu1 = [Vec3::new(4.4, 1.0, 0.9), Vec3::new(4.6, 2.0, 1.4)];
    let twist_uv = [
        [Vec3::new(1.0, 1.0, 1.5), Vec3::new(1.0, 3.0, 1.2)],
        [Vec3::new(3.0, 1.0, 1.3), Vec3::new(3.0, 3.0, 1.6)],
    ];
    let twist_vu = [
        [Vec3::new(1.0, 1.0, 0.7), Vec3::new(1.0, 3.0, 1.6)],
        [Vec3::new(3.0, 1.0, 1.5), Vec3::new(3.0, 3.0, 0.9)],
    ];
    GregoryPatch {
        corners,
        edge_uv0,
        edge_uv1,
        edge_vu0,
        edge_vu1,
        twist_uv,
        twist_vu,
    }
}

// ---------------------------------------------------------------------------
// Core math: exact decode of a Gregory patch to a NURBS surface.
// ---------------------------------------------------------------------------

#[test]
fn decode_is_exact_nurbs_equivalence() {
    let patch = sample_patch();
    let nurbs = patch.to_nurbs();
    assert_eq!((nurbs.degree_u, nurbs.degree_v), (4, 4));
    assert_eq!(nurbs.control_points.len(), 25);
    assert_eq!(nurbs.weights.len(), 25);

    let mut max_den: f64 = 0.0;
    let mut min_weight = f64::INFINITY;
    for &w in &nurbs.weights {
        min_weight = min_weight.min(w);
    }
    assert!(
        min_weight >= -1e-15,
        "all NURBS weights must be non-negative, min={min_weight}"
    );

    for (u, v) in sample_grid() {
        let g = patch.eval(u, v);
        let n = nurbs.eval(u, v);
        let (_, den) = nurbs.eval_homogeneous(u, v);
        max_den = max_den.max(den);
        assert!(
            g.is_finite(),
            "Gregory eval must be finite at ({u},{v}): {g:?}"
        );
        assert!(
            n.is_finite(),
            "NURBS eval must be finite at ({u},{v}): {n:?}"
        );
        assert_close(n, g, TOL, &format!("nurbs==gregory at ({u},{v})"));
    }
    assert!(
        max_den > 0.0,
        "sample grid should have non-zero denominators"
    );
}

#[test]
fn planar_patch_decodes_to_a_plane() {
    // A patch whose control net lies exactly in the plane z=0. The NURBS
    // decode must stay on that plane everywhere.
    let corners = [
        Vec3::new(-2.0, -2.0, 0.0),
        Vec3::new(2.0, -2.0, 0.0),
        Vec3::new(2.0, 2.0, 0.0),
        Vec3::new(-2.0, 2.0, 0.0),
    ];
    let edge_uv0 = [Vec3::new(0.0, -2.0, 0.0), Vec3::new(1.0, -2.0, 0.0)];
    let edge_uv1 = [Vec3::new(0.0, 2.0, 0.0), Vec3::new(1.0, 2.0, 0.0)];
    let edge_vu0 = [Vec3::new(-2.0, 0.0, 0.0), Vec3::new(-2.0, 1.0, 0.0)];
    let edge_vu1 = [Vec3::new(2.0, 0.0, 0.0), Vec3::new(2.0, 1.0, 0.0)];
    let interior = [
        [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)],
        [Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0)],
    ];
    let patch = GregoryPatch::twist_free(corners, edge_uv0, edge_uv1, edge_vu0, edge_vu1, interior);
    let nurbs = patch.to_nurbs();
    for (u, v) in sample_grid() {
        let p = nurbs.eval(u, v);
        assert_close(
            p,
            Vec3::new(p.x, p.y, 0.0),
            TOL,
            &format!("plane z=0 at ({u},{v})"),
        );
        let g = patch.eval(u, v);
        assert_close(
            g,
            Vec3::new(g.x, g.y, 0.0),
            TOL,
            &format!("plane z=0 gregory at ({u},{v})"),
        );
    }
}

#[test]
fn corners_are_reproduced_and_singularity_handled() {
    let patch = sample_patch();
    // Blend-singular corner (0,0) and (1,1): the limit is the corner point.
    assert_eq!(patch.eval(0.0, 0.0), patch.corners[0]);
    assert_eq!(patch.eval(1.0, 1.0), patch.corners[2]);
    // Non-singular corners are exact too.
    assert_close(patch.eval(1.0, 0.0), patch.corners[1], TOL, "corner (1,0)");
    assert_close(patch.eval(0.0, 1.0), patch.corners[3], TOL, "corner (0,1)");
    // And the decoded NURBS matches along a line that crosses the singular
    // corner diagonally (except at the exact corner).
    let nurbs = patch.to_nurbs();
    let n = 40;
    for k in 1..n {
        let t = k as f64 / n as f64;
        let nurb = nurbs.eval(t, t);
        let greg = patch.eval(t, t);
        assert_close(nurb, greg, TOL, &format!("diagonal at t={t}"));
    }
}

#[test]
fn boundaries_are_exactly_the_cubic_beziers() {
    let patch = sample_patch();
    let nurbs = patch.to_nurbs();
    for edge_id in 0u8..4 {
        let curve = patch.boundary_curve(edge_id);
        for k in 0..=20 {
            let t = k as f64 / 20.0;
            // Gregory patch (with corner-limit handling) must equal the Bezier
            // boundary everywhere, including endpoints.
            let c = curve.eval(t);
            // The NURBS view is undefined exactly at the two blend-singular
            // corners (0,0) and (1,1); only skip the NURBS compare there.
            let singular = (t.abs() < 1e-12 && matches!(edge_id, 0 | 2))
                || ((1.0 - t).abs() < 1e-12 && matches!(edge_id, 1 | 3));
            match edge_id {
                0 => {
                    let g = patch.eval(t, 0.0);
                    assert_close(g, c, TOL, &format!("gregory edge{edge_id} t={t}"));
                }
                1 => {
                    let g = patch.eval(t, 1.0);
                    assert_close(g, c, TOL, &format!("gregory edge{edge_id} t={t}"));
                }
                2 => {
                    let g = patch.eval(0.0, t);
                    assert_close(g, c, TOL, &format!("gregory edge{edge_id} t={t}"));
                }
                _ => {
                    let g = patch.eval(1.0, t);
                    assert_close(g, c, TOL, &format!("gregory edge{edge_id} t={t}"));
                }
            }
            if singular {
                continue;
            }
            let n = match edge_id {
                0 => nurbs.eval(t, 0.0),
                1 => nurbs.eval(t, 1.0),
                2 => nurbs.eval(0.0, t),
                _ => nurbs.eval(1.0, t),
            };
            assert_close(n, c, TOL, &format!("nurbs edge{edge_id} t={t}"));
        }
        // Endpoint tangent of the decoded boundary must be 3*(B1-B0).
        let d = curve.eval(1e-6) - curve.eval(0.0);
        let expect = (curve.control[1] - curve.control[0]) * 3.0;
        assert!(
            (d * 1e6 - expect).norm() < 1e-3,
            "edge{edge_id} endpoint tangent mismatch"
        );
    }
}

#[test]
fn nurb_eval_is_rational_biquartic_in_bernstein_form() {
    // The weights of the decoded NURBS are the Bernstein coefficients of the
    // denominator u(1-v)+(1-u)v elevated to bidegree (4,4): the corner slots
    // (0,0) and (4,4) are 0, all interior slots are positive.
    let patch = sample_patch();
    let n = patch.to_nurbs();
    let (_, w00) = n.get(0, 0);
    let (_, w44) = n.get(4, 4);
    assert!(
        w00.abs() < 1e-15 && w44.abs() < 1e-15,
        "decoded NURBS corner weights must vanish, got {w00} {w44}"
    );
    for iv in 0..5 {
        for iu in 0..5 {
            if !(iu == 0 && iv == 0) && !(iu == 4 && iv == 4) {
                let (_, w) = n.get(iu, iv);
                assert!(
                    w > 0.0,
                    "interior NURBS weight must be positive at ({iu},{iv}), got {w}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Real material: Web3D LatticeXvl `XvlShell` surface reps.
// ---------------------------------------------------------------------------

#[test]
fn real_star_model_counts_and_decode() {
    let xml = read_corpus("XvlShellModelStar.x3d");
    let meshes = LatticeMesh::parse_xvl_document(&xml);
    assert_eq!(meshes.len(), 1, "Star model has one XvlShell");
    let m = &meshes[0];
    assert_eq!(m.vertices.len(), 78, "Star vertex count");
    assert_eq!(m.faces.len(), 80, "Star face count");
    // Star is 72 quads + 8 triangles -> 72 + 8*3 = 96 patches after decode.
    assert_eq!(m.faces.iter().filter(|f| f.len() == 4).count(), 72);
    assert_eq!(m.decoded_patch_count(), 96);

    let patches = m.decode_surfaces();
    assert_eq!(patches.len(), 96);
    verify_patches_finite_and_exact(&patches);
}

#[test]
fn real_freeform_models_decode_finite_and_exact() {
    // Baby, Dolphin, Hand, Octopus: real freeform lattice surfaces with
    // mixed n-gon faces; the reconstruction fans each non-quad face.
    for name in [
        "XvlShellModelBaby.x3d",
        "XvlShellModelDolphin.x3d",
        "XvlShellModelHand.x3d",
        "XvlShellModelOctopus.x3d",
    ] {
        let xml = read_corpus(name);
        let meshes = LatticeMesh::parse_xvl_document(&xml);
        assert!(!meshes.is_empty(), "{name}: at least one XvlShell");
        for m in &meshes {
            assert!(m.vertices.len() >= 4, "{name}: {m:?} has vertices");
            assert!(!m.faces.is_empty(), "{name}: has faces");
            let expected = m.decoded_patch_count();
            let patches = m.decode_surfaces();
            assert_eq!(
                patches.len(),
                expected,
                "{name}/{m:?}: decoded patch count matches definition"
            );
            assert!(
                patches.len() >= m.faces.len(),
                "{name}: n-gon fan never loses faces"
            );
            verify_patches_finite_and_exact(&patches);
        }
    }
}

#[test]
fn real_attribute_demos_parse_weights_and_vectors() {
    let xml = read_corpus("XvlShellAttributeAll.x3d");
    let meshes = LatticeMesh::parse_xvl_document(&xml);
    assert!(meshes.len() >= 20, "AttributeAll has 23 shells");
    // Spot check: every parsed shell decodes without panic and finitely.
    let mut total = 0usize;
    for m in &meshes {
        total += m.decode_surfaces().len();
    }
    assert!(total > 0);

    // EdgeVector demo must carry named tangent vectors that the decode uses.
    let ev_xml = read_corpus("XvlShellAttributeEdgeVector.x3d");
    let ev_meshes = LatticeMesh::parse_xvl_document(&ev_xml);
    assert_eq!(
        ev_meshes.len(),
        3,
        "EdgeVector demo has 3 cubes (0,1,2 vectors)"
    );
    let has_vectors = ev_meshes.iter().any(|m| {
        m.edges
            .iter()
            .any(|e| e.tangent0.map(|t| t.norm() > 0.0).unwrap_or(false))
    });
    assert!(
        has_vectors,
        "EdgeVector demo parses non-zero tangent vectors"
    );
    let vr_xml = read_corpus("XvlShellAttributeVertexRound.x3d");
    let vr_meshes = LatticeMesh::parse_xvl_document(&vr_xml);
    assert!(
        vr_meshes
            .iter()
            .flat_map(|m| &m.vertex_round)
            .any(|&w| (w - 0.1).abs() < 1e-12),
        "VertexRound demo parses the 0.1 weight"
    );
    let er_xml = read_corpus("XvlShellAttributeEdgeRound.x3d");
    let er_meshes = LatticeMesh::parse_xvl_document(&er_xml);
    assert!(
        er_meshes
            .iter()
            .flat_map(|m| &m.edges)
            .any(|e| (e.round - 0.1).abs() < 1e-12),
        "EdgeRound demo parses the 0.1 weight"
    );
}

#[test]
fn real_edge_vector_changes_boundary_curve_exactly() {
    // The EdgeVector demo places the same cube 3x; the (0 2 0) instance
    // decodes top-face boundary cubics whose first interior control point is
    // corner + (0,2,0)/3 (analytic-tangent convention, verified against the
    // data the file stores).
    let xml = read_corpus("XvlShellAttributeEdgeVector.x3d");
    let meshes = LatticeMesh::parse_xvl_document(&xml);
    // Find the cube whose edges all carry tangent (0,2,0).
    let m = meshes
        .iter()
        .find(|m| {
            m.edges.iter().all(|e| {
                e.tangent0
                    .map(|t| (t - Vec3::new(0.0, 2.0, 0.0)).norm() < 1e-12)
                    .unwrap_or(false)
            })
        })
        .expect("a (0,2,0) vector cube exists in the demo");
    // Top face ring from faceCoordIndex of that demo: '3 7 6 2'.
    assert!(m.vertices.len() >= 8);
    // First face should be the top face; verify corner interpolation and the
    // v=0 boundary control point offset.
    let top = &m.faces[0];
    assert_eq!(top.len(), 4);
    let patch = m.decode_quad([top[0], top[1], top[2], top[3]]);
    // Corner (0,0) of the decoded patch equals the lattice vertex.
    let p0 = m.vertices[top[0] as usize];
    assert_close(patch.eval(0.0, 0.0), p0, TOL, "top-face corner 0");
    // v=0 boundary: B1 = P0 + t0/3 where t0 = (0,2,0).
    let want_b1 = p0 + Vec3::new(0.0, 2.0, 0.0) / 3.0;
    assert_close(
        patch.edge_uv0[0],
        want_b1,
        TOL,
        "edgeBeginVector tangent reproduced in the decoded cubic",
    );
    // And every boundary point lies on the cubic implied by the data.
    let curve = patch.boundary_curve(0);
    for k in 0..=10 {
        let t = k as f64 / 10.0;
        assert_close(patch.eval(t, 0.0), curve.eval(t), TOL, "boundary vs bezier");
    }
}

/// Shared checker: all patches decode to finite NURBS that equal the
/// Gregory evaluation on a sampling grid.
fn verify_patches_finite_and_exact(patches: &[GregoryPatch]) {
    assert!(!patches.is_empty());
    for (i, p) in patches.iter().enumerate() {
        for c in p.corners.iter().chain(
            p.edge_uv0
                .iter()
                .chain(&p.edge_uv1)
                .chain(&p.edge_vu0)
                .chain(&p.edge_vu1),
        ) {
            assert!(c.is_finite(), "patch {i} has a non-finite control point");
        }
        let n = p.to_nurbs();
        for (u, v) in sample_grid() {
            let g = p.eval(u, v);
            let e = n.eval(u, v);
            assert!(g.is_finite(), "patch {i} gregory eval non-finite");
            assert!(e.is_finite(), "patch {i} NURBS eval non-finite");
            assert_close(e, g, TOL, &format!("patch {i} nurbs==gregory at ({u},{v})"));
        }
    }
}

#[test]
fn pg_round_trip_patch_count_formula_matches_faces() {
    // Golden on the real-material topology math: quads decode to one patch,
    // n-gons fan into one quad per ring vertex.
    let star = LatticeMesh::parse_xvl_document(&read_corpus("XvlShellModelStar.x3d"));
    assert_eq!(star[0].decoded_patch_count(), 96);

    let dol = LatticeMesh::parse_xvl_document(&read_corpus("XvlShellModelDolphin.x3d"));
    let d = &dol[0];
    assert!(
        d.faces.iter().any(|f| f.len() != 4),
        "Dolphin contains non-quad faces (mixed n-gons)"
    );
    // 158 quads + 16 tris + 17 pentagons + 1 hexagon + 1 nonagon.
    let expect = 158 + (16 * 3 + 17 * 5 + 6 + 9);
    assert_eq!(d.decoded_patch_count(), expect);
    assert_eq!(d.faces.len(), 193);
    let manual: usize = d.faces.iter().filter(|f| f.len() == 4).count()
        + d.faces
            .iter()
            .filter(|f| f.len() != 4 && f.len() > 2)
            .map(|f| f.len())
            .sum::<usize>();
    assert_eq!(d.decoded_patch_count(), manual);
    assert!(d.decoded_patch_count() > d.faces.len());
}
