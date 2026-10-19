//! P3-M4-T01 — writer: internal mesh -> 3DXML ASCII.
//!
//! Emits the tessellated-ASCII `.3DRep` branch of 3DXML (the open-tooling
//! branch) from an indexed triangle mesh, as well as a complete ZIP-archive
//! `.3dxml` package, for internal use.
//!
//! ## Clean-room / license posture (license-compliant, internal-use spec)
//!
//! The markup emitted here is our own bytes in a conformant interchange
//! shell. The 3DXML format facts (element spine, attribute encodings,
//! `urn:3DXML:` wiring) come ONLY from observed bytes of public
//! MIT/Apache-2.0 sample files — recorded in
//! `docs/re-notes/3dxml-hex-notebook.md` (P1-M2-T04, section 4) — and from
//! the Dassault 3DXML specification held privately under its free
//! internal-use license (P1-M1-T02). Nothing from that licensed spec is
//! reproduced here or anywhere in this crate; the ZIP framing follows the
//! public PKWARE APPNOTE. No vendor library reads or writes for us. Format
//! version strings, line endings, and the minimal product structure are
//! writer choices, marked `[uncertain]` in the re-note, not requirements.
//!
//! ## API shape
//!
//! Both entry points consume the internal mesh as PLAIN SLICES —
//! `positions: &[[f32; 3]]` + `triangles: &[[u32; 3]]` — so this module is
//! independent of the concrete `Mesh` type (docs/layout.md: no cross-crate
//! path deps until the P6 importer hub) while being trivially callable from
//! `mesh.vertices()` / `mesh.triangles()`.
//!
//! * [`write_rep_markup`] — the core conversion: mesh -> ASCII
//!   `XMLRepresentation` `.3DRep` markup (one 3-index `strips` run per
//!   triangle; a triangle strip of 3 indices decodes to exactly one
//!   triangle, so the mesh round-trips losslessly). No normals are emitted:
//!   an indexed triangle mesh carries none and we do not invent data.
//! * [`write_package`] — wraps the rep into a deterministic ZIP-archive
//!   3DXML: `Manifest.xml` (whose `<Root>` names the model doc) + `<name>
//!   .3dxml` (Header + ProductStructure + `ReferenceRep format="TESSELLATED"`
//!   `associatedFile="urn:3DXML:<name>.3DRep"`) + `<name>.3DRep` (the ASCII
//!   rep). Three STORED members, fixed DOS timestamp, UTF-8 names — bytes
//!   are identical for a given input, which is what the golden-file test
//!   locks to.
//!
//! Validation runs BEFORE any byte is written, so a failed call emits
//! nothing. Errors are typed and never panic.

use std::fmt;
use std::io::Write;

/// Repeated observed value of the 3DXML XML namespace.
const SCHEMA_3DXML: &str = "http://www.3ds.com/xsd/3DXML";
/// The XML Schema Instance namespace (used for the `xsi:type` tags).
const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// Threshold below which a triangle is treated as zero-area (degenerate).
const DEGENERATE_EPS: f32 = 1e-12;

/// Writer options controlling the deterministic byte output.
#[derive(Debug, Clone)]
pub struct WriterOpts<'a> {
    /// Base name used for the model-doc member (`<name>.3dxml`), the rep
    /// member (`<name>.3DRep`), the Manifest `<Root>`, the `urn:3DXML:`
    /// associated file, and the model-doc Reference3D/ReferenceRep names.
    /// Must be a plain file-name-safe string for internal use.
    pub product_name: &'a str,
    /// The `<Author>` value recorded in the model-doc Header.
    pub author: &'a str,
    /// The `<Created>` value recorded in the model-doc Header. An empty
    /// string omits the element entirely (used by the golden test for
    /// byte-determinism).
    pub timestamp: &'a str,
}

