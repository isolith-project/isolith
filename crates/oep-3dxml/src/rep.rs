//! P3-M1-T02 — parse the `<Model_3dxml>` representation: nodes, references,
//! instances.
//!
//! The primary model document of a 3DXML container (named by `Manifest.xml`
//! `<Root>`, or a standalone `.3dxml`) is a full XML document whose
//! `<ProductStructure>` tree is the assembly graph:
//!
//! - **nodes** — `<Reference3D>` (a part/product definition; id + name);
//! - **references** — `<ReferenceRep>` (a representation reference: which
//!   `.3DRep` geometry member a node draws from, via the `associatedFile`
//!   URN `urn:3DXML:<member>.3DRep`);
//! - **instances** — `<Instance3D>` (a placement of a `Reference3D` inside an
//!   assembly: `<IsAggregatedBy>` = owner node id, `<IsInstanceOf>` = node id,
//!   `<RelativeMatrix>` = 12 doubles, row-major 4x3 transform).
//!
//! Rep wiring is closed by `<InstanceRep>` (an instance of a `ReferenceRep`),
//! matching the observed corpus shape (hex notebook §4.1, §4.4). Namespaces,
//! CRLF/LF line endings, single-line vs pretty-printed documents, and
//! writer-specific metadata children (`<PLM_ExternalID>`, `<V_discipline>`,
//! ...) are all tolerated; unknown elements are skipped (P2-M1-T04 tolerant
//! failure model). Any structural violation yields a typed [`RepError`],
//! never a panic (fuzz-friendly, cf. P2-M1-T05).
//!
//! Clean-room: all element names asserted here come from the public
//! `3DXML.xsd` vocabulary observed on the license-clean corpus (P1-M1-T02)
//! and the P1-M2-T04 hex notebook — no vendor documentation is quoted.

use std::io::Cursor;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Parsed `<Model_3dxml>` document: header + product structure.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModelRep {
    /// Container `<Header>` (schema version, provenance fields).
    pub header: Header,
    /// `<ProductStructure>`: the assembly graph of nodes/references/instances.
    pub structure: ProductStructure,
}

/// Container `<Header>` metadata (all optional; only `<SchemaVersion>` is
/// present in every observed corpus document).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Header {
    /// `<SchemaVersion>` (observed `4.0`/`4.2`/`4.3`).
    pub schema_version: Option<String>,
    /// `<Title>`.
    pub title: Option<String>,
    /// `<Author>`.
    pub author: Option<String>,
    /// `<Generator>` writer fingerprint (e.g. `Abaqus/CAE`, `CATIA V5`).
    pub generator: Option<String>,
    /// `<Created>` ISO date string.
    pub created: Option<String>,
}

/// `<ProductStructure>`: the assembly graph.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProductStructure {
    /// `root` attribute: id of the root `Reference3D` node.
    pub root_id: Option<String>,
    /// `<Reference3D>` nodes (products / parts).
    pub nodes: Vec<Reference3D>,
    /// `<ReferenceRep>` rep references (geometry member wiring).
    pub references: Vec<ReferenceRep>,
    /// `<Instance3D>` placements.
    pub instances: Vec<Instance3D>,
    /// `<InstanceRep>` rep-instance wiring.
    pub instance_reps: Vec<InstanceRep>,
}

/// A `<Reference3D>` node: a part/product definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference3D {
    /// `id` attribute (integer string; unique within the document).
    pub id: String,
    /// `name` attribute.
    pub name: String,
}

/// A `<ReferenceRep>` reference: pulls geometry from a `.3DRep` member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceRep {
    /// `id` attribute (integer string).
    pub id: String,
    /// `name` attribute (optional; SOLIDWORKS writer omits it).
    pub name: Option<String>,
    /// `format` attribute (observed `TESSELLATED` for ASCII reps, `UVR` for
    /// the binary V5_CFV3 branch).
    pub format: Option<String>,
    /// `version` attribute (rep XML version, e.g. `1.1`, `1.2`).
    pub version: Option<String>,
    /// `associatedFile` attribute, full URN (`urn:3DXML:...3DRep`).
    pub associated_file: Option<String>,
}

