//! P3-M4-T02 acceptance test (TDD, RED first) — round-trip stability:
//! read OUR OWN 3DXML ASCII output back and byte-class compare.
//!
//! The writer (P3-M4-T01, `crate::ascii`) turns an internal indexed triangle
//! mesh into a 3DXML ASCII ZIP package. This test proves the DEEPEST
//! stability property on top of the writer's own golden lock: the write ->
//! read-back -> write cycle is a fixed point.
//!
//!   P1 = write_package(mesh)
//!   mesh' = decode(P1)                # read our own output (test-local mini
//!                                   #   reader mirroring the writer's helpers;
//!                                   #   a real rep parser lands in M1-T02/T03)
//!   P2 = write_package(mesh')
//!   assert P2 == P1 byte-for-byte     # "byte-class compare" = strict byte equality
//!
//! Why this is the strongest stability claim:
//!   - the writer is fully deterministic (fixed DOS timestamp, LF endings,
//!     Rust f32 shortest-notation Display), so byte-equality across a cycle is
//!     achievable and meaningful — it is NOT a flaky assertion;
//!   - f32 Display has the shortest-round-trip guarantee, so parsing the
//!     emitted text back yields bit-identical positions, and re-writing yields
//!     the same shortest text — the whole archive is a fixed point;
//!   - triangle strips are emitted as one 3-index run per triangle, so index
//!     decode is exact regardless of how the caller packs indices.
//!
//! Test meshes:
//!   1. `box_mesh` — the isolith CC0-1.0 unit box (exact 0/1 coordinates).
//!   2. `noisy_mesh` — non-exact f32 values (0.1, 3.14, 2.5e-3, ...) that prove
//!      the shortest-round-trip Display survives a full cycle, not just 0/1.
//!   3. `remap_mesh` — index holes and out-of-order index values (positions
//!      array is not densely indexed) to prove index decode is literal, with a
//!      guaranteed-non-degenerate triangle list.
//!
//! License: the format here is consumed/produced only from observed facts of
//! public MIT/Apache-2.0 samples (hex notebook) + the internal-use 3DXML spec;
//! no Dassault XSD text is reproduced.
#![deny(missing_docs)]

use std::panic::{catch_unwind, AssertUnwindSafe};

use std::f32::consts::{PI, TAU};

/// The writer's public contract (P3-M4-T01). RED: `ascii` does not exist yet.
use oep_3dxml::ascii::{write_package, write_rep_markup, WriteError, WriterOpts};

/// The isolith CC0-1.0 reference box tessellation (as in the writer's tests).
fn box_mesh() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let positions = vec![
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    ];
    let triangles = vec![
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
    ];
    (positions, triangles)
}

/// A mesh with NON-exact f32 coordinates (shortest-round-trip must survive).
/// All values finite; two back-to-back triangles so volume is nonzero.
fn noisy_mesh() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let positions = vec![
        [0.1, 0.0, 0.0],
        [0.0, 0.1, 0.0],
        [0.0, 0.0, 0.1],
        [PI, 0.0, 0.0],
        [0.0, TAU, 0.0],
        [1.75e-3, -2.5e-3, 4.125],
        [-1.0e1, 2.0e1, 1.0 / 3.0],
        [1e-30, 5e30, -0.0],
    ];
    let triangles = vec![[0, 1, 2], [2, 1, 3], [4, 5, 6], [6, 5, 7]];
    (positions, triangles)
}

/// A mesh with index holes and out-of-order indices: positions 6 and 7 are
/// never referenced, and the triangles reference a spread of indices.
/// Guaranteed non-degenerate (no collinear faces).
fn remap_mesh() -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let positions: Vec<[f32; 3]> = vec![
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, 2.0],
        [2.0, 2.0, 0.0],
        [2.0, 0.0, 2.0],
        [9.0, 9.0, 9.0], // hole 1 (unused)
        [9.0, 8.0, 9.0], // hole 2 (unused)
        [2.0, 2.0, 2.0],
    ];
    // Indices deliberately out of order and spread (0..5, 8 used; 6,7 holes).
    let triangles = vec![
        [4, 3, 5],
        [3, 1, 5],
        [2, 0, 8],
        [8, 4, 2],
        [0, 1, 2],
        [5, 4, 8],
    ];
    (positions, triangles)
}

fn opts() -> WriterOpts<'static> {
    WriterOpts {
        product_name: "roundtrip",
        author: "isolith",
        timestamp: "",
    }
}