/// Errors produced by the 3DXML ASCII writer.
#[derive(Debug)]
pub enum WriteError {
    /// The mesh has no vertex positions or no triangles.
    EmptyMesh,
    /// A triangle index referenced a vertex position that does not exist.
    IndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The number of positions in the mesh.
        len: usize,
    },
    /// A vertex position is NaN or infinite (index into `positions`).
    NonFiniteVertex(usize),
    /// A triangle has zero area and would emit a meaningless facet.
    DegenerateTriangle,
    /// Underlying IO failure while writing.
    Io(std::io::Error),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::EmptyMesh => write!(f, "mesh is empty (no positions or no triangles)"),
            WriteError::IndexOutOfRange { index, len } => {
                write!(f, "triangle index {index} out of range (positions: {len})")
            }
            WriteError::NonFiniteVertex(i) => write!(f, "vertex {i} not finite"),
            WriteError::DegenerateTriangle => write!(f, "degenerate triangle (zero area)"),
            WriteError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for WriteError {}

impl From<std::io::Error> for WriteError {
    fn from(e: std::io::Error) -> Self {
        WriteError::Io(e)
    }
}

/// Validate the mesh up front: non-empty, in-range indices, finite vertex
/// positions, no zero-area triangles. Returns the first failure; a failed
/// call emits no output.
fn validate(positions: &[[f32; 3]], triangles: &[[u32; 3]]) -> Result<(), WriteError> {
    if positions.is_empty() || triangles.is_empty() {
        return Err(WriteError::EmptyMesh);
    }
    for tri in triangles {
        for &i in tri {
            if i as usize >= positions.len() {
                return Err(WriteError::IndexOutOfRange {
                    index: i,
                    len: positions.len(),
                });
            }
        }
    }
    for (vi, v) in positions.iter().enumerate() {
        if !v.iter().all(|x| x.is_finite()) {
            return Err(WriteError::NonFiniteVertex(vi));
        }
    }
    for tri in triangles {
        let (a, b, c) = (
            positions[tri[0] as usize],
            positions[tri[1] as usize],
            positions[tri[2] as usize],
        );
        let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let cx = u[1] * v[2] - u[2] * v[1];
        let cy = u[2] * v[0] - u[0] * v[2];
        let cz = u[0] * v[1] - u[1] * v[0];
        let mag = (cx * cx + cy * cy + cz * cz).sqrt();
        if mag <= DEGENERATE_EPS {
            return Err(WriteError::DegenerateTriangle);
        }
    }
    Ok(())
}

/// Escape `&`, `<`, `>` for XML text and attribute content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// The `comma-separated 'x y z' triples` encoding of the vertex buffer
/// (hex notebook §4.2: no count header; the reader counts).
fn positions_text(positions: &[[f32; 3]]) -> String {
    positions
        .iter()
        .map(|p| format!("{} {} {}", p[0], p[1], p[2]))
        .collect::<Vec<_>>()
        .join(",")
}

/// The `comma-separated, space-separated index runs` encoding of the faces;
/// one 3-index strip-run per triangle (a 3-index strip == one triangle).
fn strips_text(triangles: &[[u32; 3]]) -> String {
    triangles
        .iter()
        .map(|t| format!("{} {} {}", t[0], t[1], t[2]))
        .collect::<Vec<_>>()
        .join(",")
}