impl ReferenceRep {
    /// Member name parsed off the `associatedFile` URN (strips the
    /// `urn:3DXML:` prefix): the `.3DRep` member this rep draws from.
    /// `None` when absent or not a `urn:3DXML:` reference.
    pub fn member_name(&self) -> Option<String> {
        const PREFIX: &str = "urn:3DXML:";
        self.associated_file
            .as_deref()
            .and_then(|u| u.strip_prefix(PREFIX))
            .map(|m| m.to_string())
    }
}

/// An `<Instance3D>` placement of a node inside an assembly.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance3D {
    /// `id` attribute (integer string).
    pub id: String,
    /// `name` attribute.
    pub name: String,
    /// `<IsAggregatedBy>`: id of the owning `Reference3D` node.
    pub is_aggregated_by: Option<String>,
    /// `<IsInstanceOf>`: id of the instantiated `Reference3D` node.
    pub is_instance_of: Option<String>,
    /// `<RelativeMatrix>`: 12 doubles (row-major 4x3 transform), when
    /// present.
    pub relative_matrix: Option<Vec<f64>>,
}

/// An `<InstanceRep>`: instance of a rep reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceRep {
    /// `id` attribute (integer string).
    pub id: String,
    /// `name` attribute (optional).
    pub name: Option<String>,
    /// `<IsAggregatedBy>`: id of the owning `Reference3D` node.
    pub is_aggregated_by: Option<String>,
    /// `<IsInstanceOf>`: id of the referenced `ReferenceRep`.
    pub is_instance_of: Option<String>,
}

/// Typed parse errors from [`parse_model`]. Non-panicking by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepError {
    /// Input is not valid UTF-8 where a string was required.
    InvalidUtf8,
    /// XML is structurally malformed, truncated, or not XML at all.
    Xml {
        /// Diagnostic from the XML reader.
        message: String,
    },
    /// A `<RelativeMatrix>` does not contain exactly 12 doubles.
    Matrix {
        /// (found, expected) element counts.
        counts: (usize, usize),
    },
}