/// Write a full package to bytes via the writer under test.
fn package_bytes(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<u8> {
    let mut buf = Vec::new();
    write_package(positions, triangles, &opts(), &mut buf).expect("package write ok");
    buf
}

// ---------------------------------------------------------------------------
// Test-local mini READER for the ASCII .3DRep branch (pending M1-T02/M1-T03).
// Mirrors the writer's independent helpers so we read our own output with our
// own decode logic, not with the writer itself.
// ---------------------------------------------------------------------------

/// Walk the ZIP central directory; return (name, csize, usize, data-offset).
fn central_entries(pkg: &[u8]) -> Vec<(String, u32, u32, u32)> {
    let mut eocd = None;
    for i in (0..=pkg.len().saturating_sub(22)).rev() {
        if pkg[i..i + 4] == [0x50, 0x4b, 0x05, 0x06] {
            eocd = Some(i);
            break;
        }
    }
    let eocd = eocd.expect("end-of-central-directory signature found");
    let cd_size = u32::from_le_bytes(pkg[eocd + 12..eocd + 16].try_into().unwrap());
    let cd_off = u32::from_le_bytes(pkg[eocd + 16..eocd + 20].try_into().unwrap());
    let mut entries = Vec::new();
    let mut p = cd_off as usize;
    let end = cd_off as usize + cd_size as usize;
    while p < end {
        assert_eq!(
            &pkg[p..p + 4],
            &[0x50, 0x4b, 0x01, 0x02],
            "central directory signature"
        );
        let name_len = u16::from_le_bytes(pkg[p + 28..p + 30].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(pkg[p + 30..p + 32].try_into().unwrap()) as usize;
        let comment_len = u16::from_le_bytes(pkg[p + 32..p + 34].try_into().unwrap()) as usize;
        let csize = u32::from_le_bytes(pkg[p + 20..p + 24].try_into().unwrap());
        let usize_ = u32::from_le_bytes(pkg[p + 24..p + 28].try_into().unwrap());
        let offset = u32::from_le_bytes(pkg[p + 42..p + 46].try_into().unwrap());
        let name = String::from_utf8(pkg[p + 46..p + 46 + name_len].to_vec()).unwrap();
        entries.push((name, csize, usize_, offset));
        p += 46 + name_len + extra_len + comment_len;
    }
    entries
}

/// Return one STORED member's data bytes (writer-independent ZIP read).
fn zip_member<'a>(pkg: &'a [u8], name: &str) -> &'a [u8] {
    let (_, csize, usize_, offset) = central_entries(pkg)
        .into_iter()
        .find(|(n, _, _, _)| n == name)
        .unwrap_or_else(|| panic!("member {name} present in zip"));
    assert_eq!(csize, usize_, "STORED member: compressed == uncompressed");
    let data_off = offset as usize + 30 + name.len();
    &pkg[data_off..data_off + usize_ as usize]
}

/// Extract the `<Positions>` text block (comma-separated "x y z" triples).
fn positions_text(xml: &str) -> String {
    let start = xml.find("<Positions>").expect("Positions open tag") + "<Positions>".len();
    let end = xml.find("</Positions>").expect("Positions close tag");
    xml[start..end].to_string()
}

/// Extract the `strips` attribute text (comma-separated index runs).
fn strips_text(xml: &str) -> String {
    let start = xml.find("strips=\"").expect("strips attr") + "strips=\"".len();
    let end = xml[start..].find('"').expect("closing quote") + start;
    xml[start..end].to_string()
}

/// Decode the ASCII .3DRep back into a mesh. Positions parse to f32
/// (bit-exact via the writer's shortest-round-trip emission); strips decode to
/// one 3-index triangle per run — a strip reader must do exactly this.
fn decode_rep(pkg: &[u8], rep_member: &str) -> (Vec<[f32; 3]>, Vec<[u32; 3]>) {
    let rep = std::str::from_utf8(zip_member(pkg, rep_member)).expect("rep is UTF-8");
    let positions: Vec<[f32; 3]> = positions_text(rep)
        .split(',')
        .map(|triple| {
            let mut it = triple.split_whitespace();
            [
                it.next().expect("x").parse::<f32>().expect("f32"),
                it.next().expect("y").parse::<f32>().expect("f32"),
                it.next().expect("z").parse::<f32>().expect("f32"),
            ]
        })
        .collect();
    let triangles: Vec<[u32; 3]> = strips_text(rep)
        .split(',')
        .map(|run| {
            let v: Vec<u32> = run
                .split_whitespace()
                .map(|i| i.parse::<u32>().expect("u32"))
                .collect();
            assert_eq!(v.len(), 3, "one 3-index strip-run per triangle");
            [v[0], v[1], v[2]]
        })
        .collect();
    (positions, triangles)
}

fn mesh_member_name(product: &str) -> String {
    format!("{product}.3DRep")
}

// ---------------------------------------------------------------------------
// The round-trip stability tests
// ---------------------------------------------------------------------------

/// Result of one write -> read -> write cycle (used to keep tuples named).
struct Cycle {
    first: Vec<u8>,
    decoded: (Vec<[f32; 3]>, Vec<[u32; 3]>),
    second: Vec<u8>,
}

/// Cycle a mesh through write -> read -> write and return all three artifacts.
fn cycle(positions: &[[f32; 3]], triangles: &[[u32; 3]], rep_member: &str) -> Cycle {
    let first = package_bytes(positions, triangles);
    let decoded = decode_rep(&first, rep_member);
    let second = package_bytes(&decoded.0, &decoded.1);
    Cycle {
        first,
        decoded,
        second,
    }
}

