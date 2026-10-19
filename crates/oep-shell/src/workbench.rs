//! OEP interchange workbench (P7-M2-T01).
//!
//! The workbench is the shell's panel/command surface for geometry
//! interchange. It owns the currently loaded model (one canonical
//! `oep_geo::Document`) and exposes a typed, non-panicking action set:
//!
//! - `Load` -- CGR / 3DXML / STEP bytes through the importer hub
//!   (`oep_import::import_bytes`, the one-canonical-format composition root).
//!   The CGR lane covers both eras: old-era `.cgr` via the classic
//!   polygon-soup extractor and modern `V5_CFV2` files via the latched modern
//!   spatial-mesh decoder (P2-M7-T01).
//! - `OpenFromPdm` / `SaveToPdm` -- PDM load/save against the P8 API contract
//!   (`oep_pdm::PdmClient`), currently satisfied by the in-memory local stub.
//!   Save serializes the loaded document to the canonical oep-geo JSON format
//!   and publishes it as a new vault revision; open reads a revision and
//!   restores the document.
//! - `Save` -- the interchange save lanes for CGR/3DXML/STEP. Writers are NOT
//!   delivered by this card (3DXML writer P3-M4, STEP/OBJ/glTF exporters
//!   P6-M4, each on their own lane), so the honest typed outcome is
//!   `InterchangeSavePending` naming the owning lane -- never fabricated
//!   output.
//!
//! Every action lands on the journal (`Workbench::journal`) so the surface is
//! observable and machine-callable (the P7-M5 CLI-parity target).

use oep_import::{Document, ImportOutcome, SourceKind};
use oep_pdm::{PdmClient, PdmError, PdmWriteRequest, RevisionSummary};

/// The four interchange formats the workbench catalog surface covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interchange {
    /// P2 -- CGR (both the classic .cgr and modern V5_CFV2 family).
    Cgr,
    /// P3 -- 3DXML.
    ThreeDXml,
    /// P4 -- STEP (ISO 10303-21 faceted read lane).
    Step,
}

impl Interchange {
    /// Stable tag used in the journal and outcomes.
    pub fn tag(self) -> &'static str {
        match self {
            Interchange::Cgr => "cgr",
            Interchange::ThreeDXml => "3dxml",
            Interchange::Step => "step",
        }
    }

    fn source_kind(self) -> SourceKind {
        match self {
            Interchange::Cgr => SourceKind::Cgr,
            Interchange::ThreeDXml => SourceKind::ThreeDXml,
            Interchange::Step => SourceKind::Step,
        }
    }
}

/// One user-facing action on the workbench (a "button press").
#[derive(Debug, Clone)]
pub enum WorkbenchAction {
    /// Load geometry bytes of `format` through the importer hub.
    Load {
        /// Which interchange format the bytes carry.
        format: Interchange,
        /// Raw file bytes.
        bytes: Vec<u8>,
    },
    /// Open a revision from the PDM vault and make it the current document.
    OpenFromPdm {
        /// Vault item id.
        item_id: String,
        /// Vault revision.
        revision: String,
    },
    /// Publish the current document as a new PDM vault revision (stub).
    SaveToPdm {
        /// Vault item id.
        item_id: String,
        /// Vault revision.
        revision: String,
        /// Optional free-form change summary.
        summary: String,
    },
    /// Save the current document to an interchange format.
    Save {
        /// Target interchange format.
        format: Interchange,
    },
}

/// Structural summary of a loaded canonical document (for the journal/CLI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentStats {
    /// Number of parts.
    pub parts: usize,
    /// Number of bodies.
    pub bodies: usize,
    /// Number of features.
    pub features: usize,
    /// Number of assemblies.
    pub assemblies: usize,
    /// Number of instances across all assemblies.
    pub instances: usize,
}

impl DocumentStats {
    /// Gather the structural counts of a canonical document.
    pub fn of(doc: &Document) -> Self {
        Self {
            parts: doc.parts.len(),
            bodies: doc.bodies.len(),
            features: doc.features.len(),
            assemblies: doc.assemblies.len(),
            instances: doc.assemblies.iter().map(|a| a.instances.len()).sum(),
        }
    }
}

/// The typed result of a workbench action. Never a panic; never fabricated.
#[derive(Debug)]
pub enum WorkbenchOutcome {
    /// A load succeeded; the canonical document is now the current model.
    Loaded {
        /// The interchange format that was loaded.
        format: &'static str,
        /// The canonical document now held by the workbench.
        document: Document,
        /// Structural summary for display.
        stats: DocumentStats,
        /// Optional measured note (e.g. modern mesh latch counts).
        note: Option<String>,
    },
    /// A PDM open succeeded; the canonical document is the current model.
    OpenedFromPdm {
        /// The canonical document restored from the vault revision.
        document: Document,
        /// Structural summary for display.
        stats: DocumentStats,
    },
    /// A PDM save succeeded; the revision record is in the vault.
    SavedToPdm {
        /// The stored revision record.
        revision: RevisionSummary,
    },
    /// The interchange save lane for this format has no writer yet: typed and
    /// honest, naming the owning lane (never fabricated output).
    InterchangeSavePending {
        /// Target format tag.
        format: &'static str,
        /// Which lane owns the writer and where it lands.
        reason: String,
    },
    /// The source was recognized-ish but no importable geometry was mapped.
    Skipped {
        /// Format tag that skipped.
        format: &'static str,
        /// Classified reason, measured not guessed.
        reason: String,
    },
    /// A hard failure (I/O, unmappable PDM record, internal inconsistency).
    Failed {
        /// Human-readable failure message.
        message: String,
    },
}

