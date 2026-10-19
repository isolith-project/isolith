//! Lattice surface meshes: the "surface rep" a compliant 3DXML/3DS viewer
//! reinterprets into a smooth Gregory-patch surface.
//!
//! # What a lattice surface mesh is
//!
//! A lattice is a coarse mesh of vertices, faces and edges, where each vertex
//! and each edge optionally carries a ROUNDING WEIGHT and each edge optionally
//! carries two endpoint tangent VECTORS. The surface and the mesh share the
//! same topology; a compliant viewer reconstructs a free-form surface (a
//! network of bicubic GREGORY patches) from the lattice by an invertible
//! rounding algorithm, and can round-trip back to the mesh. Rounding weights
//! conventionally range roughly 0.5 (sharp) .. 1.0 (default) .. 1.5 (round).
//!
//! This is the mathematical family Dassault licensed from Lattice Technology
//! for 3DXML's surface data (see `docs/re-notes/gregory-patch-decode.md`).
//! The public, license-clean, real-material instantiation of the same family
//! is the Web3D Consortium LatticeXvl `XvlShell` examples committed under
//! `corpus/data/3dxml/surface/` (BSD-style Web3D license); this module parses
//! exactly those files and decodes each face's Gregory patch to NURBS.
//!
//! # The v1 reconstruction (documented, `[uncertain]` where CATIA-specific)
//!
//! - Corner control points reproduce lattice vertices exactly.
//! - Edge control points reproduce the cubic Bezier boundary curves encoded by
//!   the edge tangent vectors (endpoint tangents = `edge*vector / 3`), scaled
//!   by the edge rounding weight. Edges without a stored vector decode to a
//!   straight chord (matches the `edgeBeginVector = (0 0 0)` sample exactly).
//! - Interior twist control points use a harmonic-equilibrium fill (each
//!   interior control point equals the mean of its four grid neighbours with
//!   fixed boundary) — a smooth, twist-free, boundary-exact interior. The
//!   exact CATIA/Lattice rounding-weight stencil is proprietary and NOT
//!   reproduced in v1; `vertexRound` and the weight-vs-vector interaction are
//!   parsed and carried but do not yet alter the v1 interior.
//! - Non-quad faces fan into quads around the face centroid (edge midpoints +
//!   centroid), each decoded as a quad.

use std::collections::HashMap;

use crate::gregory::GregoryPatch;
use crate::vec3::Vec3;

/// A directed edge of a lattice mesh with optional rounding data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LatticeEdge {
    /// First endpoint vertex index.
    pub v0: u32,
    /// Second endpoint vertex index.
    pub v1: u32,
    /// Edge rounding weight (1.0 default; 0.5 sharp .. 1.5 round).
    pub round: f64,
    /// Tangent (velocity) vector at `v0`, when the source supplies one.
    pub tangent0: Option<Vec3>,
    /// Tangent (velocity) vector at `v1`, when the source supplies one.
    pub tangent1: Option<Vec3>,
}

/// A coarse lattice mesh (the "surface rep").
#[derive(Clone, Debug, PartialEq)]
pub struct LatticeMesh {
    /// Human-readable name (source X3D model / shell ordinal).
    pub name: String,
    /// Vertex positions.
    pub vertices: Vec<Vec3>,
    /// Vertices of each face, in order (each ring is a closed polygon).
    pub faces: Vec<Vec<u32>>,
    /// Edges with rounding/tangent data.
    pub edges: Vec<LatticeEdge>,
    /// Per-vertex rounding weights (default 1.0 when not supplied).
    pub vertex_round: Vec<f64>,
    /// Tessellation divisions hint from the source (informational).
    pub number_of_divisions: u32,
    /// Source `shellType` string (e.g. `LATTICE_MESH`, `POLYGON_MESH`).
    pub shell_type: String,
}

impl Default for LatticeEdge {
    fn default() -> Self {
        Self {
            v0: 0,
            v1: 0,
            round: 1.0,
            tangent0: None,
            tangent1: None,
        }
    }
}