#[test]
fn box_roundtrip_is_byte_identical() {
    let (pos, tri) = box_mesh();
    let c = cycle(&pos, &tri, &mesh_member_name("roundtrip"));
    assert_eq!(c.decoded.0, pos, "positions read back bit-identical");
    assert_eq!(c.decoded.1, tri, "triangles read back exactly");
    assert_eq!(
        c.second, c.first,
        "write(read(write(mesh))) == write(mesh) — byte-class compare"
    );
}

#[test]
fn noisy_f32_roundtrip_is_byte_identical() {
    let (pos, tri) = noisy_mesh();
    let c = cycle(&pos, &tri, &mesh_member_name("roundtrip"));
    assert_eq!(
        c.decoded.0, pos,
        "non-exact f32 positions round-trip bit-exact"
    );
    assert_eq!(c.decoded.1, tri, "triangles round-trip exactly");
    assert_eq!(
        c.second, c.first,
        "byte-class compare holds for shortest-round-trip f32"
    );
}

#[test]
fn remapped_indices_roundtrip_is_byte_identical() {
    let (pos, tri) = remap_mesh();
    let c = cycle(&pos, &tri, &mesh_member_name("roundtrip"));
    assert_eq!(c.decoded.0, pos, "positions (with holes) read back exactly");
    assert_eq!(
        c.decoded.1, tri,
        "out-of-order index values read back literally"
    );
    assert_eq!(
        c.second, c.first,
        "indices are decoded literally, byte-class compare holds"
    );
}

/// Four consecutive cycles must all be byte-identical (a true fixed point).
#[test]
fn four_cycles_are_a_fixed_point() {
    let (pos, tri) = noisy_mesh();
    let rep_member = mesh_member_name("roundtrip");
    let mut buf = package_bytes(&pos, &tri);
    let baseline = buf.clone();
    for step in 0..4u32 {
        let (p, t) = decode_rep(&buf, &rep_member);
        buf = package_bytes(&p, &t);
        assert_eq!(buf, baseline, "cycle {step} stable");
    }
}

/// The rep markup alone also round-trips byte-identically through read-back.
#[test]
fn rep_markup_roundtrips_too() {
    let (pos, tri) = noisy_mesh();
    let mut first = Vec::new();
    write_rep_markup(&pos, &tri, &opts(), &mut first).expect("rep write ok");
    let xml = String::from_utf8(first.clone()).expect("rep is UTF-8");
    // Decode via the same text extraction.
    let positions: Vec<[f32; 3]> = positions_text(&xml)
        .split(',')
        .map(|triple| {
            let mut it = triple.split_whitespace();
            [
                it.next().unwrap().parse::<f32>().unwrap(),
                it.next().unwrap().parse::<f32>().unwrap(),
                it.next().unwrap().parse::<f32>().unwrap(),
            ]
        })
        .collect();
    let triangles: Vec<[u32; 3]> = strips_text(&xml)
        .split(',')
        .map(|run| {
            let v: Vec<u32> = run.split_whitespace().map(|i| i.parse().unwrap()).collect();
            [v[0], v[1], v[2]]
        })
        .collect();
    let mut second = Vec::new();
    write_rep_markup(&positions, &triangles, &opts(), &mut second).expect("rep write ok");
    assert_eq!(second, first, "rep markup round-trips byte-identically");
}

/// The `roundtrip.3DRep` member inside the package must equal the standalone
/// rep writer output (member byte-class framing is stable, not just geometry).
#[test]
fn package_rep_member_equals_standalone_rep() {
    let (pos, tri) = box_mesh();
    let pkg = package_bytes(&pos, &tri);
    let rep_in_pkg = zip_member(&pkg, "roundtrip.3DRep").to_vec();
    let mut standalone = Vec::new();
    write_rep_markup(&pos, &tri, &opts(), &mut standalone).expect("rep write ok");
    assert_eq!(
        rep_in_pkg, standalone,
        "package rep member byte-identical to standalone rep writer"
    );
}

// ---------------------------------------------------------------------------
// Defense: typed errors, never panic (writer contract sanity inside the cycle).
// ---------------------------------------------------------------------------

#[test]
fn writer_still_returns_typed_errors_never_panics() {
    // Empty mesh -> EmptyMesh.
    let none: Vec<[f32; 3]> = Vec::new();
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut buf = Vec::new();
        write_package(&none, &[[0u32, 1, 2]], &opts(), &mut buf)
    }));
    let result = r.expect("no panic");
    assert!(matches!(result, Err(WriteError::EmptyMesh)));

    // Out-of-range index -> IndexOutOfRange.
    let pos = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut buf = Vec::new();
        write_package(&pos, &[[0u32, 1, 7]], &opts(), &mut buf)
    }))
    .expect("no panic");
    match r {
        Err(WriteError::IndexOutOfRange { index, len }) => {
            assert_eq!(index, 7);
            assert_eq!(len, 3);
        }
        other => panic!("expected IndexOutOfRange, got {other:?}"),
    }
}
