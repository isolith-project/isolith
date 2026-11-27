//! Diagnostics panel (wave E, card UI-E3 UI-ASSY-DIAGNOSTICS): the assembly
//! reports its degrees of freedom and the pairs that interfere, per pair,
//! with a jump-to that selects both components.
//!
//! ## What this panel is
//!
//! A `#[path]` child module of `assy_tree` (exactly like the mate tool):
//! `assy_tree::show` pulls it in with one `#[path]` include and renders it
//! with ONE call below the mate tool. Because it is a child module of
//! `assy_tree`, it reaches the parent's private [`super::AssemModel`] directly
//! (the same live canonical document and grounded set every other assembly
//! panel reads, one source of truth).
//!
//! ## The seam (no second solver, no second collision check)
//!
//! - **DOF readout.** The degrees of freedom are the solver's numbers, never
//!   fabricated. The panel computes the live `tree` dof itself by opening the
//!   EXISTING seam ([`oep_shell::assem::AssemblySession`]) over the current
//!   document + the tree's grounded mate set and reading `SolveReport::dof`
//!   (the same read [`assy_mates`](super::assy_mates) drives). Because the
//!   mate tool's own mate set lives in its ui state (not reachable from here),
//!   the panel ALSO reads the mate tool's published oracle
//!   ([`MATE_REPORT_KEY`](super::assy_mates::MATE_REPORT_KEY)) - which carries
//!   the seam's own `dof` for the FULL current constraint set (grounded +
//!   user mates) after every mate op or drag. When the mate tool has solved
//!   anything, that number IS the current dof; otherwise the panel's own
//!   tree-level solve is shown. Both numbers are published every frame
//!   (`tree_dof`, `mate_dof`) alongside the displayed `dof` and its
//!   `dof_source`, so the readout always equals the solver's report for the
//!   same assembly and moves when a mate is added or removed.
//! - **Interference.** Per-pair clash results come from the EXISTING analysis
//!   check ([`oep_analysis::clash::detect_in_assembly`]) over the live
//!   document - the same check the analysis crate's own committed clash
//!   fixture test drives. Never a second collision implementation, exactly as
//!   the card reads ("use the existing ... interference check. Do not write a
//!   second collision test"). Read-only: the analysis lane consumes the
//!   canonical document and reports; nothing is written to it.
//! - **Jump-to.** Each interfering-pair row carries a Jump button that selects
//!   BOTH components of the pair - the selection id list is published every
//!   frame under [`DIAG_SELECTION_KEY`] and in the report (`selection=`) so a
//!   test (or an assistive consumer) can read exactly the two ids. The tree's
//!   single highlight is set to the pair's first component (the jump focus),
//!   while the published list carries both - the card's "selects both
//!   components of the reported pair".
//!
//! ## The oracle
//!
//! Every frame the panel publishes a plain-`String` report under
//! [`DIAG_REPORT_KEY`] (`dof`, `dof_source`, `tree_dof`, `mate_dof`, `pairs`,
//! one `pair_i=` line per interfering pair, `selection`, `err`) plus the
//! jump-to selection list under [`DIAG_SELECTION_KEY`], so every acceptance
//! clause is provable with numbers read back from the running panel.

use eframe::egui;

use crate::app::App;
use crate::theme;

use oep_shell::assem::AssemblySession;

/// Context-data key of the diagnostics panel's published `String` oracle
/// (tests read it by value, like every other panel oracle in this wave).
pub const DIAG_REPORT_KEY: &str = "ui_e3_diag_report";
/// Context-data key under which the jump-to selection id list (both
/// components of the jumped pair) is published every frame.
pub const DIAG_SELECTION_KEY: &str = "ui_e3_diag_selection";
/// Context-data key of the panel's own persistent ui state.
const DIAG_STATE_KEY: &str = "ui_e3_diag_state";

/// One pair reported by the interference check, reduced to the ids the panel
/// and the oracle need (the innermost instance of each chain - this wave's
/// fixtures place parts directly, so the chain is exactly the instance).
#[derive(Debug, Clone, PartialEq)]
struct InterferingPair {
    instance_a: String,
    instance_b: String,
    /// Overlap volume in mm3 (document units cubed).
    volume: f64,
    /// Penetration location (overlap box centroid).
    location: [f64; 3],
}

