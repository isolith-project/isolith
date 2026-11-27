//! Exploded view control (wave E, card UI-E4 UI-ASSY-EXPLODE): an explode
//! slider with per-component offsets that moves components predictably and is
//! saved with the document, so the exploded state round-trips through a
//! save/reopen cycle.
//!
//! ## What this panel is
//!
//! A `#[path]` child module of `assy_tree` (exactly like the mate tool and the
//! diagnostics panel): `assy_tree::show` pulls it in with one `#[path]`
//! include and renders it with ONE call. Because it is a child module of
//! `assy_tree`, it reaches the parent's private [`super::AssemModel`] (the
//! live canonical document and journal) directly - one source of truth.
//!
//! ## The behaviour (the card's acceptance, with numbers in the test)
//!
//! - **The explode slider with per-component offsets.** A single explode
//!   factor scales every component's committed offset; each per-component row
//!   shows and edits that component's own offset vector (`x`/`y`/`z` drag
//!   values). Moving the slider (or editing one offset) recomputes every
//!   placement from the committed BASE placement plus `factor * offset` and
//!   journals the result through the shared document journal - one undoable
//!   command, byte-exact hashes, exactly like the mate tool writes placements.
//! - **Predictable movement.** At factor 0 the panel writes nothing (it skips
//!   the write entirely), so every transform is byte-identical to the
//!   unexploded document. At factor 1 each component is moved by exactly its
//!   committed offset, at factor 2 by exactly twice it - printed per component
//!   as base (`before_`), exploded (`pos_`) and the offset used (`off_`).
//! - **A per-component override moves only that component.** Editing one
//!   component's offset recomputes only its placement; the other components'
//!   transforms (and the document hash they imply) are byte-identical before
//!   and after (printed as their unchanged positions).
//! - **Saved with the document, so it round-trips.** [`saved_payload`] is the
//!   card's save hook: the canonical `.oepdoc` JSON with the explode state
//!   (factor + offsets + the committed base placements) embedded as one extra
//!   top-level `explode` member that an oep-geo reader ignores. Saving the
//!   document therefore writes the exploded positions (they are the instance
//!   placements) AND the controls that produced them; reopening restores both
//!   byte-identically ([`seed_state`] recovers the member; the restored factor
//!   and the reopened document JSON hash are both byte-equal to the saved
//!   side).
//!
//! ## The oracle
//!
//! Every frame the panel publishes a plain-`String` report under
//! [`EXPLODE_REPORT_KEY`] (`factor`, per-component `pos_`/`off_`/`before_`,
//! the document `hash`, the saved-payload `saved_hash`, `err`), the full
//! saved payload under [`EXPLODE_SAVED_JSON_KEY`] and the encoded explode
//! state under [`EXPLODE_STATE_JSON_KEY`], so every acceptance clause is
//! provable with numbers read back from the running panel.

use eframe::egui;

use crate::app::App;
use crate::theme;

/// Context-data key of the explode panel's published `String` oracle (tests
/// read it by value, like every other panel oracle in this wave).
pub const EXPLODE_REPORT_KEY: &str = "ui_e4_explode_report";
/// Context-data key under which the panel publishes the full saved `.oepdoc`
/// payload (canonical document JSON + embedded `explode` member) every frame -
/// the bytes a save of the current document would write.
pub const EXPLODE_SAVED_JSON_KEY: &str = "ui_e4_explode_saved_json";
/// Context-data key under which the panel publishes the encoded explode state
/// (factor + per-component offsets + base placements) every frame.
pub const EXPLODE_STATE_JSON_KEY: &str = "ui_e4_explode_state_json";
/// Context-data key a test may seed with an encoded explode state to be
/// applied before the first frame, exactly like the tree's seed key seeds the
/// document. Absent, untouched radial defaults are derived.
pub const EXPLODE_SEED_KEY: &str = "ui_e4_explode_seed";
/// Context-data key of the panel's own persistent ui state.
const EXPLODE_STATE_KEY: &str = "ui_e4_explode_state";
/// Journal token this panel records for an explode write (one undoable command
/// per gesture, same journal the tree and mate tool share).
pub const EXPLODE_TOKEN: &str = "assy:explode";
/// Version tag of the explode-state encoding (bump only with the decoder).
const STATE_VERSION: &str = "v1";
/// Default explode distance in document units (mm) for a component with no
/// committed offset.
const DEFAULT_EXPLODE_MM: f64 = 25.0;

