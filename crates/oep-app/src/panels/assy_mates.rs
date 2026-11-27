//! Mate tool (wave E, card UI-E2 UI-ASSY-MATES): mates are created, edited
//! and deleted against the LIVE solver from the assembly surface, and
//! dragging a component re-solves and reports degrees of freedom honestly.
//!
//! ## What this panel is
//!
//! A `#[path]` child module of `assy_tree` (exactly like the tree's own
//! doc_io include): `assy_tree::show` pulls it in with one `#[path]` include
//! and renders it with ONE call. Because it is a child module of `assy_tree`,
//! it can reach the parent's private [`super::AssemModel`] (the live
//! `document`, `grounded`, and routed `history`) directly, so every op below
//! operates on the SAME canonical document and journal the tree publishes -
//! one source of truth, byte-exact hashes.
//!
//! ## The seam (no second solver, no second transform path)
//!
//! All geometry movement goes through the existing assembly seam
//! ([`oep_shell::assem::AssemblySession`] - the same one UI-E1 uses). The
//! tool keeps its own ordered mate list (`Vec<MateSpec>`, the coincident /
//! plane-distance vocabulary the card names) and, for every operation,
//! re-opens the seam over the current document + constraint set, calls
//! [`add_mate`](oep_shell::assem::AssemblySession::add_mate) /
//! [`edit_mate`](oep_shell::assem::AssemblySession::edit_mate) /
//! [`remove_mate`](oep_shell::assem::AssemblySession::remove_mate), and on
//! [`MateOutcome::AppliedAndSolved`](oep_shell::assem::MateOutcome) writes
//! the seam's solved instance placements back into the document THROUGH THE
//! JOURNAL ([`super::doc_io::EditHistory`]), so the document hash moves
//! byte-exactly and every mate op is one undoable command. A diverging or
//! rejected solve is refused with the solver's NAMED error and never solved
//! anyway (no fabricated pose, document untouched).
//!
//! Mates are created by picking two anchors and a kind (coincident, or
//! plane-distance at a typed distance); the mate list shows the result with
//! per-row edit/delete. A drag handle per component drags the component
//! (journaled, coalesced into ONE undo entry per gesture) and immediately
//! re-solves, reporting the live `dof` from the solver's own report - never
//! swallowed.
//!
//! ## The oracle
//!
//! Each frame the panel publishes a plain-`String` report under
//! [`MATE_REPORT_KEY`] (`mates`, picks, `dof`, `moved`, `max_dist`, `err`,
//! `hash`, `can_undo`, plus the human `report=` line shown in the region) so
//! the acceptance can prove, with numbers, each acceptance clause.

use eframe::egui;

use crate::app::App;
use crate::theme;

/// Context-data key of the mate tool's published `String` oracle (tests read
/// it by value, like every other panel oracle in this wave).
pub const MATE_REPORT_KEY: &str = "ui_e2_mate_report";
/// Context-data key of the tool's own persistent UI state.
const MATE_STATE_KEY: &str = "ui_e2_mate_state";
/// Journal tokens recorded by this tool (UI-B2 routed seam).
pub const MATE_ADD_TOKEN: &str = "assy:mate:add";
pub const MATE_EDIT_TOKEN: &str = "assy:mate:edit";
pub const MATE_REMOVE_TOKEN: &str = "assy:mate:remove";
pub const MATE_DRAG_TOKEN: &str = "assy:mate:drag";

/// The derived-anchor picker vocabulary. The solver needs a point anchor at
/// the part ORIGIN on a moving instance ("origin"); a plane anchor must sit
/// on the fixed (grounded) instance. Everything else is refused with the
/// solver's named error, which the tool reports honestly.
const ANCHOR_NAMES: &[&str] = &["origin", "center", "top", "bottom", "right", "left"];

/// The mate kinds the tool creates (the card's coincident + distance
/// vocabulary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum MateKind {
    #[default]
    Coincident,
    PlaneDistance,
}

/// One user-created mate, in the seam's parse vocabulary.
#[derive(Debug, Clone, PartialEq)]
struct MateSpec {
    kind: MateKind,
    a_instance: String,
    a_anchor: String,
    b_instance: String,
    b_anchor: String,
    distance: f64,
}