fn write_rep_body<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    w: &mut W,
) -> std::io::Result<()> {
    writeln!(w, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
    writeln!(
        w,
        "<XMLRepresentation version=\"1.0\" xmlns=\"{SCHEMA_3DXML}\" xmlns:xsi=\"{XSI}\">"
    )?;
    writeln!(w, "  <Root xsi:type=\"BagRepType\">")?;
    writeln!(w, "    <Rep xsi:type=\"BagRepType\">")?;
    writeln!(w, "      <Rep xsi:type=\"PolygonalRepType\">")?;
    writeln!(
        w,
        "        <Faces><Face strips=\"{}\"/></Faces>",
        strips_text(triangles)
    )?;
    writeln!(
        w,
        "        <VertexBuffer><Positions>{}</Positions></VertexBuffer>",
        positions_text(positions)
    )?;
    writeln!(w, "      </Rep>")?;
    writeln!(w, "    </Rep>")?;
    writeln!(w, "  </Root>")?;
    writeln!(w, "</XMLRepresentation>")
}

fn write_model_doc<W: Write>(opts: &WriterOpts<'_>, w: &mut W) -> std::io::Result<()> {
    let name = xml_escape(opts.product_name);
    let author = xml_escape(opts.author);
    let title = xml_escape(opts.product_name);
    writeln!(w, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
    writeln!(
        w,
        "<Model_3dxml xmlns=\"{SCHEMA_3DXML}\" xmlns:xsi=\"{XSI}\">"
    )?;
    writeln!(w, "  <Header>")?;
    writeln!(w, "    <SchemaVersion>4.0</SchemaVersion>")?;
    writeln!(w, "    <Title>{title}</Title>")?;
    writeln!(w, "    <Author>{author}</Author>")?;
    writeln!(w, "    <Generator>Keelworks oep-3dxml</Generator>")?;
    if !opts.timestamp.is_empty() {
        writeln!(w, "    <Created>{}</Created>", xml_escape(opts.timestamp))?;
    }
    writeln!(w, "  </Header>")?;
    writeln!(w, "  <ProductStructure root=\"1\">")?;
    writeln!(w, "    <Reference3D id=\"1\" name=\"{name}\"/>")?;
    writeln!(w, "    <Instance3D id=\"11\" name=\"Instance1\">")?;
    writeln!(w, "      <IsAggregatedBy/>")?;
    writeln!(
        w,
        "      <IsInstanceOf xsi:type=\"InstanceOfReferenceType\" aggregatedById=\"1\"/>"
    )?;
    writeln!(
        w,
        "      <RelativeMatrix>1 0 0 0 0 1 0 0 0 0 1 0</RelativeMatrix>"
    )?;
    writeln!(w, "    </Instance3D>")?;
    writeln!(
        w,
        "    <ReferenceRep id=\"21\" name=\"{name}\" format=\"TESSELLATED\" version=\"1.0\" associatedFile=\"urn:3DXML:{name}.3DRep\"/>"
    )?;
    writeln!(w, "  </ProductStructure>")?;
    writeln!(w, "</Model_3dxml>")
}

fn write_manifest<W: Write>(opts: &WriterOpts<'_>, w: &mut W) -> std::io::Result<()> {
    writeln!(w, "<?xml version=\"1.0\" encoding=\"utf-8\" ?>")?;
    writeln!(
        w,
        "<Manifest xmlns:xsi=\"{XSI}\" xsi:noNamespaceSchemaLocation=\"Manifest.xsd\"><Root><![CDATA[{}.3dxml]]></Root></Manifest>",
        opts.product_name
    )
}

/// Write the tessellated-ASCII `.3DRep` markup for an indexed triangle mesh.
///
/// Emits the observed ASCII `XMLRepresentation` spine (hex notebook §4.2):
/// XML declaration, `XMLRepresentation version="1.0"`, a BagRep ->
/// PolygonalRep hierarchy, one `<Face strips="...">` (a comma-separated list
/// of 3-index triangle runs) and a `<VertexBuffer><Positions>` holding every
/// vertex as a comma-separated `x y z` triple. Geometry round-trips
/// losslessly. Deterministic for a given mesh; LF line endings and the
/// `version="1.0"` are writer choices `[uncertain]`, not requirements.
pub fn write_rep_markup<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    _opts: &WriterOpts<'_>,
    w: &mut W,
) -> Result<(), WriteError> {
    validate(positions, triangles)?;
    write_rep_body(positions, triangles, w)?;
    Ok(())
}

/// PKWARE reflected CRC-32 (poly 0xEDB88320), as used by ZIP.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn le_u16(w: &mut Vec<u8>, v: u16) {
    w.extend_from_slice(&v.to_le_bytes());
}

fn le_u32(w: &mut Vec<u8>, v: u32) {
    w.extend_from_slice(&v.to_le_bytes());
}

/// Local file header (PKWARE APPNOTE) for a STORED member.
fn zip_local_header(name: &[u8], crc: u32, size: u32) -> Vec<u8> {
    let mut h = Vec::with_capacity(30 + name.len());
    h.extend_from_slice(b"PK\x03\x04");
    le_u16(&mut h, 20); // version needed
    le_u16(&mut h, 0); // flags
    le_u16(&mut h, 0); // method: STORED
    le_u16(&mut h, 0); // mod time (1980-01-01 00:00:00)
    le_u16(&mut h, 0x0021); // mod date (1980-01-01)
    le_u32(&mut h, crc);
    le_u32(&mut h, size);
    le_u32(&mut h, size);
    le_u16(&mut h, name.len() as u16);
    le_u16(&mut h, 0); // extra len
    h.extend_from_slice(name);
    h
}