/// Per-instance `(x, y, z)` offset / base placement map, keyed by instance id.
type OffsetMap = std::collections::BTreeMap<String, [f64; 3]>;

/// The panel's live state, resident in the egui context store.
#[derive(Debug, Clone, Default)]
struct ExplodeUi {
    /// The explode factor (slider, 0.0 = unexploded).
    factor: f64,
    /// Committed per-component offset vectors (each component's own explode
    /// direction and distance), keyed by instance id.
    offsets: OffsetMap,
    /// Committed baseline placements (unexploded translations), keyed by
    /// instance id. Every explode write recomputes `base + factor * offset`
    /// from these - never accumulated.
    base: OffsetMap,
    /// True once base/offsets have been initialised for the document.
    seeded: bool,
    /// True when the document opened already-exploded (the explode state came
    /// from the saved payload), so the panel must NOT re-apply the current
    /// factor on this frame - the placements are already in the instances.
    restored: bool,
    /// Error text of the last operation (empty = clean).
    err: String,
}

// --- explode state encoding (self-contained, no JSON crate) ---
//
// The explode state travels inside the saved document JSON as an ignored
// top-level string member: `"explode": "v1|factor<f>|o:<id2>:x,y,z|b:<id2>:x,y,z"`.
// The encoding is character-safe (letters, digits, `.:,|+-`), so it embeds as
// a JSON string byte-identically without a JSON library. Floats use the
// shortest round-trip form (`{}`), so format then parse recovers the exact
// same `f64`.
//
// Tokens (separator `|`):
//   `factor<f64>`            - the explode factor
//   `o:<id>:x,y,z`           - one committed offset vector for an instance
//   `b:<id>:x,y,z`           - one committed base placement for an instance

/// Factor + per-instance offset + per-instance base, as stored in the state
/// encoding.
type ExplodeState = (
    f64,
    std::collections::BTreeMap<String, [f64; 3]>,
    std::collections::BTreeMap<String, [f64; 3]>,
);

/// Encode factor + offsets + base placements.
fn encode_state(factor: f64, offsets: &OffsetMap, base: &OffsetMap) -> String {
    let mut out = String::from(STATE_VERSION);
    out.push_str("|factor");
    out.push_str(&format!("{factor}"));
    for (id, o) in offsets {
        out.push_str("|o:");
        out.push_str(id);
        out.push(':');
        out.push_str(&format!("{},{},{}", o[0], o[1], o[2]));
    }
    for (id, b) in base {
        out.push_str("|b:");
        out.push_str(id);
        out.push(':');
        out.push_str(&format!("{},{},{}", b[0], b[1], b[2]));
    }
    out
}

/// Decode the explode state back into its three parts. `None` on any
/// malformed token (never panics on foreign bytes).
fn decode_state(text: &str) -> Option<ExplodeState> {
    let mut parts = text.split('|');
    if parts.next()? != STATE_VERSION {
        return None;
    }
    let mut factor = 0.0;
    let mut offsets = OffsetMap::new();
    let mut base = OffsetMap::new();
    for part in parts {
        if let Some(f) = part.strip_prefix("factor") {
            factor = f.parse::<f64>().ok()?;
            continue;
        }
        let is_off = part.starts_with("o:");
        let rest = if is_off {
            part.strip_prefix("o:")?
        } else {
            part.strip_prefix("b:")?
        };
        let (id, vec) = rest.split_once(':')?;
        let mut it = vec.split(',');
        let x = it.next()?.parse::<f64>().ok()?;
        let y = it.next()?.parse::<f64>().ok()?;
        let z = it.next()?.parse::<f64>().ok()?;
        if it.next().is_some() {
            return None;
        }
        if is_off {
            offsets.insert(id.to_string(), [x, y, z]);
        } else {
            base.insert(id.to_string(), [x, y, z]);
        }
    }
    Some((factor, offsets, base))
}