impl LatticeMesh {
    /// Parse every `XvlShell` lattice mesh found in an X3D document.
    pub fn parse_xvl_document(xml: &str) -> Vec<LatticeMesh> {
        XvlShellParser::new(xml).parse_all()
    }

    /// Decode every face into Gregory patches.
    ///
    /// Quad faces yield one patch; larger/smaller faces fan into quads.
    pub fn decode_surfaces(&self) -> Vec<GregoryPatch> {
        let mut out = Vec::new();
        for face in &self.faces {
            out.extend(self.decode_face(face));
        }
        out
    }

    /// Number of patches a full decode yields (quad -> 1, n-gon -> n quads).
    pub fn decoded_patch_count(&self) -> usize {
        let quads: usize = self.faces.iter().filter(|f| f.len() == 4).count();
        let ngon_patches: usize = self
            .faces
            .iter()
            .filter(|f| f.len() != 4 && f.len() > 2)
            .map(|f| f.len())
            .sum();
        quads + ngon_patches
    }

    /// Decode a single face ring into one or more Gregory patches.
    pub fn decode_face(&self, ring: &[u32]) -> Vec<GregoryPatch> {
        match ring.len() {
            4 => vec![self.decode_quad([ring[0], ring[1], ring[2], ring[3]])],
            0..=2 => Vec::new(),
            _ => self.fan_n_gon(ring),
        }
    }

    /// Find the lattice edge joining `a` and `b` (either orientation).
    fn find_edge(&self, a: u32, b: u32) -> Option<&LatticeEdge> {
        self.edges
            .iter()
            .find(|e| (e.v0 == a && e.v1 == b) || (e.v0 == b && e.v1 == a))
    }

    /// Tangents for a quad side with lattice endpoints `a`, `b` (side runs
    /// `a` -> `b`): returns (t@a, t@b) scaled by the edge rounding weight.
    fn side_tangents(&self, a: u32, b: u32) -> (Vec3, Vec3) {
        let zero = Vec3::new(0.0, 0.0, 0.0);
        match self.find_edge(a, b) {
            Some(e) => {
                let ta = if e.v0 == a { e.tangent0 } else { e.tangent1 };
                let tb = if e.v1 == b { e.tangent1 } else { e.tangent0 };
                (
                    ta.map(|t| t * e.round).unwrap_or(zero),
                    tb.map(|t| t * e.round).unwrap_or(zero),
                )
            }
            None => (zero, zero),
        }
    }

    /// Decode one quad face (four lattice vertex indices) into a Gregory patch.
    pub fn decode_quad(&self, ring: [u32; 4]) -> GregoryPatch {
        let [a, b, c, d] = ring;
        let pa = self.vertices[a as usize];
        let pb = self.vertices[b as usize];
        let pc = self.vertices[c as usize];
        let pd = self.vertices[d as usize];
        // u along A->B, v along A->D; corners (0,0)=A (1,0)=B (1,1)=C (0,1)=D.
        let (ta0, tb0) = self.side_tangents(a, b); // v=0 edge A->B
        let (td1, tc1) = self.side_tangents(d, c); // v=1 edge D->C
        let (ta2, td2) = self.side_tangents(a, d); // u=0 edge A->D
        let (tb3, tc3) = self.side_tangents(b, c); // u=1 edge B->C
        let corners = [pa, pb, pc, pd];
        let edge_uv0 = [pa + ta0 / 3.0, pb - tb0 / 3.0];
        let edge_uv1 = [pd + td1 / 3.0, pc - tc1 / 3.0];
        let edge_vu0 = [pa + ta2 / 3.0, pd - td2 / 3.0];
        let edge_vu1 = [pb + tb3 / 3.0, pc - tc3 / 3.0];
        let grid = edges_to_grid(&corners, &edge_uv0, &edge_uv1, &edge_vu0, &edge_vu1);
        let interior = harmonic_interior(&grid);
        GregoryPatch::twist_free(corners, edge_uv0, edge_uv1, edge_vu0, edge_vu1, interior)
    }

