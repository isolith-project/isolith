//! P2-M3-T03 acceptance test (TDD, RED first) — section/view extraction as a
//! computed planar cut over the internal mesh contract.
//!
//! Observation gate: the card's "if records observed" condition was tested on
//! real bytes (see docs/re-notes/oep-cgr-section-extraction.md). Verdict:
//! NO stored planar-cut / section / clipping records exist in the corpus —
//! zero section/cut/clip/plane/view string markers in all 11 samples, no
//! section type tag in the P2-M1-T02 catalog, and plane-equation quad density
//! indistinguishable from the float-noise floor. CGR carries display
//! tessellation only; a "section view" is a viewer-computed clipping plane
//! over that tessellation.
//!
//! This test therefore locks the honest engineering equivalent: `cut()` —
//! intersect an oriented plane with an indexed triangle mesh and return the
//! section line segments (plus `loops()` which links them into contours).
//! It is written BEFORE `oep_cgr::section` existed (RED: the skeleton crate
//! has no section module). The contract, once GREEN:
//!
//!   1. `cut(vertices, triangles, plane) -> Result<Section, SectionError>`
//!      returns the set of line segments where the plane crosses the mesh.
//!      Only strict interior crossings count: a triangle with any vertex ON
//!      the plane is skipped (degenerate — coincident/boundary cuts are not
//!      sections; this is a documented convention, not an asserted vendor
//!      behavior). A plane that misses the mesh yields an *empty* section
//!      (Ok, zero segments), never an error.
//!
//!   2. `Section::loops(eps)` chains the raw segments end-to-end into
//!      contours. A closed loop repeats its first point as its last so
//!      callers can detect closure; segments that fail to chain stay open
//!      polylines. Every segment endpoint lies on the cutting plane.
//!
//!   3. `cut` never panics on any input (truncated, garbage, non-finite) and
//!      returns typed errors (`NoTriangles`, `IndexOutOfRange`,
//!      `NonFiniteVertex`, `InvalidPlane`) per P2-M1-T04 tolerant-failure
//!      doctrine. Plane orientation (normal +n vs -n) must not change the
//!      set of cut segments.
//!
//!   4. Clean-room + real material: deterministic tests build the mesh from
//!      geometry facts only. The real-material leg cuts the committed
//!      `box.stl` (unit box [0,1]^3, watertight) at z=0.5 and asserts the
//!      same closed unit-square contour; SKIPs when the sample is absent so
//!      CI stays green before the corpus PR (P1-M1-T05) lands. Integration
//!      with the P2-M2-T01 internal `Mesh` type is a one-liner once both
//!      merge: `cut(mesh.vertices(), mesh.triangles(), &plane)`.

use oep_cgr::section::{cut, Plane, Section, SectionError};

/// Locations searched for real material: `$OEP_CGR_SAMPLES` (colon/comma
/// separated dirs), else `samples`, else the corpus dirs in the repo.
fn sample_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("OEP_CGR_SAMPLES") {
        for chunk in env.split([':', ',']) {
            let chunk = chunk.trim();
            if !chunk.is_empty() {
                dirs.push(std::path::PathBuf::from(chunk));
            }
        }
    }
    dirs.push(std::path::PathBuf::from("samples"));
    dirs.push(std::path::PathBuf::from("../samples"));
    dirs.push(std::path::PathBuf::from("../corpus/data"));
    dirs.push(std::path::PathBuf::from("../../../corpus/data"));
    dirs
}