/// The `"explode": "..."` member fragment for `state` (the encoding never
/// contains a double quote or backslash, so no JSON escaping is needed).
fn explode_member(state: &str) -> String {
    format!("\"explode\": \"{state}\"")
}

/// Embed the encoded explode state into a canonical `.oepdoc` JSON document,
/// producing the full saved payload: the document JSON plus one ignored
/// top-level `explode` member. A plain oep-geo reader still opens this.
///
/// The canonical serializer emits `...}\n`; the member is spliced in before
/// that final top-level closing brace.
fn embed_explode_member(document_json: &str, state: &str) -> String {
    let trimmed = document_json.trim_end_matches('\n');
    let Some(pos) = trimmed.rfind('}') else {
        return document_json.to_string();
    };
    let mut out = String::with_capacity(trimmed.len() + state.len() + 24);
    out.push_str(&trimmed[..pos]);
    out.push_str(",\n  ");
    out.push_str(&explode_member(state));
    out.push_str(&trimmed[pos..]);
    out.push('\n');
    out
}

/// Recover the encoded explode state member from a saved payload, if present.
/// `None` when there is no `explode` member (a plain unexploded document) or
/// the member is not well-formed.
fn extract_explode_member(saved_json: &str) -> Option<String> {
    let marker = "\"explode\": \"";
    let start = saved_json.find(marker)? + marker.len();
    let rest = &saved_json[start..];
    let end = rest.find('"')?;
    let enc = &rest[..end];
    decode_state(enc)?;
    Some(enc.to_string())
}

// --- geometry helpers ---

/// The ordered instance ids of the live assembly, in document order.
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

/// The translation triplets of every instance, keyed by id.
fn base_translations(model: &super::AssemModel) -> std::collections::BTreeMap<String, [f64; 3]> {
    let mut out = std::collections::BTreeMap::new();
    if let Some(doc) = &model.document {
        if let Some(asm) = doc.assemblies.first() {
            for inst in &asm.instances {
                out.insert(
                    inst.id.clone(),
                    [inst.transform[12], inst.transform[13], inst.transform[14]],
                );
            }
        }
    }
    out
}

/// The assembly centroid: the mean of the instance placement translations.
fn centroid(base: &std::collections::BTreeMap<String, [f64; 3]>) -> [f64; 3] {
    let n = base.len().max(1) as f64;
    let mut c = [0.0; 3];
    for v in base.values() {
        for i in 0..3 {
            c[i] += v[i];
        }
    }
    [c[0] / n, c[1] / n, c[2] / n]
}

