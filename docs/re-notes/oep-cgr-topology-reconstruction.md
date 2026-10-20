# oep-cgr Face/Edge Topology Reconstruction (open/unclosed boundary flags)

P2-M3-T02 · Isolith / OEP P2 CGR Reader
Status: TDD acceptance test GREEN on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

A self-contained face/edge topology reconstruction pass for indexed triangle
meshes — the "topology pass" named by P2 milestone M3. It does NOT parse CGR
bytes; it consumes the same data contract the internal `Mesh` type (P2-M2-T01)
exposes — `vertices: &[[f32; 3]]`, `triangles: &[[u32; 3]]` — and rebuilds the
combinatorial surface topology. Integration is one call once both merge:

    reconstruct(mesh.vertices(), mesh.triangles())

Deliverables:

- `crates/oep-cgr/src/topology.rs` — `reconstruct`, `Topology`, `Face`,
  `Edge`, `TopoError` (typed, non-panicking; bounds/finiteness validated).
- `crates/oep-cgr/tests/topology_reconstruct.rs` — the TDD acceptance test
  (written RED against the skeleton crate, then GREEN).
- This RE note.

## 2. What "topology reconstruction" reconstructs

Given an indexed triangle soup (the tessellation a CGR rep ultimately
carries), the pass recomputes:

| Reconstructed fact | Meaning |
|---|---|
| Unique undirected edges | every edge of the triangle soup, canonicalized (a<b) |
| Edge incidence | which faces (triangles) use each edge |
| Boundary edge | used by exactly one face — the open/unclosed boundary of the surface |
| Interior edge | used by exactly two faces — a manifold edge |
| Non-manifold edge | used by more than two faces — flagged, never a crash |
| Face shells | connected components of triangles joined along shared edges |
| Boundary loops | open contours traced along boundary edges; empty when closed |
| Open/unclosed flags | per-shell and whole-mesh: closed = zero boundary + zero non-manifold edges (watertight); otherwise open |

This is the classic Euler/adjacency computation over an indexed mesh: with
V/vertices, E/edges and F/faces of a closed surface, E = F*3/2 and the Euler
characteristic is observable; the boundary edge count is 0 iff watertight.
The pass is format-independent (no CGR knowledge needed) so it compiles and
test-flies before mesh extraction merges — deliberately mirroring the
self-contained pattern both P2-M1-T01 `header` and P2-M2-T01 `mesh` use for
the still-unmerged sibling modules.

## 3. TDD contract (the acceptance test)

Written RED first (the test named `oep_cgr::topology`, absent from the
skeleton crate). Once GREEN it locks:

1. Deterministic topology on hand-built meshes:
   - one triangle -> 3 boundary edges, 1 open shell, one 3-vertex loop;
   - closed tetrahedron -> 6 interior edges, 0 boundary, 1 closed shell,
     no loops (watertight);
   - open quad (2 triangles sharing a diagonal) -> 4 boundary + 1 interior
     edge, one open shell, one 4-vertex boundary loop;
   - two disconnected triangles -> two separate face shells;
   - a 3-way shared edge -> exactly one non-manifold edge, flagged, no panic.
2. Error handling: empty triangle list -> `NoTriangles`; out-of-range index ->
   `IndexOutOfRange`; non-finite coordinate -> `NonFiniteVertex`. Adversarial
   inputs must never unwind.
3. Real material: `box.stl` (a watertight closed unit box, 8 verts, 12 tris,
   from the P1 corpus) reconstructs as CLOSED — zero boundary edges, one face
   shell, no boundary loops. The leg SKIPs cleanly when the sample is absent
   (CI stays green until the corpus PR P1-M1-T05 lands).

Verification transcript (this host, 2026-10-15, cargo 1.98.1):

    running 9 tests ... test result: ok. 9 passed; 0 failed
    box.stl: 8 vertices, 12 triangles
    summary: 18 edges, 1 faces, closed=true (real material verified)

Quality gates (mirror CI: fmt, clippy -D warnings, unit, integration):

    cargo fmt --all -- --check                    PASS
    cargo clippy --workspace --all-targets -D warnings  PASS
    cargo test --workspace --lib                  PASS
    cargo test --workspace --tests                PASS (9/9, real box leg included)

## 4. Clean-room boundary

- No CGR or vendor bytes are committed. The test constructs mesh inputs from
  geometry facts only; the real-material leg reads a committed, license-clean
  corpus ASCII-STL (`box.stl`) via a pure-stdlib test fixture.
- The pass makes no claims about CGR's internal record layout; locating the
  triangle stream inside a CGR payload remains P2-M2-T01's extraction job.
- No dependency on the still-unmerged `oep_cgr::mesh` module: `reconstruct`
  is written against `&[[f32;3]]` / `&[[u32;3]]` directly. When both cards
  merge, the glue is a one-liner.

## 5. Version history

- 1.0 2026-10-15 initial topology reconstruction task (P2-M3-T02).