impl MateSpec {
    /// Human description shown on the mate-list row.
    fn describe(&self) -> String {
        match self.kind {
            MateKind::Coincident => format!(
                "coincident {}:{} == {}:{}",
                self.a_instance, self.a_anchor, self.b_instance, self.b_anchor
            ),
            MateKind::PlaneDistance => format!(
                "distance {}:{} ~ {}:{} = {:.1}",
                self.a_instance, self.a_anchor, self.b_instance, self.b_anchor, self.distance
            ),
        }
    }

    /// The seam `parse_mate` arguments for this spec.
    fn seam_args(&self) -> (&'static str, Vec<String>) {
        match self.kind {
            MateKind::Coincident => (
                "coincident",
                vec![
                    self.a_instance.clone(),
                    self.a_anchor.clone(),
                    self.b_instance.clone(),
                    self.b_anchor.clone(),
                ],
            ),
            MateKind::PlaneDistance => (
                "plane-distance",
                vec![
                    self.a_instance.clone(),
                    self.a_anchor.clone(),
                    self.b_instance.clone(),
                    self.b_anchor.clone(),
                    format!("{}", self.distance),
                ],
            ),
        }
    }
}

/// The tool's persistent UI state.
#[derive(Debug, Clone, Default)]
struct MateUi {
    /// A-side pick (moving instance + its origin anchor; default cap:origin).
    a_instance: String,
    a_anchor: String,
    /// B-side pick (fixed/grounded instance + anchor; default base:top).
    b_instance: String,
    b_anchor: String,
    /// Selected kind.
    kind: MateKind,
    /// Distance text field (`plane-distance` only).
    distance: String,
    /// Ordered user-created mates.
    mates: Vec<MateSpec>,
    /// The index being edited, when the form is mid-edit (its spec is in the
    /// pickers); `None` = not editing.
    editing: Option<usize>,
    /// A per-gesture drag id (UI-B2 coalescing).
    drag_id: u64,
    /// Drag-start placement `(instance_id, transform)` so the cumulative
    /// `drag_delta()` maps to an absolute pose.
    drag_start: Option<(String, oep_geo::Transform)>,
    /// Last solve report (the never-swallowed human line).
    report: String,
    /// Seam-sourced numbers for the oracle.
    last_dof: i32,
    last_moved: usize,
    last_max_dist: f64,
    err: String,
}

/// The instance ids of the live assembly, in document order.
fn instance_ids(model: &super::AssemModel) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(doc) = &model.document {
        if let Some(asm) = doc.assemblies.first() {
            for inst in &asm.instances {
                out.push(inst.id.clone());
            }
        }
    }
    out
}

/// Build the `oep-assem-constraints` JSON for `grounded` fixed mates plus the
/// user `mates`, serialized by hand in exactly the shape the seam's `open`
/// parses (the same shape `assy_tree::AssemModel::mate_set_json` builds for
/// grounding, extended with coincident / plane-distance fragments).
fn constraint_json(assembly_id: &str, grounded: &[String], mates: &[MateSpec]) -> String {
    let mut entries: Vec<String> = Vec::new();
    for g in grounded {
        entries.push(format!("{{\"kind\":\"fixed\",\"instance\":\"{g}\"}}"));
    }
    for m in mates {
        let (kind, args) = m.seam_args();
        let frag = match m.kind {
            MateKind::Coincident => format!(
                "{{\"kind\":\"{kind}\",\"a\":{{\"instance\":\"{}\",\"anchor\":\"{}\"}},\
                 \"b\":{{\"instance\":\"{}\",\"anchor\":\"{}\"}}}}",
                args[0], args[1], args[2], args[3]
            ),
            MateKind::PlaneDistance => format!(
                "{{\"kind\":\"{kind}\",\"point\":{{\"instance\":\"{}\",\"anchor\":\"{}\"}},\
                 \"plane\":{{\"instance\":\"{}\",\"anchor\":\"{}\"}},\"distance\":{}}}",
                args[0], args[1], args[2], args[3], args[4]
            ),
        };
        entries.push(frag);
    }
    format!(
        "{{\"format\":\"oep-assem-constraints\",\"version\":1,\"units\":\"mm\",\
         \"assembly\":\"{assembly_id}\",\"mates\":[{}]}}",
        entries.join(",")
    )
}

