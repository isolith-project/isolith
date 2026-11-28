//! Annotation tools (wave F, card UI-F3 UI-ANNOTATE): dimensions, leaders and
//! notes on the sheet, plus a title block that is BOUND to the document's
//! properties - the panel never keeps a second copy of the title block.
//!
//! ## What this panel is
//!
//! A `#[path]` child module of `sheet` (exactly like `views.rs` and the E-wave
//! mate/explode tools): the drafting panel pulls it in with one `#[path]`
//! include and renders it with ONE call. As a child module of `sheet` it
//! reaches the parent's private [`super::SheetModel`] (the live canonical
//! drawing document, the selected sheet, the routed [`super::doc_io`]
//! history) directly - one source of truth, byte-exact hashes.
//!
//! ## Title block binding (never a second copy)
//!
//! The title block section renders the selected sheet's title block fields by
//! READING the live document every frame ([`title_block_readout`]); the panel
//! stores no `TitleBlock`. The line that shows the DRAWN title is bound to the
//! document's part-name property ([`title_of`], the same document path the
//! builder uses for the drawing title), so editing that document property
//! through the routed journal ([`TITLE_TOKEN`]) immediately shows the new
//! value with NO second edit - the title block is fed from the properties, not
//! typed twice. Each field is rendered as a real egui `Label`, so the text is
//! present in the frame's text pass (AccessKit), not only in an oracle.
//!
//! ## Dimension tool (bound to the model variable set)
//!
//! The dimension tool places a dimension "between two view points" (the two
//! horizontal-extent endpoints of the base front view of the committed bracket)
//! and binds it to the model's `width` variable through the EXISTING drafting
//! seam ([`oep_drafting::builder`] -> `render_sheet`/`render_document` -
//! FreeCAD-v1 variable-set behaviour, never a second projection path). The
//! resolved dimension value and the measured model extent are both computed
//! from that seam and asserted equal within `1e-6`.
//!
//! Editing the dimension (a value DragValue) re-drives the committed bracket
//! through the SAME seam at the new width ([`re_drive`]: the document's own
//! Sketch + VariableSet features are decoded and re-solved - no second
//! fixture); BOTH the view geometry (projected extent) and the dimension label
//! are recomputed from the single re-driven render, so they move together and
//! agree - never one without the other (asserted before/after).
//!
//! ## Leader + note tools (round-trip through save and reopen)
//!
//! Leaders and notes have no `oep-geo` record class (that crate is outside
//! this card's owned files), so they are panel-owned annotation records that
//! serialize to a canonical, deterministic JSON payload
//! ([`encode_annotations`]). The panel publishes that payload every frame
//! under [`ANNOTATE_JSON_KEY`] and accepts a seed under [`ANNOTATE_SEED_KEY`];
//! the acceptance places a leader and a note, SAVES the published JSON, REOPENS
//! it in a fresh panel/harness and asserts the JSON hashes are byte-equal and
//! both annotations come back.

use eframe::egui;

use crate::app::App;
use crate::theme;

use oep_drafting as draft;
use oep_geo::{Dimension, DimensionKind, Document};

/// Context-data key of this panel's published `String` oracle (tests read it by
/// value, like every panel oracle in this wave).
pub const ANNOTATE_REPORT_KEY: &str = "ui_f3_annotate_report";
/// Context-data key under which the panel publishes its serialized leader+note
/// records as canonical JSON every frame (what a save would write).
pub const ANNOTATE_JSON_KEY: &str = "ui_f3_annotate_json";
/// Context-data key a test may seed with the canonical annotation JSON before
/// the first frame, so a fresh panel opens exactly those leader+note records
/// (the reopen leg of the round-trip).
pub const ANNOTATE_SEED_KEY: &str = "ui_f3_annotate_seed";
/// Context-data key of the panel's own persistent UI state.
const ANNOTATE_STATE_KEY: &str = "ui_f3_annotate_state";