impl From<quick_xml::Error> for RepError {
    fn from(e: quick_xml::Error) -> Self {
        RepError::Xml {
            message: e.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Streaming event-driven parser (quick-xml)
// ---------------------------------------------------------------------------

/// Parse a `<Model_3dxml>` model document into its typed rep model.
///
/// Tolerant on unknown elements/writer metadata; typed [`RepError`] on
/// malformed input; never panics.
pub fn parse_model(bytes: &[u8]) -> Result<ModelRep, RepError> {
    let mut reader = Reader::from_reader(Cursor::new(bytes));
    reader.config_mut().trim_text(true);

    let mut model = ModelRep::default();
    // True once a `<Model_3dxml>` root is seen; a bare prose blob with no
    // element at all must fail as malformed, not silently return an empty
    // model.
    let mut saw_root = false;
    // Current open container: which typed element we are inside.
    let mut in_header = false;
    let mut in_product_structure = false;
    // Active record builders (one at a time; records are flat siblings).
    let mut builder: Option<RecordBuilder> = None;
    // Direct-text child of the active builder/header we are currently inside
    // (e.g. `<SchemaVersion>`, `<IsAggregatedBy>`, `<RelativeMatrix>`).
    let mut leaf: Option<&'static str> = None;
    // Reusable buffer (quick-xml 0.39 buffered reader API).
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf)? {
            Event::Start(e) => {
                let name = local(&e);
                match name.as_str() {
                    "Model_3dxml" => saw_root = true,
                    "Header" => in_header = true,
                    "ProductStructure" => {
                        in_product_structure = true;
                        model.structure.root_id = attr(&e, "root");
                    }
                    // Recurse into known record elements only while inside
                    // ProductStructure; tolerate stray occurrences elsewhere.
                    _ if in_product_structure && RECORD_ELEMENTS.contains(&name.as_str()) => {
                        builder = Some(RecordBuilder::start(&e));
                    }
                    // Direct-text children of whatever is open.
                    _ if in_header || builder.is_some() => {
                        leaf = leaf_name(&name);
                    }
                    _ => {} // unknown element -> skip (tolerant)
                }
            }
            Event::Empty(e) => {
                let name = local(&e);
                if name == "Model_3dxml" {
                    saw_root = true;
                }
                if in_product_structure && RECORD_ELEMENTS.contains(&name.as_str()) {
                    // Self-closing record: finalize immediately.
                    let rec = RecordBuilder::start(&e).finish()?;
                    push_record(&mut model.structure, &name, rec);
                }
                leaf = None;
            }
            Event::Text(t) => {
                // Feed while keeping `leaf` set: a single XML child may be
                // delivered across several text runs; we clear it on End.
                if let Some(l) = leaf {
                    let text = t.decode().map_err(|_| RepError::InvalidUtf8)?;
                    if let Some(b) = &mut builder {
                        b.feed_text(l, text.as_ref());
                    } else if in_header {
                        feed_header(&mut model.header, l, text.as_ref());
                    }
                }
            }
            Event::End(e) => {
                let name = local_end(&e);
                match name.as_str() {
                    "Header" => in_header = false,
                    "ProductStructure" => in_product_structure = false,
                    _ if builder.is_some() && RECORD_ELEMENTS.contains(&name.as_str()) => {
                        let rec = builder.take().expect("builder present").finish()?;
                        push_record(&mut model.structure, &name, rec);
                    }
                    _ => {
                        // Closing a leaf element: drop the pending leaf name.
                        if in_header || builder.is_some() {
                            leaf = None;
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if !saw_root {
        return Err(RepError::Xml {
            message: "no <Model_3dxml> root element found".to_string(),
        });
    }
    Ok(model)
}

/// Map a leaf element's local name to the names we actually parse; anything
/// else (writer metadata) returns `None` so its text is ignored (tolerant).
fn leaf_name(name: &str) -> Option<&'static str> {
    match name {
        "SchemaVersion" => Some("SchemaVersion"),
        "Title" => Some("Title"),
        "Author" => Some("Author"),
        "Generator" => Some("Generator"),
        "Created" => Some("Created"),
        "IsAggregatedBy" => Some("IsAggregatedBy"),
        "IsInstanceOf" => Some("IsInstanceOf"),
        "RelativeMatrix" => Some("RelativeMatrix"),
        _ => None,
    }
}

/// Element names we build typed records for.
const RECORD_ELEMENTS: [&str; 4] = ["Reference3D", "ReferenceRep", "Instance3D", "InstanceRep"];

/// A `<Model_3dxml>` element's local name from a start/empty tag.
fn local(e: &BytesStart<'_>) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

/// Local name of a closing tag.
fn local_end(e: &quick_xml::events::BytesEnd<'_>) -> String {
    String::from_utf8_lossy(e.name().local_name().as_ref()).into_owned()
}

/// Read an attribute by local name (namespace-stripped), UTF-8 + unescape.
fn attr(e: &BytesStart<'_>, want: &str) -> Option<String> {
    for a in e.attributes().flatten() {
        if a.key.local_name().as_ref() == want.as_bytes() {
            return a
                .normalized_value(quick_xml::XmlVersion::Explicit1_0)
                .ok()
                .map(|c| c.into_owned());
        }
    }
    None
}

/// In-progress record while streaming a `Reference3D`/`ReferenceRep`/
/// `Instance3D`/`InstanceRep`.
#[derive(Debug)]
enum RecordBuilder {
    Node {
        id: String,
        name: String,
    },
    RefRep {
        id: String,
        name: Option<String>,
        format: Option<String>,
        version: Option<String>,
        associated_file: Option<String>,
    },
    Instance {
        id: String,
        name: String,
        aggregated_by: Option<String>,
        instance_of: Option<String>,
        matrix: Option<Vec<f64>>,
    },
    InstRep {
        id: String,
        name: Option<String>,
        aggregated_by: Option<String>,
        instance_of: Option<String>,
    },
}

impl RecordBuilder {
    /// Start a record from its opening (or empty) element tag.
    fn start(e: &BytesStart<'_>) -> RecordBuilder {
        match local(e).as_str() {
            "Reference3D" => RecordBuilder::Node {
                id: attr(e, "id").unwrap_or_default(),
                name: attr(e, "name").unwrap_or_default(),
            },
            "ReferenceRep" => RecordBuilder::RefRep {
                id: attr(e, "id").unwrap_or_default(),
                name: attr(e, "name"),
                format: attr(e, "format"),
                version: attr(e, "version"),
                associated_file: attr(e, "associatedFile"),
            },
            "Instance3D" => RecordBuilder::Instance {
                id: attr(e, "id").unwrap_or_default(),
                name: attr(e, "name").unwrap_or_default(),
                aggregated_by: None,
                instance_of: None,
                matrix: None,
            },
            "InstanceRep" => RecordBuilder::InstRep {
                id: attr(e, "id").unwrap_or_default(),
                name: attr(e, "name"),
                aggregated_by: None,
                instance_of: None,
            },
            _ => unreachable!("only RECORD_ELEMENTS reach RecordBuilder::start"),
        }
    }

    /// Fold a direct-text child element value into the builder.
    fn feed_text(&mut self, leaf: &str, text: &str) {
        match (self, leaf) {
            (RecordBuilder::Instance { aggregated_by, .. }, "IsAggregatedBy")
            | (RecordBuilder::InstRep { aggregated_by, .. }, "IsAggregatedBy") => {
                *aggregated_by = Some(text.trim().to_string())
            }
            (RecordBuilder::Instance { instance_of, .. }, "IsInstanceOf")
            | (RecordBuilder::InstRep { instance_of, .. }, "IsInstanceOf") => {
                *instance_of = Some(text.trim().to_string())
            }
            (RecordBuilder::Instance { matrix, .. }, "RelativeMatrix") => {
                let values: Vec<f64> = text
                    .split_whitespace()
                    .map(|tok| tok.parse().unwrap_or(f64::NAN))
                    .collect();
                match matrix {
                    Some(existing) => existing.extend(values),
                    None => *matrix = Some(values),
                }
            }
            _ => {} // writer metadata / unknown children -> ignored (tolerant)
        }
    }

    /// Finalize into the public record. `Instance3D` with a matrix of the
    /// wrong size yields `Err`.
    fn finish(self) -> Result<FinishedRecord, RepError> {
        match self {
            RecordBuilder::Node { id, name } => Ok(FinishedRecord::Node(Reference3D { id, name })),
            RecordBuilder::RefRep {
                id,
                name,
                format,
                version,
                associated_file,
            } => Ok(FinishedRecord::RefRep(ReferenceRep {
                id,
                name,
                format,
                version,
                associated_file,
            })),
            RecordBuilder::Instance {
                id,
                name,
                aggregated_by,
                instance_of,
                matrix,
            } => {
                if let Some(m) = &matrix {
                    if m.len() != 12 {
                        return Err(RepError::Matrix {
                            counts: (m.len(), 12),
                        });
                    }
                }
                Ok(FinishedRecord::Instance(Instance3D {
                    id,
                    name,
                    is_aggregated_by: aggregated_by,
                    is_instance_of: instance_of,
                    relative_matrix: matrix,
                }))
            }
            RecordBuilder::InstRep {
                id,
                name,
                aggregated_by,
                instance_of,
            } => Ok(FinishedRecord::InstRep(InstanceRep {
                id,
                name,
                is_aggregated_by: aggregated_by,
                is_instance_of: instance_of,
            })),
        }
    }
}

/// Internal dispatch: a finalized record before it lands in the structure.
#[derive(Debug)]
enum FinishedRecord {
    Node(Reference3D),
    RefRep(ReferenceRep),
    Instance(Instance3D),
    InstRep(InstanceRep),
}

/// Fan a finished record into the correct vector of `ProductStructure`.
fn push_record(structure: &mut ProductStructure, name: &str, rec: FinishedRecord) {
    match (name, rec) {
        ("Reference3D", FinishedRecord::Node(n)) => structure.nodes.push(n),
        ("ReferenceRep", FinishedRecord::RefRep(r)) => structure.references.push(r),
        ("Instance3D", FinishedRecord::Instance(i)) => structure.instances.push(i),
        ("InstanceRep", FinishedRecord::InstRep(i)) => structure.instance_reps.push(i),
        _ => {} // defensive; types are aligned by construction
    }
}

/// Feed header child-element text values (e.g. `<SchemaVersion>`).
fn feed_header(header: &mut Header, leaf: &str, text: &str) {
    let value = text.trim().to_string();
    match leaf {
        "SchemaVersion" => header.schema_version = Some(value),
        "Title" => header.title = Some(value),
        "Author" => header.author = Some(value),
        "Generator" => header.generator = Some(value),
        "Created" => header.created = Some(value),
        _ => {} // unknown header metadata ignored (tolerant)
    }
}