/// Central directory entry for a STORED member.
fn zip_central_entry(name: &[u8], crc: u32, size: u32, offset: u32) -> Vec<u8> {
    let mut h = Vec::with_capacity(46 + name.len());
    h.extend_from_slice(b"PK\x01\x02");
    le_u16(&mut h, 20); // version made by
    le_u16(&mut h, 20); // version needed
    le_u16(&mut h, 0); // flags
    le_u16(&mut h, 0); // method: STORED
    le_u16(&mut h, 0); // mod time
    le_u16(&mut h, 0x0021); // mod date
    le_u32(&mut h, crc);
    le_u32(&mut h, size);
    le_u32(&mut h, size);
    le_u16(&mut h, name.len() as u16);
    le_u16(&mut h, 0); // extra len
    le_u16(&mut h, 0); // comment len
    le_u16(&mut h, 0); // disk start
    le_u16(&mut h, 0); // internal attrs
    le_u32(&mut h, 0); // external attrs
    le_u32(&mut h, offset);
    h.extend_from_slice(name);
    h
}

/// Write a complete, deterministic ZIP-archive 3DXML package.
///
/// Exactly three STORED members, in order: `Manifest.xml` (whose `<Root>`
/// names the model doc), `<name>.3dxml` (Header + ProductStructure +
/// `ReferenceRep format="TESSELLATED" version="1.0"`
/// `associatedFile="urn:3DXML:<name>.3DRep"`), and `<name>.3DRep` (the
/// ASCII rep from [`write_rep_markup`]). Fixed DOS timestamp, UTF-8 member
/// names, PKWARE CRC-32 — byte-identical for a given input (golden-locked).
pub fn write_package<W: Write>(
    positions: &[[f32; 3]],
    triangles: &[[u32; 3]],
    opts: &WriterOpts<'_>,
    w: &mut W,
) -> Result<(), WriteError> {
    validate(positions, triangles)?;

    let mut rep = Vec::new();
    write_rep_body(positions, triangles, &mut rep)?;
    let mut model = Vec::new();
    write_model_doc(opts, &mut model)?;
    let mut manifest = Vec::new();
    write_manifest(opts, &mut manifest)?;

    let member_names: [Vec<u8>; 3] = [
        b"Manifest.xml".to_vec(),
        format!("{}.3dxml", opts.product_name).into_bytes(),
        format!("{}.3DRep", opts.product_name).into_bytes(),
    ];
    let member_data: [Vec<u8>; 3] = [manifest, model, rep];

    // Local headers + data first (offsets from the file start).
    let mut local_area = Vec::new();
    let mut offsets = [0u32; 3];
    let mut crcs = [0u32; 3];
    let mut sizes = [0u32; 3];
    for k in 0..3 {
        offsets[k] = local_area.len() as u32;
        let crc = crc32(&member_data[k]);
        let size = member_data[k].len() as u32;
        crcs[k] = crc;
        sizes[k] = size;
        local_area.extend_from_slice(&zip_local_header(&member_names[k], crc, size));
        local_area.extend_from_slice(&member_data[k]);
    }

    // Central directory + EOCD.
    let mut central = Vec::new();
    for k in 0..3 {
        central.extend_from_slice(&zip_central_entry(
            &member_names[k],
            crcs[k],
            sizes[k],
            offsets[k],
        ));
    }
    let cd_size = central.len() as u32;
    let cd_offset = local_area.len() as u32;

    w.write_all(&local_area)?;
    w.write_all(&central)?;
    let mut eocd = Vec::with_capacity(22);
    eocd.extend_from_slice(b"PK\x05\x06");
    le_u16(&mut eocd, 0); // disk number
    le_u16(&mut eocd, 0); // central dir start disk
    le_u16(&mut eocd, 3); // entries on this disk
    le_u16(&mut eocd, 3); // total entries
    le_u32(&mut eocd, cd_size);
    le_u32(&mut eocd, cd_offset);
    le_u16(&mut eocd, 0); // comment len
    w.write_all(&eocd)?;
    Ok(())
}
