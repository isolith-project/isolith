//! P2-M2-T03 acceptance test (TDD, RED first) — glTF 2.0 / GLB mesh writer
//! with a golden-file test against a reference tessellation, plus structural
//! and geometry-preservation checks.
//!
//! Written BEFORE `oep_cgr::gltf` existed (RED: this file does not compile
//! against the skeleton crate, `unresolved import oep_cgr::gltf`). The
//! contract, once GREEN, is:
//!
//! 1. `write_glb(positions, triangles, out)` emits a byte-for-byte valid
//!    binary glTF 2.0 container (GLB): 12-byte header (`glTF` magic,
//!    version 2, total length), JSON chunk (type `JSON`, space-padded to
//!    4-byte alignment), BIN chunk (type `BIN\0`, zero-padded) whose
//!    payload is the vertex + index buffers, laid out per the public
//!    Khronos glTF 2.0 spec. The bytes equal a COMMITTED golden file,
//!    generated independently from the spec by
//!    `crates/oep-cgr/tools/gen_golden_box_glb.py` (NOT by this writer),
//!    sourcing the reference tessellation (`corpus/data/box.stl`, CC0-1.0,
//!    P0-M4-T02 fixture, 12 tri / 8 verts unit box).
//! 2. `write_gltf_json(...)` + `write_gltf_bin(...)` emit the equivalent
//!    .gltf JSON document (referencing an external .bin buffer) plus the
//!    .bin payload; the JSON is schema-conformant glTF 2.0 and the pair
//!    round-trips through a renderer identically to the GLB.
//! 3. The exported geometry is EXACTLY the reference tessellation: an
//!    independent mini GLB reader in this file decodes the BIN payload back
//!    to vertex/indice lists and every vertex is one of the 8 unit-box
//!    corners, within 1e-6.
//! 4. Against REAL material, when the corpus `box.stl` (ASCII STL) is
//!    supplied via `OEP_CORPUS`, its 12 facets parse and the export
//!    preserves those 36 vertex coordinates exactly; otherwise the test
//!    skips so CI stays green before the corpus PRs merge.
//!
//! Clean-room: box.stl is a keelworks-authored CC0-1.0 fixture
//! (P0-M4-T02), and the glTF 2.0 / GLB layout is the public Khronos spec
//! (CC-BY-4.0). No proprietary bytes are used anywhere in this test.

use oep_cgr::gltf::{write_glb, write_gltf_bin, write_gltf_json};

/// The reference tessellation — the unit box corners, indexed in order of
/// first appearance in `corpus/data/box.stl` facets (P0-M4-T02 fixture).
/// Must match `gen_golden_box_glb.py` exactly (same conventions as
/// `tests/golden_stl.rs`).
fn reference_positions() -> Vec<[f32; 3]> {
    vec![
        [1.0, 0.0, 0.0], // V0
        [0.0, 1.0, 0.0], // V1
        [0.0, 0.0, 0.0], // V2
        [1.0, 1.0, 0.0], // V3
        [0.0, 0.0, 1.0], // V4
        [0.0, 1.0, 1.0], // V5
        [1.0, 0.0, 1.0], // V6
        [1.0, 1.0, 1.0], // V7
    ]
}

/// The 12 triangles of `corpus/data/box.stl`, as index triples into
/// [`reference_positions`], preserving the corpus facet order exactly.
fn reference_triangles() -> Vec<[u32; 3]> {
    vec![
        [0, 1, 2],
        [0, 3, 1],
        [4, 5, 6],
        [5, 7, 6],
        [2, 6, 4],
        [2, 0, 6],
        [3, 5, 7],
        [3, 1, 5],
        [5, 1, 2],
        [5, 2, 4],
        [0, 7, 3],
        [0, 6, 7],
    ]
}

/// Committed golden file: independent binary-GLB encoding of the reference
/// tessellation (see crates/oep-cgr/tools/gen_golden_box_glb.py — every byte
/// explained in its header). 8 verts * 12 B + 12 tris * 3 idx * 2 B (u16) =
/// 96 + 72 = 168-byte BIN after a minimal JSON chunk.
fn golden_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("box-reference.glb")
}

/// Minimal independent GLB reader for the verification legs below (reads
/// header, splits JSON/BIN chunks, no validation of the JSON itself).
struct GlbSlice<'a> {
    json: &'a [u8],
    bin: &'a [u8],
}

fn parse_glb(bytes: &[u8]) -> GlbSlice<'_> {
    assert!(bytes.len() >= 12, "GLB needs its 12-byte header");
    assert_eq!(&bytes[0..4], b"glTF", "GLB magic");
    assert_eq!(
        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
        2,
        "GLB version 2"
    );
    let total = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    assert_eq!(total, bytes.len(), "GLB header length matches file");
    let mut off = 12usize;
    let mut json = &b""[..];
    let mut bin = &b""[..];
    while off + 8 <= bytes.len() {
        let clen = u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]])
            as usize;
        let ctype = &bytes[off + 4..off + 8];
        let body = &bytes[off + 8..off + 8 + clen];
        assert_eq!(clen % 4, 0, "chunk lengths are 4-byte aligned (spec 5.2)");
        match ctype {
            b"JSON" => json = body,
            b"BIN\0" => bin = body,
            other => panic!("unknown GLB chunk type {other:?}"),
        }
        off += 8 + clen;
    }
    assert_eq!(off, total, "chunks fill the file exactly");
    assert_eq!(
        json.last().copied(),
        Some(b' '),
        "JSON chunk is space-padded"
    );
    GlbSlice { json, bin }
}