    /// Fan-decode an n-gon face (n != 4) into quads around its centroid.
    fn fan_n_gon(&self, ring: &[u32]) -> Vec<GregoryPatch> {
        let n = ring.len();
        let pts: Vec<Vec3> = ring.iter().map(|&i| self.vertices[i as usize]).collect();
        let mut centroid = Vec3::new(0.0, 0.0, 0.0);
        for p in &pts {
            centroid += *p;
        }
        centroid = centroid / n as f64;
        let mut mids = Vec::with_capacity(n);
        for i in 0..n {
            mids.push((pts[i] + pts[(i + 1) % n]) / 2.0);
        }
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let prev = (i + n - 1) % n;
            // Sub-quad corners: v_i, mid_i, centroid, mid_prev.
            let a = pts[i];
            let b = mids[i];
            let c = centroid;
            let d = mids[prev];
            // The sub-quad's v=0 edge (a->b) starts along the original lattice
            // edge (v_i -> v_{i+1}); keep the lattice tangent at v_i.
            let (ta, _) = self.side_tangents(ring[i], ring[(i + 1) % n]);
            out.push(quad_patch_geometric(
                [a, b, c, d],
                [ta, Vec3::new(0.0, 0.0, 0.0)],
                [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)],
                [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)],
                [Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)],
            ));
        }
        out
    }
}

/// Build a Gregory patch from corner positions and per-side endpoint tangents.
///
/// Sides are ordered: `v=0`: A->B, `v=1`: D->C, `u=0`: A->D, `u=1`: B->C.
/// Each `[t0, t1]` pair holds the endpoint tangent vectors of that side,
/// already scaled by any rounding weight. A missing vector decodes to a
/// straight side.
pub fn quad_patch_geometric(
    corners: [Vec3; 4],
    side0: [Vec3; 2],
    side1: [Vec3; 2],
    side2: [Vec3; 2],
    side3: [Vec3; 2],
) -> GregoryPatch {
    let [a, b, c, d] = corners;
    let edge_uv0 = [a + side0[0] / 3.0, b - side0[1] / 3.0];
    let edge_uv1 = [d + side1[0] / 3.0, c - side1[1] / 3.0];
    let edge_vu0 = [a + side2[0] / 3.0, d - side2[1] / 3.0];
    let edge_vu1 = [b + side3[0] / 3.0, c - side3[1] / 3.0];
    let grid = edges_to_grid(&corners, &edge_uv0, &edge_uv1, &edge_vu0, &edge_vu1);
    let interior = harmonic_interior(&grid);
    GregoryPatch::twist_free(corners, edge_uv0, edge_uv1, edge_vu0, edge_vu1, interior)
}

/// Assemble the 4x4 boundary-given control grid (interior slots zeroed).
fn edges_to_grid(
    corners: &[Vec3; 4],
    edge_uv0: &[Vec3; 2],
    edge_uv1: &[Vec3; 2],
    edge_vu0: &[Vec3; 2],
    edge_vu1: &[Vec3; 2],
) -> [[Vec3; 4]; 4] {
    let zero = Vec3::new(0.0, 0.0, 0.0);
    let mut g = [[zero; 4]; 4];
    g[0][0] = corners[0];
    g[3][0] = corners[1];
    g[3][3] = corners[2];
    g[0][3] = corners[3];
    g[1][0] = edge_uv0[0];
    g[2][0] = edge_uv0[1];
    g[1][3] = edge_uv1[0];
    g[2][3] = edge_uv1[1];
    g[0][1] = edge_vu0[0];
    g[0][2] = edge_vu0[1];
    g[3][1] = edge_vu1[0];
    g[3][2] = edge_vu1[1];
    g
}