impl InterferingPair {
    fn ids(&self) -> String {
        format!("{} x {}", self.instance_a, self.instance_b)
    }
}

fn chain_leaf(chain: &[String]) -> String {
    chain.last().cloned().unwrap_or_else(|| "-".to_string())
}

/// Run the EXISTING interference check over the live canonical document.
fn clashes_of(model: &super::AssemModel) -> Result<Vec<InterferingPair>, String> {
    let document = model
        .document
        .as_ref()
        .ok_or_else(|| "no document".to_string())?;
    let clashes = oep_analysis::clash::detect_in_assembly(document, &model.assembly_id())
        .map_err(|e| format!("{e:?}"))?;
    Ok(clashes
        .into_iter()
        .map(|c| InterferingPair {
            instance_a: chain_leaf(&c.chain_a),
            instance_b: chain_leaf(&c.chain_b),
            volume: c.volume,
            location: c.location,
        })
        .collect())
}

/// The tree-level live dof: open the EXISTING seam over the current document
/// and the tree's grounded mate set and read the solve report's `dof`. This
/// is the same solver and the same assembly the panel displays - never a
/// second solver. `-1` = the seam could not open or solve.
fn tree_dof_of(model: &super::AssemModel) -> i32 {
    let Some(document) = model.document.as_ref() else {
        return -1;
    };
    let Ok(json) = document.to_json_pretty() else {
        return -1;
    };
    let Ok(mut session) = AssemblySession::open(&json, &model.mate_set_json()) else {
        return -1;
    };
    match session.solve() {
        oep_shell::assem::SolveOutcome::Solved(report) => report.dof,
        oep_shell::assem::SolveOutcome::Diverged(_) => -1,
    }
}

/// One `key=value` field of a `String` report oracle line.
fn field(report: &str, key: &str) -> Option<String> {
    report
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")).map(|v| v.to_string()))
}

/// The mate tool's published live dof, when the tool has actually solved
/// something (at least one user mate, or a solve report line after a
/// drag/op). Returns `(dof, source_evidence)`.
fn mate_dof_of(ctx: &egui::Context) -> Option<i32> {
    let report: String = ctx
        .data(|d| d.get_temp::<String>(egui::Id::unique(super::assy_mates::MATE_REPORT_KEY)))
        .unwrap_or_default();
    let mates: usize = field(&report, "mates")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let solved_line = field(&report, "report").unwrap_or_default();
    let active = mates > 0 || !solved_line.trim().is_empty();
    if !active {
        return None;
    }
    field(&report, "dof").and_then(|v| v.parse().ok())
}

/// The panel's persistent ui state.
#[derive(Debug, Clone, Default)]
struct DiagUi {
    /// Instance ids of the most recent jump-to (both components).
    selection: Option<(String, String)>,
}

fn read_state(ctx: &egui::Context) -> DiagUi {
    ctx.data(|d| d.get_temp::<DiagUi>(egui::Id::unique(DIAG_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, st: &DiagUi) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(DIAG_STATE_KEY), st.clone()));
}

/// Publish the plain-`String` oracle + the jump-to selection list every frame.
fn publish_report(ctx: &egui::Context, model: &super::AssemModel, st: &DiagUi) {
    let tree_dof = tree_dof_of(model);
    let mate_dof = mate_dof_of(ctx);
    let (dof, source) = match mate_dof {
        Some(m) => (m, "mate"),
        None => (tree_dof, "tree"),
    };
    let pairs = clashes_of(model);
    let (pairs_ok, pairs_lines, err) = match pairs {
        Ok(list) => (
            true,
            list.iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "pair_{i}={}:vol={:.4}:loc={},{},{}",
                        p.ids(),
                        p.volume,
                        p.location[0],
                        p.location[1],
                        p.location[2]
                    )
                })
                .collect::<Vec<_>>(),
            "-".to_string(),
        ),
        Err(e) => (false, Vec::new(), e),
    };
    let sel = match &st.selection {
        Some((a, b)) => format!("{a},{b}"),
        None => "-".to_string(),
    };
    let mut report = String::new();
    report.push_str(&format!("dof={dof}\n"));
    report.push_str(&format!("dof_source={source}\n"));
    report.push_str(&format!("tree_dof={tree_dof}\n"));
    report.push_str(&format!(
        "mate_dof={}\n",
        mate_dof
            .map(|m| m.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    report.push_str(&format!(
        "pairs={}\n",
        if pairs_ok {
            (pairs_lines.len()).to_string()
        } else {
            "-".to_string()
        }
    ));
    for line in &pairs_lines {
        report.push_str(line);
        report.push('\n');
    }
    report.push_str(&format!("selection={sel}\n"));
    report.push_str(&format!("err={err}\n"));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(DIAG_REPORT_KEY), report));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(DIAG_SELECTION_KEY), sel));
}