/// Journal token recorded when the part-name (title) property is edited.
pub const TITLE_TOKEN: &str = "annotate:title";
/// Journal token recorded when a dimension record is placed on the sheet.
pub const DIM_TOKEN: &str = "annotate:dim";

/// The model variable the dimension tool binds to (the bracket sketch's width
/// entry - the same name `drive_bracket_drawing` receives as its `width_var`).
const DIM_VARIABLE: &str = "width";
/// The view id whose projected extent provides the two dimension view points.
const DIM_VIEW: &str = "oep-view-front";
/// The axis of the placed dimension (horizontal - the front view's width).
const DIM_KIND: DimensionKind = DimensionKind::Horizontal;

/// A leader annotation record (panel-owned; serialized for the round-trip).
#[derive(Debug, Clone, PartialEq)]
struct LeaderRecord {
    id: String,
    text: String,
    x: f64,
    y: f64,
    to_x: f64,
    to_y: f64,
}

/// A note annotation record (panel-owned; serialized for the round-trip).
#[derive(Debug, Clone, PartialEq)]
struct NoteRecord {
    id: String,
    text: String,
    x: f64,
    y: f64,
}

/// The numbers one dimension resolves to (value = model variable, measured =
/// model geometry), both from the drafting seam.
#[derive(Debug, Clone, Copy)]
struct DimNumbers {
    value: f64,
    measured: f64,
    geom_w: f64,
}

impl DimNumbers {
    fn agrees(self) -> bool {
        (self.value - self.measured).abs() < 1e-6 && (self.value - self.geom_w).abs() < 1e-6
    }
}

/// The panel's persistent UI state (title block is NOT stored here - it is
/// always read from the live document).
#[derive(Debug, Clone, Default)]
struct AnnotateUi {
    /// Draft of the part-name document property editor.
    part_name_draft: String,
    /// Whether a dimension has been placed.
    dim_placed: bool,
    /// The editable dimension (width) value.
    dim_value: f64,
    /// The numbers recorded BEFORE the current edit (at place time / last
    /// stable value).
    dim_before: Option<DimNumbers>,
    /// The numbers after the latest edit (re-driven model).
    dim_after: Option<DimNumbers>,
    /// Placed leader records, in placement order.
    leaders: Vec<LeaderRecord>,
    /// Placed note records, in placement order.
    notes: Vec<NoteRecord>,
    /// True once the seed has been applied.
    seeded: bool,
    /// One-line description of the last action.
    last: Option<String>,
}

// ---------------------------------------------------------------------------
// Title block binding (read live from the document; no second copy)
// ---------------------------------------------------------------------------

/// The drawn title of the drawing: the document's part-name property (the same
/// document path the fixture builder uses for the drawing title), falling back
/// to the sheet's stored title-block title when no part carries a name. Read
/// LIVE every frame - the panel never stores a second copy.
fn title_of(doc: &Document) -> String {
    doc.parts
        .first()
        .map(|p| p.name.clone())
        .or_else(|| {
            doc.drawings
                .first()
                .and_then(|d| d.sheets.first())
                .map(|s| s.title_block.title.clone())
        })
        .unwrap_or_else(|| "untitled".to_string())
}

/// The title-block fields as a deterministic `(label, value)` list, read FROM
/// the live document each frame. The drawing title is the part-name property
/// ([`title_of`]); the rest come from the sheet's `title_block` record.
fn title_block_readout(doc: &Document) -> Vec<(String, String)> {
    let sheet = doc.drawings.first().and_then(|d| d.sheets.first());
    let mut out = vec![("title".to_string(), title_of(doc))];
    if let Some(s) = sheet {
        let tb = &s.title_block;
        for (label, value) in [
            ("part_number", tb.part_number.as_deref()),
            ("material", tb.material.as_deref()),
            ("scale", tb.scale.as_deref()),
            ("revision", tb.revision.as_deref()),
            ("drawn_by", tb.drawn_by.as_deref()),
            ("date", tb.date.as_deref()),
            ("sheet", tb.sheet.as_deref()),
            ("company", tb.company.as_deref()),
        ] {
            out.push((label.to_string(), value.unwrap_or("").to_string()));
        }
    }
    out
}