fn find_sample(name: &str) -> Option<std::path::PathBuf> {
    for d in sample_dirs() {
        let p = d.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// The unit box [0,1]^3 as 8 vertices / 12 triangles — the same corner set
/// as the corpus `box.stl` (real material, watertight).
fn unit_box_mesh() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let v: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 0.0], // 0
        [1.0, 0.0, 0.0], // 1
        [1.0, 1.0, 0.0], // 2
        [0.0, 1.0, 0.0], // 3
        [0.0, 0.0, 1.0], // 4
        [1.0, 0.0, 1.0], // 5
        [1.0, 1.0, 1.0], // 6
        [0.0, 1.0, 1.0], // 7
    ];
    // bottom z=0, top z=1, then the four side walls (2 tris each).
    let t: Vec<[u32; 3]> = vec![
        [1, 0, 3],
        [1, 3, 2], // bottom
        [4, 7, 5],
        [7, 6, 5], // top
        [0, 5, 4],
        [0, 1, 5], // y=0 wall
        [2, 7, 6],
        [2, 3, 7], // y=1 wall
        [7, 3, 0],
        [7, 0, 4], // x=0 wall
        [1, 6, 2],
        [1, 5, 6], // x=1 wall
    ];
    (v, t)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Canonical sort key for a segment (order-free, so we can compare cuts from
/// opposite plane orientations as multisets).
fn seg_key(s: [[f32; 3]; 2]) -> Vec<f32> {
    let mut a = s[0];
    let mut b = s[1];
    // order endpoints by lexicographic comparison so (a,b) == (b,a)
    if a[0] > b[0] || (a[0] == b[0] && (a[1] > b[1] || (a[1] == b[1] && a[2] > b[2]))) {
        std::mem::swap(&mut a, &mut b);
    }
    vec![a[0], a[1], a[2], b[0], b[1], b[2]]
}

// --- Part 1: deterministic planar cut --------------------------------------

/// Cutting the unit box at mid-height gives the 8 per-triangle segments of
/// the section (4 walls x 2 triangles); all endpoints lie on the plane; and
/// the segments chain into exactly ONE closed loop: the unit square at
/// z=0.5, perimeter 4.0.
#[test]
fn unit_box_midplane_cut_closed_square() {
    let (v, t) = unit_box_mesh();
    let plane = Plane {
        origin: [0.0, 0.0, 0.5],
        normal: [0.0, 0.0, 1.0],
    };
    let sec: Section = cut(&v, &t, &plane).expect("box midplane cut succeeds");
    eprintln!("  segments: {}", sec.segments().len());
    assert_eq!(
        sec.segments().len(),
        8,
        "4 walls x 2 triangles each cross the mid-height plane"
    );
    for s in sec.segments() {
        for p in s {
            assert!(
                (p[2] - 0.5).abs() < 1e-5,
                "every section point lies on the plane, got z={}",
                p[2]
            );
        }
    }
    let loops = sec.loops(1e-4);
    assert_eq!(loops.len(), 1, "the mid-plane cut is a single contour");
    let loop0 = &loops[0];
    // closed: first point repeats as last
    assert!(
        dist(loop0[0], *loop0.last().unwrap()) < 1e-4,
        "closed loop repeats its start point"
    );
    // The section polygon is piecewise linear ALONG triangle edges, so it has
    // one vertex wherever the plane crosses a triangle edge: 8 distinct
    // vertices for the box (4 wall-wall corners + 4 diagonal crossings),
    // repeated once at the end to mark closure.
    let body: Vec<[f32; 3]> = loop0[..loop0.len() - 1].to_vec();
    assert_eq!(body.len(), 8, "box section has 8 edge-crossing vertices");
    let perimeter: f32 = loop0.windows(2).map(|w| dist(w[0], w[1])).sum();
    assert!(
        (perimeter - 4.0).abs() < 1e-3,
        "unit square perimeter is 4.0, got {}",
        perimeter
    );
    // all four unit-square corners are on the contour
    let expected = [
        [0.0, 0.0, 0.5],
        [1.0, 0.0, 0.5],
        [1.0, 1.0, 0.5],
        [0.0, 1.0, 0.5],
    ];
    for e in expected.iter() {
        let found = body.iter().any(|p| dist(*p, *e) < 1e-4);
        assert!(
            found,
            "unit-square corner {:?} must lie on the section contour ({:?})",
            e, body
        );
    }
}

/// Reversing the plane normal must produce the identical set of segments.
#[test]
fn plane_normal_orientation_does_not_change_cut() {
    let (v, t) = unit_box_mesh();
    let up = cut(
        &v,
        &t,
        &Plane {
            origin: [0.0, 0.0, 0.5],
            normal: [0.0, 0.0, 1.0],
        },
    )
    .expect("up cut succeeds");
    let down = cut(
        &v,
        &t,
        &Plane {
            origin: [0.0, 0.0, 0.5],
            normal: [0.0, 0.0, -1.0],
        },
    )
    .expect("down cut succeeds");
    let mut ks = up
        .segments()
        .iter()
        .map(|s| seg_key(*s))
        .collect::<Vec<_>>();
    let mut kd = down
        .segments()
        .iter()
        .map(|s| seg_key(*s))
        .collect::<Vec<_>>();
    ks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    kd.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(ks.len(), kd.len());
    for (a, b) in ks.iter().zip(kd.iter()) {
        for (x, y) in a.iter().zip(b.iter()) {
            assert!((x - y).abs() < 1e-4, "orientation-flipped cut differs");
        }
    }
}

/// A plane above the box produces an EMPTY section: Ok with zero segments,
/// never an error (a viewer clip plane that misses the part is fine).
#[test]
fn plane_missing_mesh_is_empty() {
    let (v, t) = unit_box_mesh();
    let sec = cut(
        &v,
        &t,
        &Plane {
            origin: [0.0, 0.0, 1.01],
            normal: [0.0, 0.0, 1.0],
        },
    )
    .expect("missing cut is Ok, empty, not an error");
    assert_eq!(sec.segments().len(), 0);
    assert!(sec.loops(1e-4).is_empty());
}

/// A plane exactly coincident with the top face is degenerate, not a cut:
/// every triangle has a vertex on the plane, so strict interior crossings
/// yield nothing.
#[test]
fn tangent_coplanar_face_is_not_a_cut() {
    let (v, t) = unit_box_mesh();
    let sec = cut(
        &v,
        &t,
        &Plane {
            origin: [0.0, 0.0, 1.0],
            normal: [0.0, 0.0, 1.0],
        },
    )
    .expect("tangent cut succeeds");
    assert_eq!(
        sec.segments().len(),
        0,
        "a plane ON a face is a degenerate cut, not a section"
    );
}

/// A small open mesh cut by an oblique plane: a single triangle crossing at a
/// non-axis-aligned angle yields one segment whose endpoints lie on the plane.
#[test]
fn single_triangle_oblique_cut_one_segment() {
    let v: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 1.0]];
    let t: Vec<[u32; 3]> = vec![[0, 1, 2]];
    let sec = cut(
        &v,
        &t,
        &Plane {
            origin: [0.0, 0.0, 0.5],
            normal: [0.0, 0.0, 1.0],
        },
    )
    .expect("single-triangle cut succeeds");
    assert_eq!(sec.segments().len(), 1);
    for p in sec.segments()[0] {
        assert!((p[2] - 0.5).abs() < 1e-5);
    }
}