/// Interior twist control points via a harmonic-equilibrium fill: each
/// interior 2x2 control point equals the mean of its four grid neighbours
/// with the boundary fixed. Returns the interior as `[iu][iv]` (2x2).
fn harmonic_interior(g: &[[Vec3; 4]; 4]) -> [[Vec3; 2]; 2] {
    // Variables in order U00=(1,1), U10=(2,1), U01=(1,2), U11=(2,2)
    // (grid coordinates (i=u, j=v) in 0..=3).
    // Equation per unknown: 4*U - neighbours = boundary sum.
    let rhs = [
        g[0][1] + g[1][0],
        g[3][1] + g[2][0],
        g[0][2] + g[1][3],
        g[3][2] + g[2][3],
    ];
    let a = [
        [4.0, -1.0, -1.0, 0.0],
        [-1.0, 4.0, 0.0, -1.0],
        [-1.0, 0.0, 4.0, -1.0],
        [0.0, -1.0, -1.0, 4.0],
    ];
    let mut inv = [[0.0_f64; 4]; 4];
    invert4x4(&a, &mut inv);
    let mut sol = [Vec3::new(0.0, 0.0, 0.0); 4];
    for r in 0..4 {
        for k in 0..4 {
            sol[r] += rhs[k] * inv[r][k];
        }
    }
    // U00 -> [0][0], U10 -> [1][0], U01 -> [0][1], U11 -> [1][1].
    [[sol[0], sol[2]], [sol[1], sol[3]]]
}

/// Invert a 4x4 matrix in place into `inv` (Gauss-Jordan with pivoting).
fn invert4x4(m: &[[f64; 4]; 4], inv: &mut [[f64; 4]; 4]) {
    let mut a = *m;
    let mut b = [[0.0_f64; 4]; 4];
    for (i, row) in b.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    for col in 0..4 {
        let mut piv = col;
        for r in (col + 1)..4 {
            if a[r][col].abs() > a[piv][col].abs() {
                piv = r;
            }
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let d = a[col][col];
        for j in 0..4 {
            a[col][j] /= d;
            b[col][j] /= d;
        }
        for r in 0..4 {
            if r != col {
                let f = a[r][col];
                if f != 0.0 {
                    for j in 0..4 {
                        a[r][j] -= f * a[col][j];
                        b[r][j] -= f * b[col][j];
                    }
                }
            }
        }
    }
    *inv = b;
}

/// A tokenised XML-ish tag together with its byte offsets in the document.
struct Tag {
    name: String,
    attrs: Vec<(String, String)>,
    /// Byte index just past the tag's closing `>`.
    end: usize,
}

/// The `XvlShell`-style X3D parser (minimal, formatting-tolerant).
struct XvlShellParser<'a> {
    xml: &'a str,
}

impl<'a> XvlShellParser<'a> {
    fn new(xml: &'a str) -> Self {
        Self { xml }
    }

    /// Tokenize the document into tags (skips comments, PI, DOCTYPE).
    fn tags(&self) -> Vec<Tag> {
        let s = self.xml;
        let mut out = Vec::new();
        let mut i = 0usize;
        while i < s.len() {
            if s[i..].starts_with("<!--") {
                i += 4;
                while i < s.len() && !s[i..].starts_with("-->") {
                    i += 1;
                }
                i = (i + 3).min(s.len());
            } else if s[i..].starts_with("<?") {
                match find_tag_end(s, i) {
                    Some(e) => i = e + 1,
                    None => break,
                }
            } else if s[i..].starts_with('<') {
                match find_tag_end(s, i) {
                    Some(e) => {
                        let inner = &s[i + 1..e];
                        if let Some(mut tag) = parse_tag(inner) {
                            tag.end = e + 1;
                            out.push(tag);
                        }
                        i = e + 1;
                    }
                    None => break,
                }
            } else {
                i += 1;
            }
        }
        out
    }