fn document_json(model: &super::AssemModel) -> Option<String> {
    model.document.as_ref()?.to_json_pretty().ok()
}

/// Open the seam over the live document plus the mate set.
fn open_session(
    model: &super::AssemModel,
    mates: &[MateSpec],
) -> Option<oep_shell::assem::AssemblySession> {
    let doc = document_json(model)?;
    let asm_id = model.assembly_id();
    let cs = constraint_json(&asm_id, &model.grounded, mates);
    oep_shell::assem::AssemblySession::open(&doc, &cs).ok()
}

/// The seam index of the `i`-th user mate (fixed mates for grounded come
/// first in the constraint set).
fn seam_index(model: &super::AssemModel, user_index: usize) -> usize {
    model.grounded.len() + user_index
}

/// Collapse a document onto the engine's canonical float image so that a
/// serialize -> parse -> serialize round-trip (what [`super::doc_io`] snapshots
/// do on undo/redo) is byte-identical. Solver-produced placements (e.g.
/// `0.90630778703665005`-style f64 noise) are not guaranteed to already lie in
/// that image; collapsing them here, inside the journaled mutation, is what
/// makes every mate op one exact, undoable command (the card's "restore the
/// exact prior transform hash" contract). The values kept are exactly the ones
/// the engine's own canonical JSON would carry.
fn canonicalize_doc(document: &mut oep_geo::Document) {
    if let Ok(json) = document.to_json_pretty() {
        if let Ok(canonical) = oep_geo::Document::from_json_pretty(&json) {
            *document = canonical;
        }
    }
}

/// Write the seam's solved placements back into the canonical document,
/// journaled. The op is ALWAYS one undoable command (even when no instance
/// moved - e.g. deleting the last mate leaves free dofs in place), so the
/// journal stays aligned with mate ops and each undo restores the exact
/// prior document. Drag-gesture placements coalesce (one gesture = one
/// entry).
fn journal_solved(
    model: &mut super::AssemModel,
    token: &'static str,
    drag: Option<super::doc_io::history::DragId>,
    moved: &[(String, oep_geo::Transform)],
) {
    let moves = moved.to_vec();
    let Some(document) = model.document.as_mut() else {
        return;
    };
    // Canonicalize BEFORE the snapshot so the journal's before-hash and the
    // stored before-state are the same canonical bytes the engine would write.
    canonicalize_doc(document);
    model.history.apply(document, token, drag, |d| {
        if let Some(asm) = d.assemblies.first_mut() {
            for (id, t) in &moves {
                if let Some(inst) = asm.instances.iter_mut().find(|i| &i.id == id) {
                    inst.transform = t.clone();
                }
            }
        }
        // Canonicalize AFTER writing so the after-hash is stable too: an undo
        // that re-parses this snapshot restores byte-for-byte.
        canonicalize_doc(d);
    });
}

