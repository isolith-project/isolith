//! Assembly tree panel (wave E, card UI-E1 UI-ASSY-TREE): an assembly
//! document is visible and editable in the tree.
//!
//! The panel hosts a live canonical assembly document (an `oep_geo::Document`,
//! seeded from the committed two-part assembly fixture - the engine's own
//! golden for "two parts inserted") and drives every assembly operation
//! through the EXISTING assembly seam ([`oep_shell::assem::AssemblySession`]),
//! never a second one. Listing instances, moving and solving all read the
//! seam's own readouts; document edits (insert an instance, isolate via the
//! serialized `visible` flags) are journaled through the routed mutation seam
//! ([`doc_io::EditHistory`]) exactly like the model tree does, so every
//! change is undoable and byte-exact.
//!
//! ## What the tree shows
//!
//! Rows are derived live from the document every frame (never a cached list):
//! the assembly root, then one row per component instance carrying its
//! instance id, its name, the part it references and a geometry note from the
//! part's bodies (the card's "the tree shows both with their instance ids").
//! Rows are `selectable_label`s: each has an accessible name, is focusable and
//! can be activated by keyboard (Space/Enter), not only by mouse.
//!
//! ## Operations (the card's acceptance)
//!
//! - **Insert**: an "Insert {part}" button per committed fixture part adds a
//!   part instance to the assembly, cloning the committed fixture's canonical
//!   instance definition (id, name, source, placement, visible) so inserting
//!   base + cap reproduces the committed two-part document exactly.
//! - **Ground**: marks one component grounded as a `Fixed` mate and hands the
//!   mate set to the seam; the grounded set is the rows the seam's
//!   `moving_ids()` excludes, published by id.
//! - **Isolate**: marks one component visible and the rest invisible via the
//!   document's serialized `visible` flags (journaled); the other component's
//!   row and geometry disappear. Un-isolate restores every `visible` flag, so
//!   the canonical document - and therefore its hash - returns to the
//!   pre-isolate value byte-for-byte.
//!
//! The oracle ([`ASSY_REPORT_KEY`]) publishes rows/selection/grounded/visible
//! and the document hash each frame; the canonical document JSON is published
//! under [`ASSY_DOC_JSON_KEY`] so the acceptance can compare, byte-for-byte,
//! the document the UI builds with the document the CLI path builds for the
//! same two inserts.

use eframe::egui;

use crate::app::App;
use crate::theme;

use oep_shell::assem::{self, AssemblySession};

// The routed document seam (wave B, card UI-B2 UI-HISTORY), included verbatim
// exactly like the model tree does, so this panel mutates its live document
// through the same journal (undoable, byte-exact hashes). The assembly
// document is the same `oep_geo::Document` type the seam wraps.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../doc_io.rs"]
mod doc_io;

/// Context-data key under which the panel publishes its plain-`String` report
/// every frame: `open`, `assembly`, `rows`, `sel`, `grounded`, `visible`,
/// `hash`. A private struct here would make the oracle unreadable across the
/// `#[path]` copies, so the report is a `String` like every other panel.
pub const ASSY_REPORT_KEY: &str = "ui_e1_assy_report";
/// Context-data key under which the panel publishes the live canonical
/// assembly document JSON every frame (what `doc_io::write_oepdoc` would
/// write) - the acceptance reads this and compares it, byte-for-byte, with
/// the CLI path's document JSON.
pub const ASSY_DOC_JSON_KEY: &str = "ui_e1_assy_doc_json";
/// Context-data key a test may seed with the canonical `.oepdoc` JSON of the
/// assembly document the panel should open before its first frame (exactly
/// like the model tree's seed key). Absent, the committed two-part fixture is
/// opened instead.
pub const ASSY_SEED_KEY: &str = "ui_e1_assy_seed";
/// Context-data key under which the panel's own typed model lives (tests read
/// the `String` oracle instead, not this).
const ASSY_STATE_KEY: &str = "ui_e1_assy_state";

/// Journal token this panel records for an insert edit (the UI-B2 journal
/// records the token plus before/after hashes; `assy:` is this panel's own
/// editing language, like the model tree's `tree:` family).
pub const INSERT_TOKEN: &str = "assy:insert";
/// Journal token this panel records for an isolate / un-isolate edit.
pub const ISOLATE_TOKEN: &str = "assy:isolate";

/// The committed two-part assembly fixture: the engine's canonical document
/// for "two parts (base, cap) inserted into an assembly". Embedded so the
/// panel opens exactly the bytes the CLI/engine builds - no second format.
pub const FIXTURE_MODEL: &str =
    include_str!("../../../oep-assem/tests/golden/two-part-fixture.oepgeo.json");