/// A unit vector from the assembly centroid out to `pos`. A component sitting
/// exactly on the centroid falls back to +Z so every component still has a
/// non-degenerate own direction.
fn radial_direction(pos: [f64; 3], c: [f64; 3]) -> [f64; 3] {
    let mut v = [pos[0] - c[0], pos[1] - c[1], pos[2] - c[2]];
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 1e-9 {
        for x in &mut v {
            *x /= len;
        }
        v
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// Default committed offsets when none are provided: every component explodes
/// radially outwards from the assembly centroid by [`DEFAULT_EXPLODE_MM`] - a
/// genuine "explode" (parts pull apart) with each component along its own
/// direction.
fn default_offsets(
    base: &std::collections::BTreeMap<String, [f64; 3]>,
) -> std::collections::BTreeMap<String, [f64; 3]> {
    let c = centroid(base);
    let mut offsets = std::collections::BTreeMap::new();
    for (id, pos) in base {
        let d = radial_direction(*pos, c);
        offsets.insert(
            id.clone(),
            [
                d[0] * DEFAULT_EXPLODE_MM,
                d[1] * DEFAULT_EXPLODE_MM,
                d[2] * DEFAULT_EXPLODE_MM,
            ],
        );
    }
    offsets
}

/// The exploded translation for one component: committed base placement plus
/// `factor` times its committed offset (never accumulated).
fn exploded_translation(base: [f64; 3], offset: [f64; 3], factor: f64) -> [f64; 3] {
    [
        base[0] + factor * offset[0],
        base[1] + factor * offset[1],
        base[2] + factor * offset[2],
    ]
}

/// Write the exploded placements for the current factor into the document
/// through the journal - one undoable command, recomputing every placement
/// from the committed base, exactly like the mate tool writes placements.
///
/// Byte-identity guarantee at factor 0: the base translation is written back
/// VERBATIM (no `base + 0 * offset` arithmetic, which could flip a `-0.0`),
/// so every transform round-trips to the unexploded document byte-for-byte
/// and the document hash equals the unexploded hash.
///
/// After writing, the document is CANONICALIZED (re-parsed through its own
/// canonical serializer, exactly like `assy_mates::canonicalize_doc`): the
/// engine's shortest-round-trip float image is applied to computed placements
/// so that a save -> reopen of this document is byte-identical to what was
/// saved (without it, serde's shortest form of a freshly-computed f64 can
/// differ by one ulp from the parsed-back document, breaking the round-trip).
fn apply_explode(model: &mut super::AssemModel, st: &ExplodeUi) {
    // Snapshot the target placements before borrowing the document mutably.
    let mut placements: std::collections::BTreeMap<String, [f64; 3]> =
        std::collections::BTreeMap::new();
    for (id, b) in &st.base {
        let off = st.offsets.get(id).copied().unwrap_or([0.0; 3]);
        let t = if st.factor == 0.0 {
            *b // verbatim base: byte-identical to the unexploded placement
        } else {
            exploded_translation(*b, off, st.factor)
        };
        placements.insert(id.clone(), t);
    }
    let Some(document) = model.document.as_mut() else {
        return;
    };
    model.history.apply(document, EXPLODE_TOKEN, None, |d| {
        if let Some(asm) = d.assemblies.first_mut() {
            for inst in &mut asm.instances {
                if let Some(t) = placements.get(&inst.id) {
                    inst.transform[12] = t[0];
                    inst.transform[13] = t[1];
                    inst.transform[14] = t[2];
                }
            }
        }
        // Canonicalize so the saved/reopened document is byte-identical (the
        // journal's before/after hashes then also match the stored snapshots).
        if let Ok(json) = d.to_json_pretty() {
            if let Ok(canonical) = oep_geo::Document::from_json_pretty(&json) {
                *d = canonical;
            }
        }
    });
}

// --- context-store plumbing ---

fn read_state(ctx: &egui::Context) -> ExplodeUi {
    ctx.data(|d| d.get_temp::<ExplodeUi>(egui::Id::unique(EXPLODE_STATE_KEY)))
        .unwrap_or_default()
}

fn write_state(ctx: &egui::Context, st: &ExplodeUi) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(EXPLODE_STATE_KEY), st.clone()));
}

/// The current canonical document JSON, if a document is open.
fn document_json(model: &super::AssemModel) -> Option<String> {
    model.document.as_ref()?.to_json_pretty().ok()
}

/// FNV-1a-64 over arbitrary bytes - the same scheme `doc_io::document_hash`
/// uses for the canonical document bytes, so hashing the saved payload here is
/// the same hashing language the wave already speaks, never a second scheme.
fn text_hash(text: &str) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for &byte in text.as_bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// FNV-1a-64 over one instance's 16 transform doubles (raw IEEE bits) - the
/// per-component hash the override clause reads: when only one component's
/// offset is overridden, the OTHER components' transform hashes must be
/// unchanged, byte-identical.
fn transform_hash(t: &oep_geo::Transform) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET;
    for c in t.0 {
        let bits = c.to_bits();
        for &byte in &[
            bits.to_le_bytes()[0],
            bits.to_le_bytes()[1],
            bits.to_le_bytes()[2],
            bits.to_le_bytes()[3],
            bits.to_le_bytes()[4],
            bits.to_le_bytes()[5],
            bits.to_le_bytes()[6],
            bits.to_le_bytes()[7],
        ] {
            h ^= u64::from(byte);
            h = h.wrapping_mul(PRIME);
        }
    }
    h
}

/// Per-instance transform hash, keyed by instance id, read straight from the
/// live document (the source of truth for what is placed).
fn transform_hashes(model: &super::AssemModel) -> std::collections::BTreeMap<String, u64> {
    let mut out = std::collections::BTreeMap::new();
    if let Some(doc) = &model.document {
        if let Some(asm) = doc.assemblies.first() {
            for inst in &asm.instances {
                out.insert(inst.id.clone(), transform_hash(&inst.transform));
            }
        }
    }
    out
}