/// Run one seam mate op (add / edit / remove) and reduce it to oracle
/// numbers. On `AppliedAndSolved` the solved placements are journaled back
/// into the document.
fn run_op(
    model: &mut super::AssemModel,
    basis_mates: &[MateSpec],
    token: &'static str,
    op: impl FnOnce(&mut oep_shell::assem::AssemblySession) -> oep_shell::assem::MateOutcome,
) -> MateOpOutcome {
    let Some(mut session) = open_session(model, basis_mates) else {
        return MateOpOutcome::refused("no seam session");
    };
    match op(&mut session) {
        oep_shell::assem::MateOutcome::AppliedAndSolved { report, .. } => {
            let moved = report.moved.clone();
            let ids = moved.len();
            let max_dist = report.max_distance_error;
            let dof = report.dof;
            journal_solved(model, token, None, &moved);
            MateOpOutcome {
                ok: true,
                dof,
                moved: ids,
                max_dist,
                err: String::new(),
            }
        }
        oep_shell::assem::MateOutcome::AppliedButDiverged { error, .. } => MateOpOutcome {
            ok: false,
            dof: -1,
            moved: 0,
            max_dist: -1.0,
            err: format!(
                "refused: {}",
                oep_shell::assem::workbench_error_kind(&error)
            ),
        },
        oep_shell::assem::MateOutcome::Rejected { error, .. } => {
            // Refused with the solver's named error: the named workbench tag,
            // or the seam's own parse/edit reason.
            let err = match &error {
                oep_shell::assem::MateEditError::Workbench(e) => {
                    format!("refused: {}", oep_shell::assem::workbench_error_kind(e))
                }
                other => format!("refused: {other}"),
            };
            MateOpOutcome {
                ok: false,
                dof: -1,
                moved: 0,
                max_dist: -1.0,
                err,
            }
        }
    }
}

struct MateOpOutcome {
    ok: bool,
    dof: i32,
    moved: usize,
    max_dist: f64,
    err: String,
}

impl MateOpOutcome {
    fn refused(err: &str) -> Self {
        Self {
            ok: false,
            dof: -1,
            moved: 0,
            max_dist: -1.0,
            err: format!("refused: {err}"),
        }
    }
}

/// Re-solve the current document + mate set through the seam and journal the
/// solved placements back (used by the drag re-solve; placements coalesce
/// with the open drag gesture when `drag` is `Some`).
fn solve_current(
    model: &mut super::AssemModel,
    mates: &[MateSpec],
    drag: Option<super::doc_io::history::DragId>,
) -> MateOpOutcome {
    let Some(mut session) = open_session(model, mates) else {
        return MateOpOutcome::refused("no seam session");
    };
    match session.solve() {
        oep_shell::assem::SolveOutcome::Solved(report) => {
            let moved = report.moved.clone();
            let ids = moved.len();
            let max_dist = report.max_distance_error;
            let dof = report.dof;
            journal_solved(model, MATE_DRAG_TOKEN, drag, &moved);
            MateOpOutcome {
                ok: true,
                dof,
                moved: ids,
                max_dist,
                err: String::new(),
            }
        }
        oep_shell::assem::SolveOutcome::Diverged(e) => MateOpOutcome {
            ok: false,
            dof: -1,
            moved: 0,
            max_dist: -1.0,
            err: format!("refused: {}", oep_shell::assem::workbench_error_kind(&e)),
        },
    }
}

fn read_state(ctx: &egui::Context) -> MateUi {
    ctx.data(|d| d.get_temp::<MateUi>(egui::Id::unique(MATE_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, st: &MateUi) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(MATE_STATE_KEY), st.clone()));
}

/// Publish the plain-`String` oracle every frame.
fn publish_report(ctx: &egui::Context, model: &super::AssemModel, st: &MateUi) {
    let hash = model
        .document
        .as_ref()
        .map(super::doc_io::document_hash)
        .unwrap_or(0);
    let kind = match st.kind {
        MateKind::Coincident => "coincident",
        MateKind::PlaneDistance => "plane-distance",
    };
    let err = if st.err.is_empty() { "-" } else { &st.err };
    let report = format!(
        "mates={}\na_instance={}\na_anchor={}\nb_instance={}\nb_anchor={}\nkind={}\n\
         dof={}\nmoved={}\nmax_dist={:.3e}\nerr={err}\nhash={hash}\ncan_undo={}\nreport={}\n",
        st.mates.len(),
        st.a_instance,
        st.a_anchor,
        st.b_instance,
        st.b_anchor,
        kind,
        st.last_dof,
        st.last_moved,
        st.last_max_dist,
        model.history.can_undo(),
        st.report,
    );
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(MATE_REPORT_KEY), report));
}