/// Assembly id of the committed fixture.
pub const FIXTURE_ASSEMBLY_ID: &str = "oep-assembly-fixture";

/// The ordered committed part ids of the fixture (the "Insert"-able parts).
pub fn fixture_part_ids() -> Vec<String> {
    oep_geo::Document::from_json_pretty(FIXTURE_MODEL)
        .map(|d| d.parts.iter().map(|p| p.id.clone()).collect())
        .unwrap_or_default()
}

/// Clone the committed fixture's canonical instance definition for `part_id`
/// (id, name, source, placement, visible) - the insert comes "from the
/// committed fixtures", exactly as the golden defines it.
pub fn fixture_instance_for(part_id: &str) -> Option<oep_geo::Instance> {
    let doc = oep_geo::Document::from_json_pretty(FIXTURE_MODEL).ok()?;
    for asm in &doc.assemblies {
        for inst in &asm.instances {
            if inst.source.referenced_id() == part_id {
                return Some(inst.clone());
            }
        }
    }
    None
}

/// The panel's live assembly state, resident in the egui context store (like
/// the model tree's `TreeModel`).
#[derive(Debug, Clone, Default)]
struct AssemModel {
    /// The canonical assembly document (parts + assembly + instances).
    document: Option<oep_geo::Document>,
    /// The selected instance id (`None` = nothing selected).
    selection: Option<String>,
    /// Instance ids grounded (fixed) by the user.
    grounded: Vec<String>,
    /// When `Some(id)`, only that instance is visible (isolated).
    isolated: Option<String>,
    /// The routed, undoable edit history over the document (UI-B2 journal:
    /// every insert / isolate edit is one journaled command).
    history: doc_io::EditHistory,
    /// True once seeded.
    seeded: bool,
}

impl AssemModel {
    fn assembly_id(&self) -> String {
        self.document
            .as_ref()
            .and_then(|d| d.assemblies.first().map(|a| a.id.clone()))
            .unwrap_or_else(|| FIXTURE_ASSEMBLY_ID.to_string())
    }

    /// Build an `AssemblySession` over the live document plus the current mate
    /// set (committed mates + a `Fixed` mate per grounded id), through the
    /// existing seam. Every seam read below - instances, moving_ids, solve -
    /// comes from here, never from a second assembly implementation.
    fn session(&self) -> Result<AssemblySession, assem::AssemError> {
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| assem::AssemError::NoGeometry("no document".to_string()))?;
        let json = doc
            .to_json_pretty()
            .map_err(|e| assem::AssemError::Io(format!("serialize: {e}")))?;
        AssemblySession::open(&json, &self.mate_set_json())
    }

    /// The canonical `oep-assem-constraints` JSON this panel hands the seam:
    /// one `Fixed` mate per grounded instance. Deterministic - same grounded
    /// set, same JSON. (The committed fixture's solver mates are irrelevant to
    /// the tree behaviour; grounding is a Fixed-mate concern the seam owns.)
    fn mate_set_json(&self) -> String {
        let mut mates = String::new();
        for (i, id) in self.grounded.iter().enumerate() {
            if i > 0 {
                mates.push(',');
            }
            mates.push_str(&format!("{{\"kind\":\"fixed\",\"instance\":\"{id}\"}}"));
        }
        format!(
            "{{\"format\":\"oep-assem-constraints\",\"version\":1,\"units\":\"mm\",\
             \"assembly\":\"{}\",\"mates\":[{}]}}",
            self.assembly_id(),
            mates
        )
    }
}

/// Seed the live document on the first frame: the canonical JSON a test
/// placed under [`ASSY_SEED_KEY`] if any (equivalent to opening that assembly
/// document), else the committed two-part fixture - the engine's own "two
/// parts inserted" golden. A failed parse leaves an honest empty tree.
fn seed_model(ctx: &egui::Context, model: &mut AssemModel) {
    let seeded_json: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(ASSY_SEED_KEY)));
    let json = seeded_json.unwrap_or_else(|| FIXTURE_MODEL.to_string());
    match oep_geo::Document::from_json_pretty(&json) {
        Ok(doc) if doc.validate().is_empty() => {
            model.document = Some(doc);
        }
        _ => model.document = None,
    }
    model.seeded = true;
}

/// One visible component row in the assembly tree.
#[derive(Debug, Clone)]
struct AssyRow {
    instance_id: String,
    name: String,
    part_id: String,
    geometry: String,
}