/// The full saved `.oepdoc` payload for the current state: the canonical
/// document JSON with the explode state embedded as an ignored member. This is
/// the card's save hook - the exact bytes a save would write, so the exploded
/// positions (already in the instance placements) AND the controlling factor /
/// offsets reopen together. Published every frame under
/// [`EXPLODE_SAVED_JSON_KEY`] so tests (and any future save wiring) read the
/// exact bytes without reaching into this panel's private type.
fn saved_payload(model: &super::AssemModel, st: &ExplodeUi) -> Option<String> {
    let doc_json = document_json(model)?;
    Some(embed_explode_member(
        &doc_json,
        &encode_state(st.factor, &st.offsets, &st.base),
    ))
}

/// Seed the explode panel on the first frame: restore state from the saved
/// payload's embedded `explode` member (reopening an exploded document - the
/// placements are already in the instances, so `restored = true`), else from
/// an explicit [`EXPLODE_SEED_KEY`], else derive radial defaults for a fresh
/// document (factor 0, nothing moved).
fn seed_state(ctx: &egui::Context, model: &mut super::AssemModel, st: &mut ExplodeUi) {
    let seeded_state: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(EXPLODE_SEED_KEY)));
    let raw_seed: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(super::ASSY_SEED_KEY)));

    st.base = base_translations(model);
    if let Some(enc) = extract_explode_member(raw_seed.as_deref().unwrap_or("")) {
        // Reopening an exploded document: recover the controls. The placements
        // that produced the current instances are already there.
        if let Some((factor, offsets, base)) = decode_state(&enc) {
            st.factor = factor;
            st.offsets = offsets;
            if !base.is_empty() {
                st.base = base;
            }
            st.restored = true;
            st.seeded = true;
        }
    } else if let Some(enc) = seeded_state {
        if let Some((factor, offsets, _)) = decode_state(&enc) {
            st.factor = factor;
            st.offsets = offsets;
            st.seeded = true;
        }
    } else {
        st.factor = 0.0;
        st.offsets = default_offsets(&st.base);
        st.seeded = true;
    }
    // Any instance that has a base but no committed offset still gets its own
    // radial explode direction (so every component moves along its own
    // direction even when a seeded state names only some of them).
    let defaults = default_offsets(&st.base);
    for id in st.base.keys() {
        st.offsets.entry(id.clone()).or_insert_with(|| defaults[id]);
    }
}

/// Publish the plain-`String` oracle plus the saved payload / encoded state
/// every frame.
fn publish_report(ctx: &egui::Context, model: &super::AssemModel, st: &ExplodeUi) {
    let hash = model
        .document
        .as_ref()
        .map(super::doc_io::document_hash)
        .unwrap_or(0);
    let ids = instance_ids(model);
    let thashes = transform_hashes(model);
    let placed = base_translations(model);

    let mut report = String::new();
    report.push_str(&format!("factor={}\n", st.factor));
    for id in &ids {
        let base = st.base.get(id).copied().unwrap_or([0.0; 3]);
        let off = st.offsets.get(id).copied().unwrap_or([0.0; 3]);
        // The ACTUAL document placement after any explode write (source of
        // truth: what the test compares against `before + factor * offset`).
        let pos = placed.get(id).copied().unwrap_or(base);
        let expected = exploded_translation(base, off, st.factor);
        report.push_str(&format!("pos_{id}={},{},{}\n", pos[0], pos[1], pos[2]));
        report.push_str(&format!(
            "exp_{id}={},{},{}\n",
            expected[0], expected[1], expected[2]
        ));
        report.push_str(&format!("off_{id}={},{},{}\n", off[0], off[1], off[2]));
        report.push_str(&format!(
            "before_{id}={},{},{}\n",
            base[0], base[1], base[2]
        ));
        if let Some(th) = thashes.get(id) {
            report.push_str(&format!("thash_{id}={th}\n"));
        }
    }
    report.push_str(&format!("hash={hash}\n"));

    let saved = saved_payload(model, st);
    let saved_hash = saved.as_deref().map(text_hash).unwrap_or(0);
    report.push_str(&format!("saved_hash={saved_hash}\n"));
    if st.err.is_empty() {
        report.push_str("err=-\n");
    } else {
        report.push_str(&format!("err={}\n", st.err));
    }

    ctx.data_mut(|d| d.insert_temp::<String>(egui::Id::unique(EXPLODE_REPORT_KEY), report));
    if let Some(saved) = &saved {
        ctx.data_mut(|d| {
            d.insert_temp::<String>(egui::Id::unique(EXPLODE_SAVED_JSON_KEY), saved.clone());
        });
    }
    let enc = encode_state(st.factor, &st.offsets, &st.base);
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(EXPLODE_STATE_JSON_KEY), enc);
    });
}