    fn parse_all(&self) -> Vec<LatticeMesh> {
        let tags = self.tags();
        // Global DEF -> point list register for <Coordinate> nodes.
        let mut coord_def: HashMap<String, Vec<f64>> = HashMap::new();
        for t in &tags {
            if t.name == "Coordinate" {
                if let (Some(def), Some(pt)) = (attr(&t.attrs, "DEF"), attr(&t.attrs, "point")) {
                    coord_def.insert(def.to_string(), parse_numbers(pt));
                }
            }
        }

        let mut meshes = Vec::new();
        for t in &tags {
            if t.name != "XvlShell" {
                continue;
            }
            let name = attr(&t.attrs, "DEF")
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("shell#{}", meshes.len() + 1));
            // Children (Coordinate) live between this start tag and the next
            // `</XvlShell>`; collect them from the raw substring.
            let rest = &self.xml[t.end..];
            let close = rest
                .find("</XvlShell>")
                .map(|k| t.end + k)
                .unwrap_or(self.xml.len());
            let body = &self.xml[t.end..close];
            let mut vertices = Vec::new();
            for coord in Self::coordinates_in(body) {
                resolve_coordinate(&coord, &coord_def, &mut vertices);
            }
            meshes.push(self.build_mesh(&name, &t.attrs, vertices));
        }
        meshes
    }

    /// Find `<Coordinate .../>` tags inside a fragment of X3D markup.
    fn coordinates_in(body: &str) -> Vec<Tag> {
        let mut out = Vec::new();
        let mut i = 0usize;
        while i < body.len() {
            if body[i..].starts_with("<!--") {
                i += 4;
                while i < body.len() && !body[i..].starts_with("-->") {
                    i += 1;
                }
                i = (i + 3).min(body.len());
                continue;
            }
            match find_tag_end(body, i) {
                Some(e) => {
                    if body[i..e + 1].starts_with("<Coordinate") {
                        if let Some(mut tag) = parse_tag(&body[i + 1..e]) {
                            tag.end = e + 1;
                            out.push(tag);
                        }
                    }
                    i = e + 1;
                }
                None => i += 1,
            }
        }
        out
    }

    fn build_mesh(
        &self,
        name: &str,
        attrs: &[(String, String)],
        vertices: Vec<Vec3>,
    ) -> LatticeMesh {
        let face_idx = attr(attrs, "faceCoordIndex")
            .map(parse_ints)
            .unwrap_or_default();
        let faces = split_faces(&face_idx);
        let edge_begin = attr(attrs, "edgeBeginCoordIndex")
            .map(parse_ints)
            .unwrap_or_default();
        let edge_end = attr(attrs, "edgeEndCoordIndex")
            .map(parse_ints)
            .unwrap_or_default();
        let edge_round = attr(attrs, "edgeRound")
            .map(parse_numbers)
            .unwrap_or_default();
        let begin_vec = attr(attrs, "edgeBeginVector")
            .map(parse_numbers)
            .unwrap_or_default();
        let end_vec = attr(attrs, "edgeEndVector")
            .map(parse_numbers)
            .unwrap_or_default();
        let vertex_round_raw = attr(attrs, "vertexRound")
            .map(parse_numbers)
            .unwrap_or_default();

        let n_edges = edge_begin.len().max(edge_end.len());
        let mut edges = Vec::with_capacity(n_edges);
        for k in 0..n_edges {
            let v0 = *edge_begin.get(k).unwrap_or(&0) as u32;
            let v1 = *edge_end.get(k).unwrap_or(&0) as u32;
            let round = edge_round.get(k).copied().unwrap_or(1.0);
            let tangent0 = begin_vec
                .get(3 * k..3 * k + 3)
                .map(|s| Vec3::new(s[0], s[1], s[2]));
            let tangent1 = end_vec
                .get(3 * k..3 * k + 3)
                .map(|s| Vec3::new(s[0], s[1], s[2]));
            edges.push(LatticeEdge {
                v0,
                v1,
                round,
                tangent0,
                tangent1,
            });
        }

        let nv = vertices.len();
        let vertex_round: Vec<f64> = (0..nv)
            .map(|i| vertex_round_raw.get(i).copied().unwrap_or(1.0))
            .collect();

        let divisions = attr(attrs, "numberOfDivisions")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .unwrap_or(4);
        let shell_type = attr(attrs, "shellType").unwrap_or_default().to_string();

        LatticeMesh {
            name: name.to_string(),
            vertices,
            faces,
            edges,
            vertex_round,
            number_of_divisions: divisions,
            shell_type,
        }
    }
}