/// The ordered visible instance rows of the assembly, derived live from the
/// document every frame (row = instance id, name, part id, geometry note).
fn rows_of(document: &oep_geo::Document, isolated: &Option<String>) -> Vec<AssyRow> {
    let mut rows = Vec::new();
    let Some(asm) = document.assemblies.first() else {
        return rows;
    };
    for inst in &asm.instances {
        match isolated {
            Some(id) if id != &inst.id => continue, // isolation hides others
            None if !inst.visible => continue,      // honour serialized visible
            _ => {}
        }
        let part_id = inst.source.referenced_id().to_string();
        let geometry = document
            .parts
            .iter()
            .find(|p| p.id == part_id)
            .map(|p| {
                format!(
                    "{} bod{}",
                    p.body_ids.len(),
                    if p.body_ids.len() == 1 { "y" } else { "ies" }
                )
            })
            .unwrap_or_else(|| "-".to_string());
        rows.push(AssyRow {
            instance_id: inst.id.clone(),
            name: inst.name.clone(),
            part_id,
            geometry,
        });
    }
    rows
}

/// One UI gesture collected during the frame, applied to the model afterwards
/// (immediate-mode: the panel paints first, then applies one action).
#[derive(Debug, Clone)]
enum AssyAction {
    None,
    Select,
    Insert(String),
    Ground(String),
    Unground(String),
    Isolate(String),
    Unisolate,
}

impl AssyAction {
    fn is_none(&self) -> bool {
        matches!(self, AssyAction::None)
    }
}

/// Render the assembly tree panel content inside its already-positioned
/// region (the nested lower portion of the model-tree column).
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    ui.label(theme::heading(theme::mode_of(ui), "Assembly Tree").size(13.0));
    ui.separator();
    ui.add_space(6.0);

    let mut model = read_model(ui.ctx());
    if !model.seeded {
        seed_model(ui.ctx(), &mut model);
    }

    let Some(document) = model.document.as_ref() else {
        ui.label(egui::RichText::new("No assembly open").weak().italics());
        ui.label(
            egui::RichText::new("Insert two parts from the committed fixtures")
                .weak()
                .italics(),
        );
        publish_report(ui.ctx(), &model, &[]);
        write_model(ui.ctx(), &model);
        app.status.mode = "assembly · empty".to_string();
        return;
    };

    let rows = rows_of(document, &model.isolated);

    // Selectable rows with accessible names ("{name} · instance {id} · from
    // {part}" [+ " · grounded"]). Focusable + Space/Enter-activatable.
    let mut action = AssyAction::None;
    for row in &rows {
        let selected = model.selection.as_deref() == Some(row.instance_id.as_str());
        let grounded_mark = if model.grounded.contains(&row.instance_id) {
            " · grounded"
        } else {
            ""
        };
        let label = format!(
            "{} · instance {} · from {}{grounded_mark}",
            row.name, row.instance_id, row.part_id
        );
        if ui.selectable_label(selected, &label).clicked() {
            model.selection = Some(row.instance_id.clone());
            action = AssyAction::Select;
        }
        // Geometry note (the "geometry" the card's isolation hides).
        ui.label(egui::RichText::new(format!("  {} ({})", row.part_id, row.geometry)).weak());
    }

    // Toolbar: insert a committed part, ground/unground, isolate/un-isolate.
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        let have_sel = model.selection.is_some();
        for part in fixture_part_ids() {
            if ui.button(format!("Insert {part}")).clicked() {
                action = AssyAction::Insert(part);
            }
        }
        if have_sel {
            let sel = model.selection.clone().unwrap_or_default();
            let grounded = model.grounded.contains(&sel);
            let isolated_this = model.isolated.as_deref() == Some(sel.as_str());
            if ui
                .button(if grounded { "Unground" } else { "Ground" })
                .clicked()
            {
                action = if grounded {
                    AssyAction::Unground(sel.clone())
                } else {
                    AssyAction::Ground(sel.clone())
                };
            }
            if ui
                .button(if isolated_this {
                    "Un-isolate"
                } else {
                    "Isolate"
                })
                .clicked()
            {
                action = if isolated_this {
                    AssyAction::Unisolate
                } else {
                    AssyAction::Isolate(sel.clone())
                };
            }
        } else {
            let _ = ui.add_enabled_ui(false, |ui| {
                let _ = ui.button("Ground");
            });
            let _ = ui.add_enabled_ui(false, |ui| {
                let _ = ui.button("Isolate");
            });
        }
    });
    ui.label(
        egui::RichText::new("Tab focuses a row or button · Space/Enter activates")
            .weak()
            .italics(),
    );

    if !action.is_none() {
        apply_action(&mut model, action);
    }

    publish_report(ui.ctx(), &model, &rows);
    write_model(ui.ctx(), &model);
    app.status.mode = format!(
        "assembly · {} component{}",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" }
    );
}