/// Render the diagnostics panel as a normal in-flow SECTION of the assembly
/// region, rendered right after the tree's toolbar and BEFORE the mate tool.
/// One call from `assy_tree::show`.
///
/// The assembly region is a fixed rect that must now host three wave-E
/// surfaces (tree rows, diagnostics, mate tool). Laying everything out in
/// order - tree, then this diagnostics section, then the mate tool - keeps
/// every section fully visible with NO overlap in the shipped 1280x800
/// window (a section anchored after the mate tool was measured to overlap
/// the mate tool's controls), and the diagnostics readout sits naturally
/// right under the components it reports on. Cost: the dof readout that
/// comes from the mate tool's solve is published one frame after the op
/// (the mate tool runs later in the same frame); the readout updates within
/// one repaint either way.
#[allow(clippy::needless_pass_by_value)]
pub fn show(ui: &mut egui::Ui, app: &mut App, model: &mut super::AssemModel) {
    let _ = app;
    let mode = theme::mode_of(ui);
    ui.separator();
    ui.label(theme::heading(mode, "Diagnostics").size(13.0));

    let mut st = read_state(ui.ctx());

    // Live numbers this frame: tree-level solve over the grounded set, and
    // the mate tool's last published solve dof (published last frame, since
    // the mate tool renders after this section).
    let tree_dof = tree_dof_of(model);
    let mate_dof = mate_dof_of(ui.ctx());
    let (dof, source) = match mate_dof {
        Some(m) => (m, "mate"),
        None => (tree_dof, "tree"),
    };
    let clashes = clashes_of(model);

    // --- the dof readout line ---
    ui.horizontal_wrapped(|ui| {
        ui.label("degrees of freedom:");
        let dof_text = if dof < 0 {
            "unresolved".to_string()
        } else {
            format!("{dof}")
        };
        ui.label(
            egui::RichText::new(dof_text)
                .monospace()
                .size(12.0)
                .color(oep_ui_theme::TEXT_PRIMARY.for_mode(mode)),
        );
        ui.label(
            egui::RichText::new(format!("(from {source} solver report)"))
                .weak()
                .italics(),
        );
    });

    // --- interference table (per interfering pair + a jump-to) ---
    let mut jumped: Option<(String, String)> = None;
    match clashes {
        Ok(list) if list.is_empty() => {
            ui.label(
                egui::RichText::new(format!(
                    "interference: none ({})",
                    model
                        .document
                        .as_ref()
                        .map(|d| d.assemblies.first().map(|a| a.instances.len()).unwrap_or(0))
                        .unwrap_or(0)
                ))
                .weak()
                .italics(),
            );
        }
        Ok(list) => {
            ui.label(
                egui::RichText::new(format!(
                    "interference: {} pair{}",
                    list.len(),
                    if list.len() == 1 { "" } else { "s" }
                ))
                .color(oep_ui_theme::TEXT_PRIMARY.for_mode(mode)),
            );
            for p in &list {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "{}  (overlap {:.1} mm3 @ {},{},{})",
                        p.ids(),
                        p.volume,
                        p.location[0],
                        p.location[1],
                        p.location[2]
                    ));
                    let jump_label = format!("Jump {}", p.ids());
                    if ui.button(&jump_label).clicked() {
                        jumped = Some((p.instance_a.clone(), p.instance_b.clone()));
                    }
                });
            }
        }
        Err(e) => {
            ui.label(
                egui::RichText::new(format!("interference: {e}"))
                    .color(oep_ui_theme::DANGER.for_mode(mode)),
            );
        }
    }

    // --- apply the jump (select both components) ---
    if let Some((a, b)) = jumped {
        st.selection = Some((a.clone(), b.clone()));
        // The tree's single highlight is the jump focus (first component);
        // the published selection list carries BOTH ids.
        model.selection = Some(a);
    }

    publish_report(ui.ctx(), model, &st);
    write_state(ui.ctx(), &st);
}