/// Render the exploded-view control as a normal in-flow SECTION of the
/// assembly region, AFTER the diagnostics panel and the mate tool, so the
/// fixed-height assembly region keeps the layout E2/E3 already has (the mate
/// tool's pinned "+ Add mate" row stays exactly where the E2/E3 tests click
/// it) and the explode section simply extends below. One call from
/// `assy_tree::show`; it mutates the SAME journaled document the tree and
/// the mate tool already use, so a saved/exploded round trip publishes from
/// the same source of truth.
pub fn show(ui: &mut egui::Ui, app: &mut App, model: &mut super::AssemModel) {
    let _ = app;
    let mode = theme::mode_of(ui);
    ui.separator();
    ui.label(theme::heading(mode, "Explode").size(13.0));

    let mut st = read_state(ui.ctx());
    if !st.seeded {
        seed_state(ui.ctx(), model, &mut st);
        // A fresh seed with a non-zero factor is applied so the fixture moves
        // on open; a reopened (restored) document never re-applies.
        if !st.restored && st.factor != 0.0 {
            apply_explode(model, &st);
        }
    }

    let mut changed_factor = false;
    let mut changed_offsets: Vec<(String, [f64; 3])> = Vec::new();

    // --- the explode factor slider (one compact row with the value) ---
    ui.horizontal_wrapped(|ui| {
        let resp = ui.add(
            egui::Slider::new(&mut st.factor, 0.0..=2.0)
                .text("explode factor")
                .show_value(false),
        );
        changed_factor = resp.changed();
        ui.label(
            egui::RichText::new(format!("{:.1}", st.factor))
                .monospace()
                .size(12.0),
        );
    });

    // --- per-component offset editors (the card's "per-component offsets") ---
    // Compact and overflow-proof: the offset editors render ONE axis per line
    // inside a short bounded ScrollArea. A DragValue with its long prefixed
    // name spans most of the 240px column, so three-of-a-row would overflow
    // horizontally and push the mate tool's pinned rows OUTSIDE the region
    // column E2/E3 click in; the bound keeps the section's vertical footprint
    // tiny so the fixed-height assembly region still shows Mates after it.
    let ids = instance_ids(model);
    egui::ScrollArea::vertical()
        .id_salt("ui_e4_explode_offsets")
        .max_height(64.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for id in &ids {
                let off = st.offsets.entry(id.clone()).or_insert([0.0; 3]);
                let mut new_off = *off;
                ui.label(egui::RichText::new(format!("offset {id}:")).weak());
                let mut changed_this = false;
                for (axis, slot) in new_off.iter_mut().enumerate() {
                    let resp = ui.add(
                        egui::DragValue::new(slot)
                            .prefix(format!("explode offset {id} {axis}"))
                            .speed(1.0),
                    );
                    if resp.changed() {
                        changed_this = true;
                    }
                }
                if changed_this {
                    changed_offsets.push((id.clone(), new_off));
                }
            }
        });

    // --- apply a UI change as one journaled explode write ---
    if changed_factor || !changed_offsets.is_empty() {
        for (id, o) in changed_offsets {
            st.offsets.insert(id, o);
        }
        // A user change always re-applies from the committed base (restored
        // base when a saved document was reopened), so the document reflects
        // the new factor/offsets exactly.
        apply_explode(model, &st);
        st.restored = false;
    }

    publish_report(ui.ctx(), model, &st);
    write_state(ui.ctx(), &st);
}