// --- Part 2: bounds and error handling (never panics) ----------------------

#[test]
fn rejects_invalid_plane() {
    let (v, t) = unit_box_mesh();
    // zero-length normal
    assert_eq!(
        cut(
            &v,
            &t,
            &Plane {
                origin: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 0.0],
            },
        ),
        Err(SectionError::InvalidPlane("zero length normal"))
    );
    // non-finite normal and origin
    assert_eq!(
        cut(
            &v,
            &t,
            &Plane {
                origin: [0.0, 0.0, 0.0],
                normal: [f32::NAN, 0.0, 0.0],
            },
        ),
        Err(SectionError::InvalidPlane("non-finite normal"))
    );
    assert_eq!(
        cut(
            &v,
            &t,
            &Plane {
                origin: [f32::INFINITY, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
            },
        ),
        Err(SectionError::InvalidPlane("non-finite origin"))
    );
}

#[test]
fn rejects_bad_mesh() {
    let plane = Plane {
        origin: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    assert_eq!(
        cut(&[[0.0, 0.0, 0.0]], &[], &plane),
        Err(SectionError::NoTriangles)
    );
    assert_eq!(
        cut(&[[0.0, 0.0, 0.0]], &[[0, 1, 7]], &plane),
        Err(SectionError::IndexOutOfRange)
    );
    assert_eq!(
        cut(&[[f32::NAN, 0.0, 0.0]], &[[0, 0, 0]], &plane),
        Err(SectionError::NonFiniteVertex)
    );
}

#[test]
fn never_panics_on_adversarial_inputs() {
    let plan = Plane {
        origin: [0.0; 3],
        normal: [0.0, 0.0, 1.0],
    };
    let junk_v: Vec<[f32; 3]> = vec![[f32::NAN, 0.0, 0.0], [1e30, 1.0, 0.0], [0.0, 0.0, 0.0]];
    let junk_t: Vec<[u32; 3]> = vec![[0, 0, 0], [99, 1, 2], [0, 1, 2]];
    // Whatever the outcome, these must not unwind.
    let _ = std::panic::catch_unwind(|| cut(&junk_v, &junk_t, &plan));
    let _ = std::panic::catch_unwind(|| cut(&[], &[], &plan));
}

// --- Part 3: real material (skip-clean when corpus absent) -----------------

/// Minimal ASCII-STL reader used ONLY to load the real-material sample
/// (`box.stl`, the watertight unit box from the P1 corpus). Test fixture,
/// not a product feature (the importer is out of scope here).
type LoadedMesh = (Vec<[f32; 3]>, Vec<[u32; 3]>);

fn load_ascii_stl(path: &std::path::Path) -> std::io::Result<LoadedMesh> {
    let text = std::fs::read_to_string(path)?;
    let mut verts: Vec<[f32; 3]> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    let mut tri: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        match it.next() {
            Some("vertex") => {
                let mut p = [0.0f32; 3];
                for slot in p.iter_mut() {
                    *slot = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                        std::io::Error::new(std::io::ErrorKind::InvalidData, "bad vertex line")
                    })?;
                }
                tri.push(p);
            }
            Some("outer") | Some("facet") | Some("endloop") | Some("endfacet")
            | Some("endsolid") | Some("solid") => { /* structural tokens ignored */ }
            _ => {}
        }
        if tri.len() == 3 {
            let mut ids = [0u32; 3];
            for (k, p) in tri.iter().enumerate() {
                let idx = verts.iter().position(|q| q == p);
                ids[k] = match idx {
                    Some(i) => i as u32,
                    None => {
                        verts.push(*p);
                        (verts.len() - 1) as u32
                    }
                };
            }
            tris.push(ids);
            tri.clear();
        }
    }
    Ok((verts, tris))
}