/// Resolve a `<Coordinate>` into vertex positions, handling DEF/USE.
fn resolve_coordinate(t: &Tag, coord_def: &HashMap<String, Vec<f64>>, out: &mut Vec<Vec3>) {
    if let Some(use_name) = attr(&t.attrs, "USE") {
        if let Some(pt) = coord_def.get(use_name) {
            out.extend(chunks3(pt));
        }
        return;
    }
    if let Some(pt) = attr(&t.attrs, "point") {
        out.extend(chunks3(&parse_numbers(pt)));
    }
}

fn chunks3(v: &[f64]) -> Vec<Vec3> {
    v.chunks(3)
        .filter(|c| c.len() == 3)
        .map(|c| Vec3::new(c[0], c[1], c[2]))
        .collect()
}

/// Find the byte index just past the closing `>` of the tag starting at `i`,
/// respecting single/double quotes inside the tag.
fn find_tag_end(s: &str, i: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if i >= bytes.len() {
        return None;
    }
    if bytes[i] != b'<' {
        return None;
    }
    let mut quote = 0u8;
    let mut j = i + 1;
    while j < bytes.len() {
        let b = bytes[j];
        if quote == 0 {
            if b == b'\'' || b == b'"' {
                quote = b;
            } else if b == b'>' {
                return Some(j);
            }
        } else if b == quote {
            quote = 0;
        }
        j += 1;
    }
    None
}

/// Parse a tag body (without `<` `>`) into a name + attribute list.
fn parse_tag(inner: &str) -> Option<Tag> {
    let inner = inner.trim_end();
    if inner.is_empty() {
        return None;
    }
    let mut name = String::new();
    let mut rest_start = 0usize;
    for (i, ch) in inner.char_indices() {
        if ch.is_whitespace() {
            rest_start = i;
            break;
        }
        name.push(ch);
    }
    if name.is_empty() {
        return None;
    }
    if name.ends_with('/') {
        name.pop();
    }
    let rest = inner[rest_start..].trim_start();
    let mut attrs = Vec::new();
    let rbytes = rest.as_bytes();
    let mut pos = 0usize;
    while pos < rbytes.len() {
        while pos < rbytes.len() && rbytes[pos].is_ascii_whitespace() {
            pos += 1;
        }
        if pos >= rbytes.len() || rbytes[pos] == b'/' {
            break;
        }
        let mut aname = String::new();
        while pos < rbytes.len()
            && !rbytes[pos].is_ascii_whitespace()
            && rbytes[pos] != b'='
            && rbytes[pos] != b'/'
        {
            aname.push(rbytes[pos] as char);
            pos += 1;
        }
        while pos < rbytes.len() && (rbytes[pos].is_ascii_whitespace() || rbytes[pos] == b'=') {
            pos += 1;
        }
        let mut avalue = String::new();
        if pos < rbytes.len() && (rbytes[pos] == b'\'' || rbytes[pos] == b'"') {
            let q = rbytes[pos];
            pos += 1;
            while pos < rbytes.len() && rbytes[pos] != q {
                avalue.push(rbytes[pos] as char);
                pos += 1;
            }
            if pos < rbytes.len() {
                pos += 1;
            }
        } else {
            while pos < rbytes.len() && !rbytes[pos].is_ascii_whitespace() && rbytes[pos] != b'/' {
                avalue.push(rbytes[pos] as char);
                pos += 1;
            }
        }
        if !aname.is_empty() {
            attrs.push((aname, avalue));
        }
    }
    Some(Tag {
        name,
        attrs,
        end: 0,
    })
}

/// Look up an attribute by name (case-insensitive).
fn attr<'s>(attrs: &'s [(String, String)], name: &str) -> Option<&'s str> {
    attrs
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Split a `-1`-terminated face index stream into vertex rings.
fn split_faces(idx: &[i64]) -> Vec<Vec<u32>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    for &v in idx {
        if v == -1 {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else if v >= 0 {
            cur.push(v as u32);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Parse an integer MFInt32 list (whitespace and/or comma separated).
fn parse_ints(v: &str) -> Vec<i64> {
    v.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<i64>().ok())
        .collect()
}

/// Parse a float MFFloat list (whitespace and/or comma separated).
fn parse_numbers(v: &str) -> Vec<f64> {
    v.split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse::<f64>().ok())
        .collect()
}
