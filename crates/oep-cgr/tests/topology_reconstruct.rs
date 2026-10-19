//! P2-M3-T02 acceptance test (TDD, RED first) — face/edge topology
//! reconstruction with open/unclosed boundary flags.
//!
//! Written BEFORE the `oep_cgr::topology` module existed (RED: this file
//! does not compile against the skeleton crate — there is no `oep_cgr::
//! topology` module). The contract, once GREEN, is:
//!
//!   1. `reconstruct(vertices, triangles) -> Result<Topology, TopoError>`
//!      rebuilds, from an indexed triangle mesh, the combinatorial topology of
//!      the surface: unique undirected edges, their incidence (which faces use
//!      each edge), the face shells (connected components of triangles joined
//!      along shared edges), and the boundary loops (open contours).
//!
//!   2. Each edge is classified by its incident-face count. `faces == 1`
//!      means a boundary edge (the open/unclosed boundary of the surface);
//!      `faces == 2` means an interior edge; more than two means a
//!      non-manifold edge (flagged, never crashes the pass). A surface with
//!      zero boundary edges is *closed* (watertight); otherwise it is *open*
//!      (has boundary edges). Every `Face` shell and the whole mesh expose
//!      these open/unclosed flags.
//!
//!   3. `reconstruct` is combinatorial (index-based): the vertex positions are
//!      used ONLY to validate that every triangle index is in range and every
//!      referenced coordinate is finite. It never panics on any input
//!      (truncated, garbage, or real corpus) and returns a typed error.
//!
//!   4. Clean-room + real material: the deterministic tests build meshes only
//!      from observed format facts. The real-material leg reconstructs the
//!      topology of a real committed triangle mesh (`box.stl`, a watertight
//!      closed box) and asserts the box reconstructs as *closed* (zero boundary
//!      edges, one face shell, no boundary loops). It SKIPs when the sample is
//!      absent so CI stays green before the corpus PR (P1-M1-T05) lands.
//!      Integration with the P2-M2-T01 internal `Mesh` type is a one-liner once
//!      both merge: `reconstruct(mesh.vertices(), mesh.triangles())`.

use oep_cgr::topology::{reconstruct, TopoError, Topology};

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

// --- Part 1: deterministic topology reconstruction -------------------------

/// One triangle in the XY plane: 3 boundary edges, 1 open face shell,
/// one 3-vertex boundary loop.
#[test]
fn single_triangle_reconstructs_as_open() {
    let v: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let t: Vec<[u32; 3]> = vec![[0, 1, 2]];
    let topo: Topology = reconstruct(&v, &t).expect("single triangle reconstructs");
    assert_eq!(topo.edge_count(), 3);
    assert_eq!(
        topo.boundary_edge_count(),
        3,
        "all 3 edges are open boundary"
    );
    assert_eq!(topo.interior_edge_count(), 0);
    assert_eq!(topo.non_manifold_edge_count(), 0);
    assert!(topo.is_open(), "has boundary edges -> open");
    assert!(!topo.is_closed(), "not watertight");
    let faces = topo.faces();
    assert_eq!(faces.len(), 1, "one face shell");
    assert_eq!(faces[0].triangle_indices(), &vec![0][..]);
    assert!(!faces[0].is_closed(), "face shell open too");
    let loops = topo.boundary_loops();
    assert_eq!(loops.len(), 1, "one boundary loop");
    let mut verts: Vec<u32> = loops[0].clone();
    verts.sort_unstable();
    assert_eq!(
        verts,
        vec![0, 1, 2],
        "loop visits all 3 vertices exactly once"
    );
}

/// A closed tetrahedron: 4 triangles, 6 edges, ALL interior, one closed face
/// shell, no boundary edges, no boundary loops -> watertight.
#[test]
fn closed_tetrahedron_reconstructs_as_closed() {
    let v: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let t: Vec<[u32; 3]> = vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 2, 3]];
    let topo: Topology = reconstruct(&v, &t).expect("tetrahedron reconstructs");
    assert_eq!(topo.edge_count(), 6, "all vertex pairs of 4 corners");
    assert_eq!(
        topo.boundary_edge_count(),
        0,
        "closed surface has no boundary"
    );
    assert_eq!(topo.interior_edge_count(), 6);
    assert_eq!(topo.non_manifold_edge_count(), 0);
    assert!(topo.is_closed(), "watertight");
    assert!(!topo.is_open());
    let faces = topo.faces();
    assert_eq!(faces.len(), 1, "one closed shell");
    assert_eq!(faces[0].triangle_indices().len(), 4);
    assert!(faces[0].is_closed());
    assert!(topo.boundary_loops().is_empty(), "no open contours");
}

/// An open square (two triangles sharing the diagonal): 5 edges, the shared
/// diagonal is interior, the 4 outer edges are boundary. One open shell.
#[test]
fn open_quad_diagonal_is_interior_outers_are_boundary() {
    let v: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let t: Vec<[u32; 3]> = vec![[0, 1, 2], [0, 2, 3]];
    let topo: Topology = reconstruct(&v, &t).expect("quad reconstructs");
    assert_eq!(topo.edge_count(), 5, "4 outer + 1 diagonal");
    assert_eq!(topo.boundary_edge_count(), 4, "outer boundary");
    assert_eq!(topo.interior_edge_count(), 1, "the diagonal");
    assert!(topo.is_open());
    let faces = topo.faces();
    assert_eq!(faces.len(), 1, "two triangles are one connected shell");
    assert!(!faces[0].is_closed());
    let loops = topo.boundary_loops();
    assert_eq!(loops.len(), 1, "one closed quad boundary loop");
    let mut verts: Vec<u32> = loops[0].clone();
    verts.sort_unstable();
    assert_eq!(verts, vec![0, 1, 2, 3], "outer loop visits all 4 corners");
}