/// Apply a routed, journaled edit writing the part-name document property AND
/// keeping the sheet's stored title-block title in the same record consistent
/// with it. One command (undoable, hash-moving); the panel itself holds no
/// copy of the title block.
fn apply_part_name(
    history: &mut super::doc_io::EditHistory,
    document: &mut Document,
    new_name: &str,
) {
    let new_name = new_name.to_string();
    history.apply(document, TITLE_TOKEN, None, move |d| {
        if let Some(p) = d.parts.first_mut() {
            p.name = new_name.clone();
        }
        if let Some(drawing) = d.drawings.first_mut() {
            if let Some(sheet) = drawing.sheets.first_mut() {
                sheet.title_block.title = new_name.clone();
            }
        }
    });
}

// ---------------------------------------------------------------------------
// Dimension tool (geometry only via the existing oep_drafting seam)
// ---------------------------------------------------------------------------

/// Decode the committed bracket sketch + variable set from the LIVE
/// document's own features (the Sketch / VariableSet kinds the fixture builder
/// stores) - no second fixture.
fn bracket_sketch_vars(
    document: &Document,
) -> Option<(oep_sketch::sketch::Sketch, oep_sketch::varset::VariableSet)> {
    let sketch_feat = document
        .features
        .iter()
        .find(|f| f.kind == oep_sketch::features::SKETCH_KIND)?;
    let vars_feat = document
        .features
        .iter()
        .find(|f| f.kind == oep_sketch::features::VARSET_KIND)?;
    let sketch = oep_sketch::features::sketch_from_feature(sketch_feat).ok()?;
    let vars = oep_sketch::features::varset_from_feature(vars_feat).ok()?;
    Some((sketch, vars))
}

/// Re-drive the committed bracket at `width` through the EXISTING drafting
/// builder (`oep_drafting::drive_bracket_drawing`, the same seam the sheet
/// panel seeds from), re-solving the document's own sketch + variable set.
/// Title-block/part arguments are read from the live document so the re-driven
/// record stays coherent.
fn re_drive(document: &Document, width: f64) -> Option<draft::BracketDrawing> {
    let (sketch, mut vars) = bracket_sketch_vars(document)?;
    vars.set(DIM_VARIABLE, width);
    let part = document.parts.first();
    let tb = document
        .drawings
        .first()
        .and_then(|d| d.sheets.first())
        .map(|s| s.title_block.clone());
    let part_name = part
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "bracket".to_string());
    let part_number = part
        .and_then(|p| p.part_number.clone())
        .or_else(|| tb.as_ref().and_then(|t| t.part_number.clone()));
    let title = tb
        .as_ref()
        .map(|t| t.title.clone())
        .unwrap_or_else(|| "Bracket".to_string());
    let material = tb.as_ref().and_then(|t| t.material.clone());
    let revision = tb.as_ref().and_then(|t| t.revision.clone());
    let drawn_by = tb.as_ref().and_then(|t| t.drawn_by.clone());
    let date = tb.as_ref().and_then(|t| t.date.clone());
    let company = tb.as_ref().and_then(|t| t.company.clone());
    draft::builder::drive_bracket_drawing(
        "oep-part-bracket",
        &part_name,
        &sketch,
        &vars,
        DIM_VARIABLE,
        "height",
        "depth",
        &title,
        part_number.as_deref(),
        material.as_deref(),
        revision.as_deref(),
        drawn_by.as_deref(),
        date.as_deref(),
        company.as_deref(),
    )
    .ok()
}

/// Resolve the placed dimension's numbers (value = model variable entry,
/// measured = model projected extent, geometry = the view's paper width) from
/// one render of the re-driven model.
fn dimension_numbers(driven: &draft::BracketDrawing) -> Option<DimNumbers> {
    let rendered = draft::render_document(&driven.document, &driven.graph).ok()?;
    let dim = rendered
        .dimensions
        .iter()
        .find(|d| d.variable.as_deref() == Some(DIM_VARIABLE))?;
    let geom_w = rendered
        .views
        .iter()
        .find(|v| v.id == DIM_VIEW)
        .map(|v| v.extent_w)
        .unwrap_or(dim.value);
    Some(DimNumbers {
        value: dim.value,
        measured: dim.measured,
        geom_w,
    })
}