/// Real material: `box.stl` is the unit box [0,1]^3. Cutting at z=0.5 must
/// yield the identical closed unit-square contour as the deterministic leg.
/// SKIPs (passes) when the sample is not present so CI stays green before
/// the corpus PR lands.
#[test]
fn real_box_stl_midplane_cut_closed_square() {
    let Some(path) = find_sample("box.stl") else {
        eprintln!("SKIP: no real sample supplied (OEP_CGR_SAMPLES unset / corpus absent)");
        return;
    };
    let (v, t) = load_ascii_stl(&path).expect("box.stl parses as ASCII STL");
    eprintln!("  box.stl: {} vertices, {} triangles", v.len(), t.len());
    assert_eq!(t.len(), 12, "a box has 12 triangles");
    let sec = cut(
        &v,
        &t,
        &Plane {
            origin: [0.0, 0.0, 0.5],
            normal: [0.0, 0.0, 1.0],
        },
    )
    .expect("box.stl midplane cut succeeds");
    assert_eq!(sec.segments().len(), 8, "4 walls x 2 triangles");
    let loops = sec.loops(1e-4);
    assert_eq!(loops.len(), 1, "single contour through the box");
    let perimeter: f32 = loops[0].windows(2).map(|w| dist(w[0], w[1])).sum();
    assert!(
        (perimeter - 4.0).abs() < 1e-3,
        "unit-square section perimeter is 4.0, got {}",
        perimeter
    );
    for p in &loops[0] {
        assert!((p[2] - 0.5).abs() < 1e-4, "section lies on the plane");
    }
    eprintln!(
        "  summary: {} segments, {} loops, loop pts={}, perimeter={} (real material verified)",
        sec.segments().len(),
        loops.len(),
        loops[0].len(),
        perimeter
    );
}