fn apply_action(model: &mut AssemModel, action: AssyAction) {
    let Some(document) = model.document.as_mut() else {
        return;
    };
    match action {
        AssyAction::None | AssyAction::Select => {}
        AssyAction::Insert(part) => {
            // Insert this committed part's canonical instance into the
            // assembly, journaled. Idempotent: a part already instanced is
            // not duplicated.
            if let Some(inst) = fixture_instance_for(&part) {
                let already = document
                    .assemblies
                    .iter()
                    .any(|a| a.instances.iter().any(|i| i.id == inst.id));
                if !already {
                    model.history.apply(document, INSERT_TOKEN, None, |d| {
                        if let Some(asm) = d.assemblies.first_mut() {
                            asm.instances.push(inst);
                        }
                    });
                }
            }
        }
        AssyAction::Ground(instance_id) => {
            if !model.grounded.contains(&instance_id) {
                model.grounded.push(instance_id);
            }
        }
        AssyAction::Unground(instance_id) => {
            model.grounded.retain(|id| id != &instance_id);
        }
        AssyAction::Isolate(instance_id) => {
            // Serialize isolation into the document's `visible` flags
            // (journaled): only the isolated instance stays visible.
            model.isolated = Some(instance_id.clone());
            let only = instance_id;
            model.history.apply(document, ISOLATE_TOKEN, None, |d| {
                if let Some(asm) = d.assemblies.first_mut() {
                    for inst in &mut asm.instances {
                        inst.visible = inst.id == only;
                    }
                }
            });
        }
        AssyAction::Unisolate => {
            model.isolated = None;
            model.history.apply(document, ISOLATE_TOKEN, None, |d| {
                if let Some(asm) = d.assemblies.first_mut() {
                    for inst in &mut asm.instances {
                        inst.visible = true;
                    }
                }
            });
        }
    }
}

// --- context-store plumbing (read/write the panel model) ---

fn read_model(ctx: &egui::Context) -> AssemModel {
    ctx.data(|d| d.get_temp(egui::Id::unique(ASSY_STATE_KEY)))
        .unwrap_or_default()
}

fn write_model(ctx: &egui::Context, model: &AssemModel) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(ASSY_STATE_KEY), model.clone()));
}

/// Publish the plain-`String` report oracle with the live assembly truth plus
/// the canonical document JSON under [`ASSY_DOC_JSON_KEY`].
fn publish_report(ctx: &egui::Context, model: &AssemModel, rows: &[AssyRow]) {
    let hash = model
        .document
        .as_ref()
        .map(doc_io::document_hash)
        .unwrap_or(0);
    let sel = model.selection.clone().unwrap_or_else(|| "-".to_string());
    // Grounded set, read from the seam: every assembly instance EXCEPT those
    // the live solve would move (`moving_ids`), i.e. exactly the instances
    // pinned by a Fixed mate. This is the same readout the CLI prints for the
    // same mate set, so the "grounded set as expected" claim is seam-verified.
    let grounded = model
        .session()
        .map(|s| {
            let moving = s.moving_ids();
            let mut pinned: Vec<String> = s
                .instances()
                .iter()
                .map(|i| i.id.clone())
                .filter(|id| !moving.contains(id))
                .collect();
            pinned.sort();
            pinned.join(",")
        })
        .unwrap_or_else(|_| "-".to_string());
    // Visible set: with isolation, the surviving rows; without, every instance
    // the document lists. Both are the card's "visible set" id list.
    let visible = if model.isolated.is_some() {
        if rows.is_empty() {
            "-".to_string()
        } else {
            rows.iter()
                .map(|r| r.instance_id.clone())
                .collect::<Vec<_>>()
                .join(",")
        }
    } else {
        let all: Vec<String> = model
            .document
            .as_ref()
            .map(|d| {
                d.assemblies
                    .iter()
                    .flat_map(|a| a.instances.iter().map(|i| i.id.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if all.is_empty() {
            "-".to_string()
        } else {
            all.join(",")
        }
    };
    let report = format!(
        "open={}\nassembly={}\nrows={}\nsel={sel}\ngrounded={grounded}\nvisible={visible}\nhash={hash}\n",
        model.document.is_some() as u8,
        model.assembly_id(),
        rows.len(),
    );
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(ASSY_REPORT_KEY), report));

    if let Some(document) = &model.document {
        if let Ok(json) = document.to_json_pretty() {
            ctx.data_mut(|d| {
                d.insert_temp::<String>(egui::Id::unique(ASSY_DOC_JSON_KEY), json);
            });
        }
    }
}