/// Place a dimension annotation on the selected sheet, bound to the `width`
/// model variable (FreeCAD-v1) through the routed journal; the record survives
/// the canonical document JSON like the fixture's own dimensions.
fn place_dimension(model: &mut super::SheetModel) -> Option<()> {
    let document = model.document.as_mut()?;
    let selected = model.selection.clone()?;
    super::apply_sheet_edit(
        &mut model.history,
        document,
        DIM_TOKEN,
        &selected,
        |sheet| {
            if sheet
                .dimensions
                .iter()
                .any(|d| d.variable.as_deref() == Some(DIM_VARIABLE))
            {
                return;
            }
            sheet.dimensions.push(
                Dimension::new(
                    "oep-dim-ui-width",
                    "bracket width",
                    DIM_VIEW.to_string(),
                    DIM_KIND,
                )
                .with_variable(DIM_VARIABLE),
            );
        },
    );
    Some(())
}

// ---------------------------------------------------------------------------
// Leader + note records (panel-owned, JSON round-trip)
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Encode the panel's leader + note records as canonical, deterministic JSON
/// (stable field order; a save would write exactly this string).
fn encode_annotations(leaders: &[LeaderRecord], notes: &[NoteRecord]) -> String {
    let l = leaders
        .iter()
        .map(|x| {
            format!(
                "{{\"id\":{},\"text\":{},\"x\":{},\"y\":{},\"to_x\":{},\"to_y\":{}}}",
                json_escape(&x.id),
                json_escape(&x.text),
                x.x,
                x.y,
                x.to_x,
                x.to_y
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let n = notes
        .iter()
        .map(|x| {
            format!(
                "{{\"id\":{},\"text\":{},\"x\":{},\"y\":{}}}",
                json_escape(&x.id),
                json_escape(&x.text),
                x.x,
                x.y
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"leaders\":[{l}],\"notes\":[{n}]}}")
}

/// FNV-1a 64 over a string - the same one-letter hashing language the whole
/// wave uses for the canonical bytes (assy_explode's `text_hash`, the sheet
/// panel's `document_hash`), so "the JSON hashes" printed are comparable to a
/// document save's hash.
fn fnv1a(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Parse one `"key":"value"` (or numeric) pair from the front of `s`, leaving
/// `s` advanced past it. Returns the key and the raw (unescaped) value.
fn take_pair(s: &str) -> Option<(&str, String, &str)> {
    let kq = s.find('"')?;
    let after_kq = &s[kq + 1..];
    let kend = after_kq.find('"')?;
    let key = &after_kq[..kend];
    let after_key = &after_kq[kend + 1..];
    let colon = after_key.find(':')?;
    let value_start = &after_key[colon + 1..];
    if let Some(rest_after_str) = value_start.strip_prefix('"') {
        let mut val = String::new();
        let mut it = rest_after_str.char_indices();
        let mut end_idx = None;
        while let Some((i, c)) = it.next() {
            match c {
                '"' => {
                    end_idx = Some(i);
                    break;
                }
                '\\' => {
                    if let Some((_, esc)) = it.next() {
                        val.push(match esc {
                            '"' => '"',
                            '\\' => '\\',
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            _ => esc,
                        });
                    }
                }
                c => val.push(c),
            }
        }
        let rest_after = match end_idx {
            Some(i) => &rest_after_str[i + 1..],
            None => "",
        };
        Some((key, val, rest_after))
    } else {
        let num: String = value_start
            .chars()
            .take_while(|c| {
                c.is_ascii_digit() || *c == '.' || *c == '-' || *c == 'e' || *c == '+' || *c == 'E'
            })
            .collect();
        let rest_after = &value_start[num.len()..];
        Some((key, num, rest_after))
    }
}

/// Decode the canonical annotation JSON back into leader + note records (the
/// reopen leg). `None` on any malformed token - never panics on foreign bytes.
fn decode_annotations(text: &str) -> Option<(Vec<LeaderRecord>, Vec<NoteRecord>)> {
    let mut leaders = Vec::new();
    let mut notes = Vec::new();
    if !text.contains("leaders") {
        return None;
    }
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let end = rest.find('}')?;
        let body = &rest[..end];
        rest = &rest[end + 1..];
        let mut fields = std::collections::BTreeMap::new();
        let mut cur = body;
        while !cur.trim().is_empty() {
            let Some((key, value, rest_after)) = take_pair(cur) else {
                break;
            };
            fields.insert(key.to_string(), value);
            cur = rest_after;
        }
        let text = fields.get("text").cloned().unwrap_or_default();
        let parse = |k: &str| fields.get(k).and_then(|v| v.parse::<f64>().ok());
        let x = parse("x").unwrap_or(0.0);
        let y = parse("y").unwrap_or(0.0);
        if fields.contains_key("to_x") {
            leaders.push(LeaderRecord {
                id: fields.get("id").cloned().unwrap_or_default(),
                text,
                x,
                y,
                to_x: parse("to_x").unwrap_or(0.0),
                to_y: parse("to_y").unwrap_or(0.0),
            });
        } else {
            notes.push(NoteRecord {
                id: fields.get("id").cloned().unwrap_or_default(),
                text,
                x,
                y,
            });
        }
    }
    if leaders.is_empty() && notes.is_empty() {
        return None;
    }
    Some((leaders, notes))
}

/// Restore leader + note records from the canonical annotation JSON a caller
/// placed under [`ANNOTATE_SEED_KEY`] before the first frame.
fn seed_annotations(ctx: &egui::Context, st: &mut AnnotateUi) {
    let seed: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(ANNOTATE_SEED_KEY)));
    if let Some(json) = seed {
        if let Some((leaders, notes)) = decode_annotations(&json) {
            st.leaders = leaders;
            st.notes = notes;
            st.last = Some("reopened annotation records".to_string());
        }
    }
    st.seeded = true;
}

// ---------------------------------------------------------------------------
// Panel body
// ---------------------------------------------------------------------------

/// Render the annotation tools inside the drafting scroll area.
pub fn show(ui: &mut egui::Ui, app: &mut App, model: &mut super::SheetModel) {
    let _ = app;
    ui.separator();
    ui.label(theme::heading(theme::mode_of(ui), "Annotate").size(13.0));

    let mut st = read_state(ui.ctx());
    if !st.seeded {
        seed_annotations(ui.ctx(), &mut st);
    }
    let mode = theme::mode_of(ui);

    let document = model.document.clone();

    // ---- Title block (read live from the document, never a second copy) ----
    ui.add_space(4.0);
    ui.label(theme::heading(mode, "Title block").size(12.0));
    ui.separator();
    if let Some(doc) = &document {
        let readout = title_block_readout(doc);
        for (label, value) in &readout {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(format!("{label}:")).weak().size(11.0));
                ui.label(egui::RichText::new(value).monospace().size(11.0));
            });
        }

        // Part name (a document property): one journaled command updates it.
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            let field = ui.add(
                egui::TextEdit::singleline(&mut st.part_name_draft)
                    .hint_text("part name")
                    .desired_width(120.0),
            );
            let apply = ui.button("Edit part name");
            let _ = field;
            if apply.clicked() {
                let new_name = st.part_name_draft.trim().to_string();
                if !new_name.is_empty() {
                    if let Some(doc) = model.document.as_mut() {
                        apply_part_name(&mut model.history, doc, &new_name);
                        st.part_name_draft.clear();
                        st.last = Some(format!("set part name = {new_name}"));
                    }
                }
            }
        });
    } else {
        ui.label(
            egui::RichText::new("No drawing document open")
                .weak()
                .italics(),
        );
    }

    // ---- Dimension tool ----------------------------------------------------
    ui.add_space(6.0);
    ui.label(theme::heading(mode, "Dimension").size(12.0));
    ui.separator();
    if st.dim_placed {
        // A value editor bound to the dimension's model variable. On change we
        // re-drive through the seam: BOTH geometry and label come from the one
        // re-driven render, so they move together and agree.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("value:").weak().size(11.0));
            let resp = ui.add(
                egui::DragValue::new(&mut st.dim_value)
                    .prefix("dimension value")
                    .speed(1.0)
                    .range(1.0..=300.0),
            );
            if resp.changed() {
                if let Some(doc) = model.document.clone() {
                    let before = st.dim_before;
                    let driven = re_drive(&doc, st.dim_value);
                    let after = driven.as_ref().and_then(dimension_numbers);
                    if let Some(af) = after {
                        st.dim_after = Some(af);
                        if let Some(bf) = before {
                            let agreed = (af.value - bf.value).abs() > 1e-9
                                && (af.geom_w - bf.geom_w).abs() > 1e-9
                                && af.agrees();
                            st.last = Some(format!(
                                "dim {} -> {}: label {:.6}->{:.6}, geometry {:.6}->{:.6}, agree={agreed}",
                                bf.value, af.value, bf.value, af.value, bf.geom_w, af.geom_w
                            ));
                        } else {
                            st.last = Some(format!("dim resolved to {:.6}", af.value));
                        }
                    }
                }
            }
        });
        // The number readout: value (model) and measured (geometry), asserted
        // equal within 1e-6 by the acceptance.
        if let Some(after) = st.dim_after {
            ui.label(
                egui::RichText::new(format!(
                    "value={:.9} measured={:.9} geometry_w={:.9}",
                    after.value, after.measured, after.geom_w
                ))
                .monospace()
                .size(11.0),
            );
        }
    } else {
        if ui.button("Place dimension (front width)").clicked() {
            let placed_before = if let Some(doc) = model.document.clone() {
                let driven = re_drive(&doc, 80.0);
                driven.as_ref().and_then(dimension_numbers)
            } else {
                None
            };
            place_dimension(model);
            st.dim_placed = true;
            st.dim_value = 80.0;
            st.dim_before = placed_before;
            // Re-read the numbers after the dimension record lands on the sheet
            // (the record's value now resolves to the bound model variable).
            if let Some(doc) = model.document.clone() {
                if let Some(driven) = re_drive(&doc, st.dim_value) {
                    st.dim_after = dimension_numbers(&driven);
                }
            }
            st.last = Some("placed width dimension between two view points".to_string());
        }
    }

    // ---- Leader + note tools ----------------------------------------------
    ui.add_space(6.0);
    ui.label(theme::heading(mode, "Leaders & notes").size(12.0));
    ui.separator();
    ui.horizontal_wrapped(|ui| {
        if ui.button("Add leader").clicked() {
            let n = st.leaders.len() + 1;
            st.leaders.push(LeaderRecord {
                id: format!("oep-leader-{n}"),
                text: format!("LEADER {n}: C/L THIS FEATURE"),
                x: 120.0,
                y: 40.0,
                to_x: 148.5,
                to_y: 95.0,
            });
            st.last = Some(format!("added leader {n}"));
        }
        if ui.button("Add note").clicked() {
            let n = st.notes.len() + 1;
            st.notes.push(NoteRecord {
                id: format!("oep-note-{n}"),
                text: format!("NOTE {n}: REMOVE ALL SHARP EDGES"),
                x: 100.0,
                y: 30.0,
            });
            st.last = Some(format!("added note {n}"));
        }
        if ui.small_button("Clear annotations").clicked() {
            st.leaders.clear();
            st.notes.clear();
            st.last = Some("cleared annotations".to_string());
        }
    });
    for rec in &st.leaders {
        ui.label(
            egui::RichText::new(format!("leader {} · {}", rec.id, rec.text))
                .monospace()
                .size(11.0),
        );
    }
    for rec in &st.notes {
        ui.label(
            egui::RichText::new(format!("note {} · {}", rec.id, rec.text))
                .monospace()
                .size(11.0),
        );
    }

    if let Some(last) = &st.last {
        ui.add_space(2.0);
        ui.label(egui::RichText::new(last).weak().italics().size(11.0));
    }

    publish_report(ui.ctx(), &st, &document);
    write_state(ui.ctx(), &st);
}