/// Two triangles sharing no edge are two separate face shells.
#[test]
fn disconnected_triangles_are_two_shells() {
    let v: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [5.0, 5.0, 5.0],
        [6.0, 5.0, 5.0],
        [5.0, 6.0, 5.0],
    ];
    let t: Vec<[u32; 3]> = vec![[0, 1, 2], [3, 4, 5]];
    let topo: Topology = reconstruct(&v, &t).expect("two shells reconstruct");
    assert_eq!(topo.faces().len(), 2, "two disconnected face shells");
    assert_eq!(
        topo.boundary_edge_count(),
        6,
        "2 triangles x 3 boundary edges"
    );
}

/// An edge shared by THREE triangles is non-manifold: flagged, not a crash.
#[test]
fn non_manifold_edge_is_flagged() {
    let v: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 0.0],  // 0
        [1.0, 0.0, 0.0],  // 1
        [0.0, 1.0, 0.0],  // 2
        [0.0, -1.0, 0.0], // 3
        [0.0, 0.0, 1.0],  // 4
    ];
    // three triangles all share edge (0,1)
    let t: Vec<[u32; 3]> = vec![[0, 1, 2], [0, 1, 3], [0, 1, 4]];
    let topo: Topology = reconstruct(&v, &t).expect("non-manifold reconstructs without panic");
    assert_eq!(
        topo.non_manifold_edge_count(),
        1,
        "edge 0-1 is non-manifold"
    );
    assert_eq!(
        topo.boundary_edge_count(),
        6,
        "the other 6 edge-uses are single-incidence"
    );
    assert_eq!(topo.interior_edge_count(), 0);
}

// --- Part 2: bounds and error handling (never panics) ----------------------

#[test]
fn rejects_out_of_range_triangle_index() {
    let v: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let t: Vec<[u32; 3]> = vec![[0, 1, 7]]; // index 7 does not exist
    assert_eq!(reconstruct(&v, &t), Err(TopoError::IndexOutOfRange));
}

#[test]
fn rejects_empty_triangle_list() {
    let v: Vec<[f32; 3]> = vec![[0.0, 0.0, 0.0]];
    let t: Vec<[u32; 3]> = vec![];
    assert_eq!(reconstruct(&v, &t), Err(TopoError::NoTriangles));
}

#[test]
fn never_panics_on_adversarial_inputs() {
    let junk: Vec<[f32; 3]> = vec![[f32::NAN, 0.0, 0.0], [1e30, 1.0, 0.0]];
    let tris: Vec<[u32; 3]> = vec![[0, 0, 0], [99, 1, 2], [0, 1, 2]];
    // Regardless of the result value, this must not unwind.
    let _ = std::panic::catch_unwind(|| reconstruct(&junk, &tris));
    let _ = std::panic::catch_unwind(|| reconstruct(&[], &[]));
}

// --- Part 3: real material (skip-clean when corpus absent) ----------------

/// Minimal ASCII-STL reader used ONLY to load the real-material sample
/// (`box.stl`, a watertight closed box). This is a test fixture, not a
/// product feature (the P2-M2-T02 STL writer is the export lane; the importer
/// is not in scope here).
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
                let x: f32 = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "bad vertex line")
                })?;
                let y: f32 = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "bad vertex line")
                })?;
                let z: f32 = it.next().and_then(|s| s.parse().ok()).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "bad vertex line")
                })?;
                tri.push([x, y, z]);
            }
            Some("outer") | Some("facet") | Some("endloop") | Some("endfacet")
            | Some("endsolid") | Some("solid") => { /* structural tokens ignored */ }
            _ => {}
        }
        if tri.len() == 3 {
            // de-duplicate by exact coordinate match
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

/// Real material: `box.stl` is a unit box, 12 triangles, watertight. The
/// reconstruction MUST report it closed (zero boundary edges, one face shell,
/// no boundary loops). SKIPs (passes) when the sample is not present so CI
/// stays green before the corpus PR lands.
#[test]
fn real_box_stl_reconstructs_as_closed() {
    let Some(path) = find_sample("box.stl") else {
        eprintln!("SKIP: no real sample supplied (OEP_CGR_SAMPLES unset / corpus absent)");
        return;
    };
    let (v, t) = load_ascii_stl(&path).expect("box.stl parses as ASCII STL");
    eprintln!("  box.stl: {} vertices, {} triangles", v.len(), t.len());
    assert_eq!(t.len(), 12, "a box has 12 triangles");
    let topo = reconstruct(&v, &t).expect("box reconstructs");
    assert!(topo.is_closed(), "a watertight box is closed");
    assert_eq!(topo.boundary_edge_count(), 0, "no open boundary on a box");
    assert_eq!(topo.faces().len(), 1, "one connected shell");
    assert!(topo.faces()[0].is_closed());
    assert!(
        topo.boundary_loops().is_empty(),
        "no open contours on a box"
    );
    eprintln!(
        "  summary: {} edges, {} faces, closed={} (real material verified)",
        topo.edge_count(),
        topo.faces().len(),
        topo.is_closed()
    );
}