const SAVE_LANE_REASON: &[(&str, &str)] = &[
    (
        "cgr",
        "CGR save lane is not delivered by P7-M2-T01 (format is read-first; no clean-room CGR writer card exists yet)",
    ),
    (
        "3dxml",
        "3DXML writer is the P3-M4 lane (oep-3dxml writer card) and has not reached this accumulated stack yet",
    ),
    (
        "step",
        "STEP export is the P6-M4-T01 exporter lane (STEP/STL/glTF/OBJ/SVG/PDF) and has not reached this accumulated stack yet",
    ),
];

/// The workbench state machine: current canonical document + PDM vault +
/// operation journal.
pub struct Workbench {
    /// The currently loaded model, if any.
    document: Option<Document>,
    /// The PDM vault behind the [`oep_pdm::PdmClient`] contract.
    vault: Box<dyn PdmClient>,
    /// Ordered, human-readable journal of every action taken.
    journal: Vec<String>,
}

impl Workbench {
    /// A workbench bound to a caller-supplied PDM client (a stub today).
    ///
    /// The shell owns a `Box<dyn PdmClient>` so the same command surface will
    /// bind the real P8 server client when it lands (P7-M2-T02) with no
    /// caller changes.
    pub fn new(vault: Box<dyn PdmClient>) -> Self {
        Self {
            document: None,
            vault,
            journal: Vec::new(),
        }
    }

    /// A workbench bound to a fresh in-memory local PDM stub.
    pub fn with_local_vault() -> Self {
        Self::new(Box::<oep_pdm::LocalPdmVault>::default())
    }

    /// The current canonical document, if one is loaded.
    pub fn current_document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    /// The ordered operation journal (each entry is one action outcome).
    pub fn journal(&self) -> &[String] {
        &self.journal
    }

    /// Attach a canonical document directly without an import (authoring path:
    /// a model produced in the shell, used by the PDM round trip tests and the
    /// future sketcher). Appends a journal line.
    pub fn attach_document(&mut self, document: Document, label: &str) {
        let stats = DocumentStats::of(&document);
        let line = format!(
            "attached {label}: parts={} bodies={} features={}",
            stats.parts, stats.bodies, stats.features
        );
        self.journal.push(line);
        self.document = Some(document);
    }

    /// Execute one action and return its typed outcome.
    ///
    /// Non-panicking on every input path: an unrecognized source, junk bytes,
    /// a missing PDM item, or an absent writer all produce a typed outcome.
    pub fn run(&mut self, action: WorkbenchAction) -> WorkbenchOutcome {
        match action {
            WorkbenchAction::Load { format, bytes } => self.load(format, &bytes),
            WorkbenchAction::OpenFromPdm { item_id, revision } => {
                self.open_from_pdm(&item_id, &revision)
            }
            WorkbenchAction::SaveToPdm {
                item_id,
                revision,
                summary,
            } => self.save_to_pdm(&item_id, &revision, &summary),
            WorkbenchAction::Save { format } => self.save_interchange(format),
        }
    }

    fn load(&mut self, format: Interchange, bytes: &[u8]) -> WorkbenchOutcome {
        let kind = format.source_kind();
        match oep_import::import_bytes(kind, bytes) {
            Ok(ImportOutcome::Imported { document, report }) => {
                let note = modern_latch_note(&document);
                let stats = DocumentStats::of(&document);
                let note_line = note
                    .as_deref()
                    .map(|n| format!(" [{n}]"))
                    .unwrap_or_default();
                let line = format!(
                    "loaded {}: parts={} bodies={} features={}{}",
                    format.tag(),
                    stats.parts,
                    stats.bodies,
                    stats.features,
                    note_line
                );
                if !report.warnings.is_empty() {
                    self.journal.push(line);
                    for w in &report.warnings {
                        self.journal.push(format!("  warning: {w}"));
                    }
                } else {
                    self.journal.push(line);
                }
                self.document = Some(document.clone());
                WorkbenchOutcome::Loaded {
                    format: format.tag(),
                    document,
                    stats,
                    note,
                }
            }
            Ok(ImportOutcome::Skipped { source, reason }) => {
                let line = format!("{} load skipped: {reason}", source.name());
                self.journal.push(line);
                WorkbenchOutcome::Skipped {
                    format: format.tag(),
                    reason,
                }
            }
            Ok(ImportOutcome::Reserved { source, reason }) => {
                let line = format!("{} load reserved: {reason}", source.name());
                self.journal.push(line);
                WorkbenchOutcome::Skipped {
                    format: format.tag(),
                    reason,
                }
            }
            Err(e) => {
                let line = format!("{} load failed: {e}", format.tag());
                self.journal.push(line);
                WorkbenchOutcome::Failed {
                    message: e.to_string(),
                }
            }
        }
    }