/// Decode the BIN payload of the reference export: 8 VEC3 f32 positions
/// followed by 36 u16 index values (UNSIGNED_SHORT).
fn decode_reference_bin(bin: &[u8]) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    assert_eq!(bin.len(), 96 + 72, "8*12 + 36*2 bytes");
    let pos = |k: usize, c: usize| {
        let o = 12 * k + 4 * c;
        f32::from_le_bytes([bin[o], bin[o + 1], bin[o + 2], bin[o + 3]])
    };
    let mut positions = Vec::with_capacity(8);
    for p in 0..8 {
        positions.push([pos(p, 0), pos(p, 1), pos(p, 2)]);
    }
    let idx = |k: usize| {
        let o = 96 + 2 * k;
        u16::from_le_bytes([bin[o], bin[o + 1]]) as u32
    };
    let mut triangles = Vec::with_capacity(12);
    for t in 0..12 {
        triangles.push([idx(3 * t), idx(3 * t + 1), idx(3 * t + 2)]);
    }
    (positions, triangles)
}

// --- Part 1: GLB byte-for-byte golden ---

#[test]
fn glb_matches_committed_golden_file() {
    let mut out = Vec::new();
    write_glb(&reference_positions(), &reference_triangles(), &mut out)
        .expect("GLB export must not fail on the reference tessellation");
    let golden = std::fs::read(golden_path())
        .unwrap_or_else(|e| panic!("missing golden file {}: {e}", golden_path().display()));
    assert_eq!(
        out, golden,
        "exported GLB bytes must match the golden file (independent spec encoder)"
    );
    // Sanity: the golden itself is a structurally valid GLB.
    let g = parse_glb(&golden);
    let (pos, tri) = decode_reference_bin(g.bin);
    assert_eq!(pos.len(), 8);
    assert_eq!(tri.len(), 12);
}

#[test]
fn glb_layout_is_spec_conformant() {
    let mut out = Vec::new();
    write_glb(&reference_positions(), &reference_triangles(), &mut out).unwrap();
    let g = parse_glb(&out);
    // JSON chunk carries the required top-level keys (asset version + a
    // mesh primitive referencing POSITION and indices accessors).
    let json = std::str::from_utf8(g.json).expect("JSON chunk is UTF-8");
    assert!(
        json.contains("\"version\":\"2.0\""),
        "asset version present: {json}"
    );
    assert!(json.contains("\"POSITION\""), "POSITION attribute declared");
    assert!(json.contains("\"indices\""), "indices accessor wired");
    assert!(json.contains("\"bufferViews\""), "bufferViews declared");
    // Accessors declare the box metrics.
    assert!(json.contains("\"count\":8"), "8 vertices declared");
    assert!(json.contains("\"count\":36"), "36 index values declared");
    assert!(
        !json.contains("NaN") && !json.contains("inf"),
        "no non-finite literals"
    );
    // BIN chunk geometry.
    let (pos, tri) = decode_reference_bin(g.bin);
    assert_eq!(pos.len(), 8);
    assert_eq!(tri.len(), 12);
}

// --- Part 2: geometry preservation against the reference tessellation ---

#[test]
fn glb_geometry_matches_reference_tessellation() {
    let mut out = Vec::new();
    write_glb(&reference_positions(), &reference_triangles(), &mut out).unwrap();
    let g = parse_glb(&out);
    let (pos, tri) = decode_reference_bin(g.bin);
    // 0/1 box corners are exactly representable in f32; key on bit pattern.
    let corners: std::collections::HashSet<[u32; 3]> = reference_positions()
        .into_iter()
        .map(|v| v.map(|c| c.to_bits()))
        .collect();
    assert_eq!(pos.len(), 8);
    for v in &pos {
        assert!(
            corners.contains(&v.map(|c| c.to_bits())),
            "vertex {v:?} is a box corner"
        );
    }
    assert_eq!(
        tri,
        reference_triangles(),
        "triangle indices preserved exactly"
    );
}

// --- Part 3: .gltf + .bin pair equivalence ---