/// Render the mate tool inside a compact vertical section below the tree.
/// One call from `assy_tree::show`; it owns no region, only content.
pub fn show(ui: &mut egui::Ui, app: &mut App, model: &mut super::AssemModel) {
    let _ = app;
    ui.separator();
    ui.label(theme::heading(theme::mode_of(ui), "Mates").size(13.0));

    let mut st = read_state(ui.ctx());
    let instances = instance_ids(model);

    // Sensible defaults on the first frame with a document: A = the moving
    // instance (cap) at origin, B = the grounded instance (base) at top.
    if st.a_instance.is_empty() {
        st.a_instance = instances
            .iter()
            .find(|id| id.as_str() == "cap")
            .cloned()
            .or_else(|| instances.first().cloned())
            .unwrap_or_default();
    }
    if st.b_instance.is_empty() {
        st.b_instance = instances
            .iter()
            .find(|id| id.as_str() == "base")
            .cloned()
            .or_else(|| instances.get(1).cloned())
            .or_else(|| instances.first().cloned())
            .unwrap_or_default();
    }
    if st.a_anchor.is_empty() {
        st.a_anchor = "origin".to_string();
    }
    if st.b_anchor.is_empty() {
        st.b_anchor = "top".to_string();
    }
    if st.distance.is_empty() {
        st.distance = "25".to_string();
    }

    let mut queued: Option<&'static str> = None; // add | edit | del | undo | redo
    let mut pending_edit_index: Option<usize> = None;
    let mut delete_index: Option<usize> = None;

    // --- kind + distance + add/edit (PINNED above the scroll region) ---
    // The primary action and its kind stay at a fixed position, so they are
    // always reachable in the shipped 1280x800 window and the report below is
    // never buried behind a scroll.
    ui.horizontal_wrapped(|ui| {
        let c = ui.selectable_label(st.kind == MateKind::Coincident, "kind coincident");
        if c.clicked() {
            st.kind = MateKind::Coincident;
        }
        let d = ui.selectable_label(st.kind == MateKind::PlaneDistance, "kind distance");
        if d.clicked() {
            st.kind = MateKind::PlaneDistance;
        }
        if st.kind == MateKind::PlaneDistance {
            ui.label("d=");
            ui.add(egui::TextEdit::singleline(&mut st.distance).desired_width(42.0));
        }
        let add_label = if st.editing.is_some() {
            "Apply edit"
        } else {
            "+ Add mate"
        };
        if ui.button(add_label).clicked() {
            queued = Some(if st.editing.is_some() { "edit" } else { "add" });
        }
    });

    // --- the never-swallowed solve report line, PINNED so it is always
    // visible in the region (never silently swallowed behind a scroll) ---
    let report_color = if st.err.is_empty() {
        oep_ui_theme::TEXT_MUTED.for_mode(theme::mode_of(ui))
    } else {
        oep_ui_theme::DANGER.for_mode(theme::mode_of(ui))
    };
    ui.label(
        egui::RichText::new(&st.report)
            .monospace()
            .size(11.0)
            .color(report_color),
    );

    // The assembly region has a FIXED height (the lower 48% of the model-tree
    // column, UI-E1); with the tree rows and toolbar above, the mate tool's
    // own content would run past the region's clip rect in the shipped
    // 1280x800 window. The picks, list, drag and undo/redo live in a vertical
    // ScrollArea bounded to the remaining region height; the kind/Add row and
    // the report line above stay pinned. In the tall acceptance harness
    // nothing overflows, so the scroll area is inert and every widget is
    // clicked directly.
    let scroll_height = ui.available_height().max(60.0);
    egui::ScrollArea::vertical()
        .id_salt("ui_e2_mates_scroll")
        .max_height(scroll_height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // --- pick two anchors (one compact line per side) ---
            // A = the moving instance + its origin anchor; B = the fixed/grounded
            // instance + any anchor (plane or point). Each side is a horizontal wrap
            // of instance chips + anchor chips; clicking updates the pick in state.
            let a_inst = st.a_instance.clone();
            let a_anchor = st.a_anchor.clone();
            let b_inst = st.b_instance.clone();
            let b_anchor = st.b_anchor.clone();
            for side in ['A', 'B'] {
                let sel_inst = if side == 'A' { &a_inst } else { &b_inst };
                let sel_anchor = if side == 'A' { &a_anchor } else { &b_anchor };
                ui.horizontal_wrapped(|ui| {
                    ui.label(side.to_string());
                    for id in &instances {
                        let label = format!("{side} inst {id}");
                        if ui.selectable_label(sel_inst == id, label).clicked() {
                            if side == 'A' {
                                st.a_instance = id.clone();
                                // The solver needs a point anchor at the part origin
                                // on the moving side; help the common case.
                                st.a_anchor = "origin".to_string();
                            } else {
                                st.b_instance = id.clone();
                            }
                        }
                    }
                    ui.separator();
                    for name in ANCHOR_NAMES {
                        let label = format!("{side} anchor {name}");
                        if ui.selectable_label(sel_anchor == name, label).clicked() {
                            if side == 'A' {
                                st.a_anchor = name.to_string();
                            } else {
                                st.b_anchor = name.to_string();
                            }
                        }
                    }
                });
            }

            // --- the mate list (with per-row edit/delete) ---
            for (i, m) in st.mates.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(m.describe());
                    if st.editing != Some(i) {
                        if ui.button(format!("Edit {i}")).clicked() {
                            pending_edit_index = Some(i);
                        }
                    } else {
                        ui.label("editing");
                    }
                    if ui.button(format!("Delete {i}")).clicked() {
                        delete_index = Some(i);
                    }
                });
            }

            // --- component drag: dragging re-solves and reports dof honestly ---
            ui.horizontal_wrapped(|ui| {
                ui.label("drag:");
                for id in &instances {
                    let response =
                        ui.add(egui::Label::new(format!("drag {id}")).sense(egui::Sense::drag()));
                    if response.drag_started() {
                        st.drag_id = st.drag_id.wrapping_add(1).max(1);
                        // Snapshot the drag-start placement so `drag_delta()` (which
                        // is cumulative from the gesture start) maps to an absolute
                        // pose, never to runaway per-frame increments.
                        if let Some(doc) = &model.document {
                            if let Some(asm) = doc.assemblies.first() {
                                if let Some(inst) = asm.instances.iter().find(|i| i.id == *id) {
                                    st.drag_start = Some((id.clone(), inst.transform.clone()));
                                }
                            }
                        }
                    }
                    if response.dragged() {
                        let Some(document) = model.document.as_mut() else {
                            continue;
                        };
                        let Some((start_id, start_t)) = st.drag_start.clone() else {
                            continue;
                        };
                        if start_id != *id {
                            continue;
                        }
                        let dx = response.drag_delta().x as f64 * 0.05;
                        let dy = response.drag_delta().y as f64 * 0.05;
                        let drag = super::doc_io::history::DragId(st.drag_id);
                        model
                            .history
                            .apply(document, MATE_DRAG_TOKEN, Some(drag), |d| {
                                if let Some(asm) = d.assemblies.first_mut() {
                                    if let Some(inst) =
                                        asm.instances.iter_mut().find(|i| i.id == *id)
                                    {
                                        inst.transform[12] = start_t[12] + dx;
                                        inst.transform[13] = start_t[13] + dy;
                                    }
                                }
                                // Keep the coalesced gesture entry byte-stable too.
                                canonicalize_doc(d);
                            });
                    }
                    if response.drag_stopped() {
                        let drag = super::doc_io::history::DragId(st.drag_id);
                        st.drag_start = None;
                        // Re-solve and REPORT dof (never swallowed); placements of the
                        // whole gesture coalesce into ONE undo entry.
                        let out = solve_current(model, &st.mates, Some(drag));
                        st.apply(out);
                        model.history.journal_mut().end_drag(drag);
                    }
                }
            });

            // --- undo / redo of the tree's journal (mates and any tree edit share
            // the same document journal, so both restore the exact prior doc). Named
            // "Mate undo"/"Mate redo" so they never collide with the model tree's own
            // Undo/Redo buttons in the shared AccessKit tree.
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(model.history.can_undo(), egui::Button::new("Mate undo"))
                    .clicked()
                {
                    queued = Some("undo");
                }
                if ui
                    .add_enabled(model.history.can_redo(), egui::Button::new("Mate redo"))
                    .clicked()
                {
                    queued = Some("redo");
                }
            });
        });

    // --- apply the queued operation ---
    if let Some(edit) = pending_edit_index {
        if edit < st.mates.len() {
            let spec = st.mates[edit].clone();
            st.kind = spec.kind;
            st.a_instance = spec.a_instance.clone();
            st.a_anchor = spec.a_anchor.clone();
            st.b_instance = spec.b_instance.clone();
            st.b_anchor = spec.b_anchor.clone();
            st.distance = format!("{:.1}", spec.distance);
            st.editing = Some(edit);
        }
    } else if let Some(del) = delete_index {
        if del < st.mates.len() {
            let idx = seam_index(model, del);
            let out = run_op(model, &st.mates, MATE_REMOVE_TOKEN, |s| s.remove_mate(idx));
            if out.ok {
                st.mates.remove(del);
                st.editing = None;
            }
            st.apply(out);
        }
    }

    match queued {
        Some("add") => {
            let spec = MateSpec {
                kind: st.kind,
                a_instance: st.a_instance.clone(),
                a_anchor: st.a_anchor.clone(),
                b_instance: st.b_instance.clone(),
                b_anchor: st.b_anchor.clone(),
                distance: st.distance.trim().parse().unwrap_or(25.0),
            };
            let out = run_op(model, &st.mates, MATE_ADD_TOKEN, |s| {
                let (kind, args) = spec.seam_args();
                let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                match oep_shell::assem::parse_mate(kind, &refs) {
                    Ok(m) => s.add_mate(m),
                    Err(e) => oep_shell::assem::MateOutcome::Rejected {
                        mate_index: 0,
                        error: oep_shell::assem::MateEditError::Other(e),
                    },
                }
            });
            if out.ok {
                st.mates.push(spec);
                st.editing = None;
            }
            st.apply(out);
        }
        Some("edit") => {
            if let Some(i) = st.editing {
                if i < st.mates.len() {
                    let spec = MateSpec {
                        kind: st.kind,
                        a_instance: st.a_instance.clone(),
                        a_anchor: st.a_anchor.clone(),
                        b_instance: st.b_instance.clone(),
                        b_anchor: st.b_anchor.clone(),
                        distance: st.distance.trim().parse().unwrap_or(25.0),
                    };
                    let edit_index = seam_index(model, i);
                    let out = run_op(model, &st.mates, MATE_EDIT_TOKEN, |s| {
                        let (kind, args) = spec.seam_args();
                        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                        match oep_shell::assem::parse_mate(kind, &refs) {
                            Ok(m) => s.edit_mate(edit_index, m),
                            Err(e) => oep_shell::assem::MateOutcome::Rejected {
                                mate_index: edit_index,
                                error: oep_shell::assem::MateEditError::Other(e),
                            },
                        }
                    });
                    if out.ok {
                        st.mates[i] = spec;
                        st.editing = None;
                    }
                    st.apply(out);
                }
            }
        }
        Some("undo") => {
            if let Some(document) = model.document.as_mut() {
                if model.history.undo(document).is_some() {
                    st.report = "undo: exact prior doc restored".to_string();
                    st.err = String::new();
                }
            }
        }
        Some("redo") => {
            if let Some(document) = model.document.as_mut() {
                if model.history.redo(document).is_some() {
                    st.report = "redo: exact next doc restored".to_string();
                    st.err = String::new();
                }
            }
        }
        _ => {}
    }

    publish_report(ui.ctx(), model, &st);
    write_state(ui.ctx(), &st);
}

impl MateUi {
    /// Fold an op outcome into the oracle numbers + the human report line.
    fn apply(&mut self, out: MateOpOutcome) {
        self.last_dof = out.dof;
        self.last_moved = out.moved;
        self.last_max_dist = out.max_dist;
        self.err = out.err;
        self.report = if self.err.is_empty() {
            format!(
                "solve dof={} moved={} max_dist={:.3e}",
                self.last_dof, self.last_moved, self.last_max_dist
            )
        } else {
            format!("{} (not solved)", self.err)
        };
    }
}