    fn open_from_pdm(&mut self, item_id: &str, revision: &str) -> WorkbenchOutcome {
        let bytes = match self.vault.open_revision(item_id, revision) {
            Ok(bytes) => bytes,
            Err(PdmError::ItemNotFound { .. }) => {
                let line = format!("pdm open {item_id}@{revision}: item not found");
                self.journal.push(line);
                return WorkbenchOutcome::Failed {
                    message: format!("{item_id}@{revision} not in the PDM vault"),
                };
            }
            Err(PdmError::RevisionNotFound { .. }) => {
                let line = format!("pdm open {item_id}@{revision}: revision not found");
                self.journal.push(line);
                return WorkbenchOutcome::Failed {
                    message: format!("{item_id}@{revision} not in the PDM vault"),
                };
            }
            Err(e) => {
                let line = format!("pdm open {item_id}@{revision}: {e}");
                self.journal.push(line);
                return WorkbenchOutcome::Failed {
                    message: e.to_string(),
                };
            }
        };
        let text = match std::str::from_utf8(&bytes) {
            Ok(t) => t,
            Err(_) => {
                let line = format!("pdm open {item_id}@{revision}: blob is not UTF-8 oep-geo");
                self.journal.push(line);
                return WorkbenchOutcome::Failed {
                    message: format!(
                        "{item_id}@{revision} blob is not a canonical oep-geo document"
                    ),
                };
            }
        };
        match Document::from_json_pretty(text) {
            Ok(document) => {
                let stats = DocumentStats::of(&document);
                let line = format!(
                    "opened from PDM {item_id}@{revision}: parts={} bodies={}",
                    stats.parts, stats.bodies
                );
                self.journal.push(line);
                self.document = Some(document.clone());
                WorkbenchOutcome::OpenedFromPdm { document, stats }
            }
            Err(e) => {
                let line = format!("pdm open {item_id}@{revision}: bad canonical json ({e})");
                self.journal.push(line);
                WorkbenchOutcome::Failed {
                    message: format!("{item_id}@{revision} is not a canonical oep-geo document"),
                }
            }
        }
    }

    fn save_to_pdm(&mut self, item_id: &str, revision: &str, summary: &str) -> WorkbenchOutcome {
        let Some(document) = self.document.as_ref() else {
            let line = "pdm save: no document loaded".to_string();
            self.journal.push(line);
            return WorkbenchOutcome::Failed {
                message: "no document loaded to save to PDM".to_string(),
            };
        };
        let json = match document.to_json_pretty() {
            Ok(j) => j,
            Err(e) => {
                let line = format!("pdm save: serialize failed ({e})");
                self.journal.push(line);
                return WorkbenchOutcome::Failed {
                    message: format!("failed to serialize the canonical document ({e})"),
                };
            }
        };
        let mut request = PdmWriteRequest::new(item_id, revision, json.into_bytes());
        if !summary.is_empty() {
            request = request.with_summary(summary);
        }
        match self.vault.save_revision(&request) {
            Ok(record) => {
                let line = format!(
                    "saved to PDM {item_id}@{revision} ({}, digest {})",
                    record.state,
                    record.blob_digest.as_deref().unwrap_or("none")
                );
                self.journal.push(line);
                WorkbenchOutcome::SavedToPdm { revision: record }
            }
            Err(e) => {
                let line = format!("pdm save {item_id}@{revision}: {e}");
                self.journal.push(line);
                WorkbenchOutcome::Failed {
                    message: e.to_string(),
                }
            }
        }
    }

    fn save_interchange(&mut self, format: Interchange) -> WorkbenchOutcome {
        let tag = format.tag();
        let reason = SAVE_LANE_REASON
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, r)| (*r).to_string())
            .unwrap_or_else(|| "writer not delivered".to_string());
        let line = format!("{tag} save pending: {reason}");
        self.journal.push(line);
        WorkbenchOutcome::InterchangeSavePending {
            format: tag,
            reason,
        }
    }
}

/// Render a measured note from a modern CGR import feature, when present.
fn modern_latch_note(doc: &Document) -> Option<String> {
    let feat = doc.features.first()?;
    let param = |k: &str| feat.parameters.get(k).and_then(|v| v.as_u64());
    let (records, index, vert, tri, maxi) = (
        param("records")?,
        param("index_count")?,
        param("vertex_count")?,
        param("triangle_count")?,
        param("max_index")?,
    );
    Some(format!(
        "modern mesh latch: records={records} index={index} vertex={vert} triangle={tri} max_index={maxi}"
    ))
}