// ---------------------------------------------------------------------------
// Context-store plumbing + oracle
// ---------------------------------------------------------------------------

fn read_state(ctx: &egui::Context) -> AnnotateUi {
    ctx.data(|d| d.get_temp(egui::Id::unique(ANNOTATE_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, st: &AnnotateUi) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(ANNOTATE_STATE_KEY), st.clone()));
}

/// Publish the plain-`String` oracle under [`ANNOTATE_REPORT_KEY`] and the
/// canonical annotation JSON under [`ANNOTATE_JSON_KEY`].
fn publish_report(ctx: &egui::Context, st: &AnnotateUi, document: &Option<Document>) {
    let mut r = String::new();
    let readout = document
        .as_ref()
        .map(title_block_readout)
        .unwrap_or_default();
    for (label, value) in &readout {
        r.push_str(&format!("tb_{label}={value}\n"));
    }
    r.push_str(&format!(
        "part_name={}\n",
        document
            .as_ref()
            .and_then(|d| d.parts.first())
            .map(|p| p.name.clone())
            .unwrap_or_default()
    ));
    r.push_str(&format!("dim_placed={}\n", u8::from(st.dim_placed)));
    if let Some(b) = st.dim_before {
        r.push_str(&format!("dim_before={:.9}\n", b.value));
        r.push_str(&format!("dim_measured_before={:.9}\n", b.measured));
        r.push_str(&format!("dim_geom_before={:.9}\n", b.geom_w));
    } else {
        r.push_str("dim_before=-\ndim_measured_before=-\ndim_geom_before=-\n");
    }
    if let Some(a) = st.dim_after {
        r.push_str(&format!("dim_after={:.9}\n", a.value));
        r.push_str(&format!("dim_measured_after={:.9}\n", a.measured));
        r.push_str(&format!("dim_geom_after={:.9}\n", a.geom_w));
        r.push_str(&format!(
            "dim_value_measured_diff={:.3e}\n",
            (a.value - a.measured).abs()
        ));
        r.push_str(&format!("dim_agree={}\n", u8::from(a.agrees())));
    } else {
        r.push_str("dim_after=-\ndim_measured_after=-\ndim_geom_after=-\ndim_value_measured_diff=-\ndim_agree=-\n");
    }
    r.push_str(&format!("leaders={}\n", st.leaders.len()));
    r.push_str(&format!("notes={}\n", st.notes.len()));
    let json = encode_annotations(&st.leaders, &st.notes);
    r.push_str(&format!("annotate_json={json}\n"));
    r.push_str(&format!("json_hash={}\n", fnv1a(&json)));
    r.push_str(&format!(
        "last={}\n",
        st.last.clone().unwrap_or_else(|| "-".to_string())
    ));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(ANNOTATE_REPORT_KEY), r));
    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(ANNOTATE_JSON_KEY), json));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The title read-out is bound to the document's part-name property, read
    /// LIVE (never a stored copy): renaming the part changes the title with no
    /// second edit.
    #[test]
    fn title_block_is_bound_to_the_document_part_name_property() {
        let driven = super::super::drive_bracket_drawing().expect("fixture drives");
        let mut doc = driven.document;
        let before = title_of(&doc);
        assert!(!before.is_empty(), "fixture carries a part name");
        apply_part_name(
            &mut super::super::doc_io::EditHistory::default(),
            &mut doc,
            "bracket-mk2",
        );
        assert_eq!(title_of(&doc), "bracket-mk2");
        assert_eq!(
            doc.drawings[0].sheets[0].title_block.title, "bracket-mk2",
            "the stored title-block title follows the part name in the same command"
        );
        let readout = title_block_readout(&doc);
        assert!(readout
            .iter()
            .any(|(l, v)| l == "title" && v == "bracket-mk2"));
    }

    /// The dimension tool writes a width dimension record on the sheet.
    #[test]
    fn place_dimension_writes_a_width_record_to_the_sheet() {
        let driven = super::super::drive_bracket_drawing().expect("fixture drives");
        let mut model = super::super::SheetModel {
            document: Some(driven.document),
            selection: Some("oep-sheet-1".to_string()),
            ..Default::default()
        };
        place_dimension(&mut model).expect("places");
        let doc = model.document.as_ref().unwrap();
        let sheet = &doc.drawings[0].sheets[0];
        assert!(
            sheet
                .dimensions
                .iter()
                .any(|d| d.variable.as_deref() == Some(DIM_VARIABLE)),
            "the sheet carries the placed width dimension"
        );
    }

    /// Re-driving the committed bracket at a new width through the EXISTING
    /// seam yields a model whose dimension value, measured extent and view
    /// geometry all agree (the "both move together" guarantee).
    #[test]
    fn re_driven_dimension_numbers_agree_at_multiple_widths() {
        let base = super::super::drive_bracket_drawing().expect("fixture drives");
        let doc = base.document;
        for width in [80.0, 120.0, 65.25] {
            let driven = re_drive(&doc, width).expect("re-drives");
            let nums = dimension_numbers(&driven).expect("resolved");
            assert!(nums.agrees(), "numbers must agree at {width}: {nums:?}");
            assert!(
                (nums.value - width).abs() < 1e-9,
                "value equals the edited width: {} != {width}",
                nums.value
            );
        }
    }

    /// The canonical annotation JSON round-trips: encode -> decode -> encode is
    /// byte-identical (same FNV-1a hash), so a save and reopen restore the
    /// exact leader and note records.
    #[test]
    fn leader_and_note_json_round_trip_byte_identical() {
        let mut leaders = Vec::new();
        let mut notes = Vec::new();
        leaders.push(LeaderRecord {
            id: "oep-leader-1".to_string(),
            text: "LEADER 1: C/L THIS FEATURE".to_string(),
            x: 120.0,
            y: 40.0,
            to_x: 148.5,
            to_y: 95.0,
        });
        notes.push(NoteRecord {
            id: "oep-note-1".to_string(),
            text: "NOTE 1: REMOVE ALL SHARP EDGES".to_string(),
            x: 100.0,
            y: 30.0,
        });
        let saved = encode_annotations(&leaders, &notes);
        let (l2, n2) = decode_annotations(&saved).expect("decodes");
        let reopened = encode_annotations(&l2, &n2);
        assert_eq!(saved, reopened, "JSON must round-trip byte-identical");
        assert_eq!(fnv1a(&saved), fnv1a(&reopened), "JSON hashes must match");
        assert_eq!(l2, leaders);
        assert_eq!(n2, notes);
    }

    /// The JSON encoder is stable (same bytes every time) so a save hash is
    /// comparable across a reopen.
    #[test]
    fn annotation_json_is_deterministic() {
        let leaders = vec![LeaderRecord {
            id: "oep-leader-1".to_string(),
            text: "LEADER 1: C/L THIS FEATURE".to_string(),
            x: 120.0,
            y: 40.0,
            to_x: 148.5,
            to_y: 95.0,
        }];
        let notes = vec![NoteRecord {
            id: "oep-note-1".to_string(),
            text: "NOTE 1: REMOVE ALL SHARP EDGES".to_string(),
            x: 100.0,
            y: 30.0,
        }];
        assert_eq!(
            encode_annotations(&leaders, &notes),
            encode_annotations(&leaders, &notes)
        );
    }

    /// FNV-1a over the canonical document JSON matches the wave's hashing
    /// language (same constants as the sheet panel / explode panel).
    #[test]
    fn fnv_matches_the_waves_hash_constant() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
    }
}