#[test]
fn gltf_json_and_bin_pair_match_glb_geometry() {
    let mut json = String::new();
    write_gltf_json(
        &reference_positions(),
        &reference_triangles(),
        "box.bin",
        &mut json,
    )
    .expect("glTF JSON export must not fail on the reference tessellation");
    let mut bin = Vec::new();
    write_gltf_bin(&reference_positions(), &reference_triangles(), &mut bin).unwrap();

    // JSON: schema-conformant .gltf referencing the external buffer.
    assert!(
        json.starts_with('{') && json.ends_with('}'),
        "JSON document"
    );
    assert!(json.contains("\"version\":\"2.0\""));
    assert!(
        json.contains("\"uri\":\"box.bin\""),
        "external buffer referenced"
    );
    assert!(
        json.contains("\"byteLength\":168"),
        "buffer byte length (96+72)"
    );
    assert!(!json.contains("\"uri\":\"box.glb\""), "no GLB uri in .gltf");
    // BIN payload identical to the GLB's BIN chunk.
    let mut glb = Vec::new();
    write_glb(&reference_positions(), &reference_triangles(), &mut glb).unwrap();
    let g = parse_glb(&glb);
    assert_eq!(bin, g.bin, ".bin payload equals GLB BIN chunk");
}

#[test]
fn edge_inputs_are_handled_without_panicking() {
    // Empty mesh -> a valid minimal GLB (header + JSON, empty BIN payload).
    let mut empty = Vec::new();
    write_glb(&[], &[], &mut empty).expect("empty mesh exports");
    let g = parse_glb(&empty);
    assert_eq!(g.bin.len(), 0, "no BIN payload for an empty mesh");
    // Out-of-range triangle index -> typed error, not a panic.
    let pos = reference_positions();
    let bad = vec![[0u32, 1, 99]];
    let mut out = Vec::new();
    assert!(
        write_glb(&pos, &bad, &mut out).is_err(),
        "out-of-range index is a typed error"
    );
    // Degenerate triangle (duplicate indices) -> typed error like STL.
    let degen = vec![[0u32, 0, 0]];
    let mut out = Vec::new();
    assert!(
        write_glb(&pos, &degen, &mut out).is_err(),
        "degenerate triangle is a typed error"
    );
}

// --- Part 4: real material (skip-clean when corpus absent) ---

/// Search locations for the corpus reference: `$OEP_CORPUS` (path to the
/// corpus `data/` dir) or conventional relative paths, matching the
/// `OEP_CGR_SAMPLES` pattern used by the sibling card.
fn corpus_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(env) = std::env::var("OEP_CORPUS") {
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
    dirs
}

/// Minimal ASCII-STL vertex extractor for the corpus reference fixture.
fn parse_ascii_stl_vertices(text: &str) -> Vec<([f32; 3], [f32; 3], [f32; 3])> {
    let mut facets = Vec::new();
    let mut current: Vec<[f32; 3]> = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("vertex") {
            let parts: Vec<f32> = rest
                .split_whitespace()
                .map(|p| p.parse().unwrap())
                .collect();
            assert_eq!(parts.len(), 3);
            current.push([parts[0], parts[1], parts[2]]);
        } else if l.starts_with("endfacet") {
            assert_eq!(current.len(), 3, "each facet has exactly 3 vertices");
            facets.push((current[0], current[1], current[2]));
            current = Vec::new();
        }
    }
    facets
}

#[test]
fn real_material_box_stl_geometry_preserved() {
    let mut found = None;
    for dir in corpus_dirs() {
        let probe = dir.join("box.stl");
        if probe.is_file() {
            found = Some(probe);
            break;
        }
    }
    let Some(path) = found else {
        eprintln!("SKIP  corpus box.stl not found (set OEP_CORPUS=<corpus/data dir>); tolerant CI-safe skip");
        return;
    };
    eprintln!(
        "info  verifying export on real material: {}",
        path.display()
    );

    let ascii = std::fs::read_to_string(&path).expect("read corpus box.stl");
    let corpus_facets = parse_ascii_stl_vertices(&ascii);
    assert_eq!(corpus_facets.len(), 12, "corpus box.stl has 12 facets");
    let corpus_vertices: Vec<[f32; 3]> = corpus_facets
        .into_iter()
        .flat_map(|(a, b, c)| [a, b, c])
        .collect();

    // Reference mesh carries the exact same 36 vertex coordinates (indexed).
    let positions = reference_positions();
    let triangles = reference_triangles();
    let ours: Vec<[f32; 3]> = triangles
        .iter()
        .flat_map(|t| t.iter().map(|&i| positions[i as usize]))
        .collect();
    assert_eq!(corpus_vertices.len(), ours.len(), "both list 36 vertices");
    for (k, (c, o)) in corpus_vertices.iter().zip(ours.iter()).enumerate() {
        for axis in 0..3 {
            assert!(
                (c[axis] - o[axis]).abs() < 1e-6,
                "vertex {k} axis {axis}: corpus {} vs ours {o:?}",
                c[axis]
            );
        }
    }

    // Full pipeline on the real reference: corpus -> internal -> GLB and
    // verify the binary bytes are the committed golden (geometry identical).
    let mut glb = Vec::new();
    write_glb(&positions, &triangles, &mut glb)
        .expect("GLB export on real reference must not fail");
    let golden = std::fs::read(golden_path())
        .unwrap_or_else(|e| panic!("missing golden file {}: {e}", golden_path().display()));
    assert_eq!(glb, golden, "real-material GLB matches the golden file");
    eprintln!(
        "PASS  real corpus box.stl: 12 facets -> 36 vertex coords preserved exactly \
         (GLB {} bytes == golden)",
        glb.len()
    );
}
