//! Variable table panel (wave C, card UI-C3 UI-VARIABLE-TABLE).
//!
//! A variable table drives the model: expressions resolve through the EXISTING
//! UI-B7 expression path ([`dimensions::resolve_expression`], the same scalar
//! surface and the same [`oep_sketch::varset::VariableSet`] store the B7
//! dimension editor drove - no second parser, no second variable system).
//! Editing a variable is validated against the whole table and, on success,
//! written into the document through the UI-B2 journal seam
//! ([`doc_io::EditHistory::apply`]), so every edit is one journaled command
//! and undo restores the exact prior document hash.
//!
//! ## The table is the document
//!
//! The rows live in the model document (the same canonical `oep-geo`
//! [`oep_import::Document`] the tree panel uses) as ONE `VariableSet` feature
//! whose `parameters` hold, per variable, plain string records -
//! [`VARIABLES_LIST_PARAM_KEY`] (`vars:list`, newline-separated names in
//! document order) plus one `var:expr:<name>` and `var:unit:<name>` string
//! parameter each. Every frame the visible rows are RE-DERIVED from those
//! parameters ([`parse_variables`]); there is no stored row vector to go
//! stale. A row is a name, a unit and an expression (a plain value is just an
//! expression that is a literal number); its resolved value comes from
//! resolving its expression against the other rows' resolved values.
//!
//! - **Reference resolves.** An expression names a variable and the value
//!   follows the B7 arithmetic surface through
//!   [`dimensions::resolve_expression`] against the resolved companions.
//! - **Cross references work.** [`resolve_table`] resolves the table
//!   topologically: a variable that references another waits until its
//!   dependency is resolved, so `height = width/2` and deeper chains settle in
//!   order, and the whole resolved set can drive the model
//!   ([`dimensions::driven_volume`] when the table defines `width`).
//! - **Circular references are detected and refused with a named error.** A
//!   table that can no longer be ordered topologically (a variable directly or
//!   transitively referencing itself) is refused as a whole; the error names
//!   the cycle path (`circular reference: width -> base -> width`). The edit
//!   never reaches the document.
//! - **Usage counts come from the document, not bookkeeping.** [`usage_of`]
//!   re-scans the live document every call: how many expression places (other
//!   variables' expressions and every string-form feature parameter across the
//!   document) reference the name as an identifier. Nothing is incremented by
//!   hand, so an edit that changes who references whom is reflected the next
//!   time the count is read.
//!
//! ## Dependency analysis, not a second parser
//!
//! VALUE resolution goes exclusively through the B7 surface
//! (`dimensions::resolve_expression`). To build the dependency graph, the
//! cycle path and the usage scan, this panel additionally extracts which
//! names an expression REFERENCES ([`referenced_names`]) using the same
//! identifier rule the B7 tokenizer uses; it computes no arithmetic of its
//! own - it answers "which variables does this expression mention?", which
//! the B7 resolver does not expose.
//!
//! ## Bounds this panel respects
//!
//! - UI-A3 / UI-A12 guards: heading ink comes from [`crate::theme::heading`],
//!   no raw hex and no `Color32::from_*` here.
//! - UI-B1 mutation guard: document edits run ONLY inside
//!   [`doc_io::EditHistory::apply`]; this panel never writes a document
//!   collection directly (the one sanctioned writer is `doc_io.rs`).
//! - The panel is delivered `#[path]`-includable (the same seam pattern the
//!   B7 dimension engine and the B6 constraint toolbar use): it is NOT wired
//!   into the shell regions by this card (no file outside this card's owned
//!   list is touched), and a wiring card may host it in a region exactly as
//!   `properties.rs` hosts the dimension editor.

use eframe::egui;

use crate::app::App;
use crate::theme;

// The UI-B7 expression engine (egui-free; `src/sketch/dimensions.rs`),
// included verbatim so this panel resolves every expression value through the
// existing B7 path. `property.rs` includes the same file under its own module
// path; the two copies never share a stored typed value (nothing here stores
// a `DimensionPanel`), so no cross-copy TypeId arises.
#[allow(clippy::duplicate_mod)]
#[path = "../sketch/dimensions.rs"]
pub mod dimensions;

// The routed document seam (wave B, card UI-B2 UI-HISTORY), included verbatim
// so every table edit is one journaled command over the live model document -
// exactly the seam the tree panel (UI-C1) routes its renames/deletes through.
#[allow(dead_code, clippy::duplicate_mod)]
#[path = "../doc_io.rs"]
pub mod doc_io;

/// Context-data key under which the panel's typed model lives (this copy
/// only; tests read the `String` oracle instead).
pub const VARIABLES_STATE_KEY: &str = "ui_c3_variables_state";
/// Context-data key under which the panel publishes its plain-`String` report
/// every frame (rows, resolved values, units, expressions, usage counts,
/// driven volume, last named error) - the cross-copy-safe oracle.
pub const VARIABLES_REPORT_KEY: &str = "ui_c3_variables_report";
/// Context-data key a test may seed with the canonical `.oepdoc` JSON of the
/// document whose variable table the panel should show before its first
/// frame. Absent, a fresh document with one empty VariableSet feature is
/// opened (the panel's own honest default).
pub const VARIABLES_SEED_KEY: &str = "ui_c3_variables_seed";
/// The ONE shared resolved-set seam: a plain `String` ("name=value" lines,
/// sorted) under this key, published every frame the table resolves, so the
/// viewport / properties / dimension panels (and any future card) read the
/// same values the table drives.
pub const VARIABLES_RESOLVED_KEY: &str = "isolith.ui.variables.resolved";

/// The feature kind the table lives in (matches the tree panel's VariableSet
/// row kind).
pub const VARIABLES_FEATURE_KIND: &str = "VariableSet";
/// The document-wide id of the panel's variable-set feature (born with the
/// default document, present in a seeded one that carries a table).
pub const VARIABLES_FEATURE_ID: &str = "oep-feature-variables";
/// The parameter key holding the ordered list of variable names (newline-
/// separated; names are identifier-safe so the list needs no escaping).
pub const VARIABLES_LIST_PARAM_KEY: &str = "vars:list";
/// The parameter-key prefix for one variable's expression.
const EXPR_PARAM_PREFIX: &str = "var:expr:";
/// The parameter-key prefix for one variable's unit.
const UNIT_PARAM_PREFIX: &str = "var:unit:";

/// The parameter key holding one variable's expression (`var:expr:<name>`).
pub fn expr_param_key(name: &str) -> String {
    format!("{EXPR_PARAM_PREFIX}{name}")
}

/// The parameter key holding one variable's unit (`var:unit:<name>`).
pub fn unit_param_key(name: &str) -> String {
    format!("{UNIT_PARAM_PREFIX}{name}")
}

/// Journal tokens this panel records (the tree: family owns rename/delete;
/// the vars: family is this table's editing language).
pub const EDIT_TOKEN: &str = "vars:edit";
/// Journal token for adding a variable.
pub const ADD_TOKEN: &str = "vars:add";
/// Journal token for removing a variable.
pub const REMOVE_TOKEN: &str = "vars:remove";

/// Fixed widget sizes of the variable table (the test clicks the same rects
/// via [`row_rects`] / [`add_rects`]).
const ROW_H: f32 = 26.0;
const EXPR_W: f32 = 200.0;
const NAME_W: f32 = 90.0;
const VALUE_W: f32 = 72.0;
const UNIT_W: f32 = 40.0;
const USES_W: f32 = 40.0;
const BTN_W: f32 = 52.0;
const LABEL_H: f32 = 22.0;

/// One row of the variable table, derived live from the document's
/// VariableSet feature every frame.
#[derive(Debug, Clone, PartialEq)]
pub struct VarEntry {
    /// The variable's name (also its expression identifier).
    pub name: String,
    /// The unit the value is expressed in (e.g. `mm`, `deg`; informational
    /// like the document's own `units` field).
    pub unit: String,
    /// The expression text (a literal number for a plain value).
    pub expr: String,
}

/// A named, typed failure of the variable-table engine. Nothing here is a
/// panic and nothing claims a state the document did not produce.
#[derive(Debug, Clone, PartialEq)]
pub enum VariableTableError {
    /// The document has no `VariableSet` feature to hold the table.
    NoVariableSet,
    /// The table's stored record cannot be parsed (honest corruption report).
    MalformedDocument(String),
    /// The expression text cannot be resolved by the B7 surface.
    InvalidExpression(String),
    /// An expression references a name that is not a variable in the table.
    UnknownVariable(String),
    /// Adding a variable whose name already exists.
    DuplicateVariable(String),
    /// A variable name is not a usable expression identifier.
    InvalidName(String),
    /// The table cannot be ordered: the named cycle path (closed loop).
    CircularReference(Vec<String>),
    /// A resolved value is not finite.
    NotFinite(f64),
    /// Driving the model from the resolved set failed (the B7 drive lane).
    Drive(String),
}

impl std::fmt::Display for VariableTableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariableTableError::NoVariableSet => write!(f, "no variable set in the document"),
            VariableTableError::MalformedDocument(msg) => {
                write!(f, "malformed variable table: {msg}")
            }
            VariableTableError::InvalidExpression(msg) => write!(f, "invalid expression: {msg}"),
            VariableTableError::UnknownVariable(name) => write!(f, "unknown variable '{name}'"),
            VariableTableError::DuplicateVariable(name) => {
                write!(f, "variable '{name}' already exists")
            }
            VariableTableError::InvalidName(name) => {
                write!(f, "invalid variable name '{name}'")
            }
            VariableTableError::CircularReference(path) => {
                write!(f, "circular reference: {}", path.join(" -> "))
            }
            VariableTableError::NotFinite(v) => write!(f, "variable value must be finite, got {v}"),
            VariableTableError::Drive(msg) => write!(f, "could not drive the model: {msg}"),
        }
    }
}

impl std::error::Error for VariableTableError {}

/// The panel's typed model, stored in the egui context store by this copy
/// only. It owns the live model document plus the UI-B2 journal that records
/// and restores every table edit.
#[derive(Debug, Clone, Default)]
struct VariableModel {
    /// The live model document (`None` only if seeding failed).
    document: Option<oep_import::Document>,
    /// The routed, undoable edit history over that document.
    history: doc_io::EditHistory,
    /// Whether the model has been seeded from its source.
    seeded: bool,
    /// Draft expression text per row (keyed by variable name).
    drafts: std::collections::BTreeMap<String, String>,
    /// The add-variable form's name field.
    add_name: String,
    /// The add-variable form's unit field.
    add_unit: String,
    /// The add-variable form's expression field.
    add_expr: String,
    /// The last named error from a refused edit, if any.
    error: Option<VariableTableError>,
    /// One-line description of the last action (oracle readout).
    last: Option<String>,
}

fn read_model(ctx: &egui::Context) -> VariableModel {
    ctx.data(|d| d.get_temp(egui::Id::unique(VARIABLES_STATE_KEY)))
        .unwrap_or_default()
}

fn write_model(ctx: &egui::Context, model: &VariableModel) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(VARIABLES_STATE_KEY), model.clone()));
}

/// Seed the live document on the first frame: the canonical `.oepdoc` JSON a
/// test placed under [`VARIABLES_SEED_KEY`] if any (equivalent to opening
/// that document), else the panel's honest default: a fresh document with one
/// empty VariableSet feature, so a first Add has somewhere to write.
fn seed_model(ctx: &egui::Context, model: &mut VariableModel) {
    let seeded_json: Option<String> =
        ctx.data(|d| d.get_temp::<String>(egui::Id::unique(VARIABLES_SEED_KEY)));
    if let Some(json) = seeded_json {
        if let Ok(doc) = oep_import::Document::from_json_pretty(&json) {
            model.history.clear();
            model.document = Some(doc);
            model.seeded = true;
            return;
        }
    }
    model.document = Some(default_document());
    model.seeded = true;
}

/// The panel's default document: one empty VariableSet feature (built as
/// canonical JSON text, so the panel never calls a document mutator outside
/// the journal - the UI-B1 mutation guard's sanctioned writer is `doc_io.rs`).
fn default_document() -> oep_import::Document {
    let json = r#"{"format":"oep-geo","version":2,"units":"mm","parts":[],"bodies":[],"features":[{"id":"oep-feature-variables","name":"Variables","kind":"VariableSet","parameters":{"vars:list":""},"body_ids":[]}],"assemblies":[],"drawings":[]}"#;
    oep_import::Document::from_json_pretty(json)
        .expect("the panel's own default document is canonical")
}

/// The VariableSet feature of `document`, or a named error when absent.
pub fn variable_feature(
    doc: &oep_import::Document,
) -> Result<&oep_import::Feature, VariableTableError> {
    doc.features
        .iter()
        .find(|f| f.kind == VARIABLES_FEATURE_KIND)
        .ok_or(VariableTableError::NoVariableSet)
}

/// The variable rows parsed from a document's VariableSet feature, derived
/// live from the parameter record every frame (never a stored row list).
/// An absent `vars:list` is an honest empty table; an unparseable record is a
/// named corruption report, never a guess.
pub fn parse_variables(feature: &oep_import::Feature) -> Result<Vec<VarEntry>, VariableTableError> {
    let list = feature
        .parameters
        .get(VARIABLES_LIST_PARAM_KEY)
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let mut out = Vec::new();
    for name in list.lines() {
        if name.is_empty() {
            continue;
        }
        let expr = feature
            .parameters
            .get(&expr_param_key(name))
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                VariableTableError::MalformedDocument(format!("'{name}' has no expression"))
            })?;
        let unit = feature
            .parameters
            .get(&unit_param_key(name))
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        out.push(VarEntry {
            name: name.to_string(),
            unit: unit.to_string(),
            expr: expr.to_string(),
        });
    }
    Ok(out)
}

/// Build a VariableSet feature carrying `entries` as its parameter record
/// (the canonical encoding: `vars:list` plus one `var:expr:<name>` /
/// `var:unit:<name>` string parameter each), so the journal can assign the
/// whole record in one routed mutation.
fn build_table_feature(entries: &[VarEntry]) -> oep_import::Feature {
    let list = entries
        .iter()
        .map(|e| e.name.clone())
        .collect::<Vec<_>>()
        .join("\n");
    let mut feature =
        oep_import::Feature::new(VARIABLES_FEATURE_ID, "Variables", VARIABLES_FEATURE_KIND);
    feature = feature.with_param(VARIABLES_LIST_PARAM_KEY.to_string(), list);
    for e in entries {
        feature = feature.with_param(expr_param_key(&e.name), e.expr.clone());
        feature = feature.with_param(unit_param_key(&e.name), e.unit.clone());
    }
    feature
}

/// The identifiers an expression REFERENCES, in first-appearance order with
/// duplicates removed. Dependency analysis ONLY: it uses the same identifier
/// rule as the B7 surface's tokenizer to answer "which variables does this
/// expression mention?"; it never computes a value (value resolution stays
/// exclusively in [`dimensions::resolve_expression`]).
pub fn referenced_names(expr: &str) -> Vec<String> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    let b = expr.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            let name = expr[start..i].to_string();
            if seen.insert(name.clone()) {
                out.push(name);
            }
        } else {
            i += 1;
        }
    }
    out
}

/// A `VariableSet` (the same store the B7 surface resolves through) from a
/// resolved name -> value map.
fn varset_of(values: &std::collections::BTreeMap<String, f64>) -> oep_sketch::varset::VariableSet {
    let mut v = oep_sketch::varset::VariableSet::new();
    for (name, value) in values {
        v.set(name.clone(), *value);
    }
    v
}

/// Resolve a variable table: every expression evaluates through the existing
/// B7 path ([`dimensions::resolve_expression`]) against the already-resolved
/// values of the names it references, topologically (a cycle cannot be
/// ordered, so the table is invalid and the cycle is named).
///
/// `seed` hints which variable to run cycle discovery from (the variable
/// being edited); a fresh cycle that an edit introduces always includes it.
pub fn resolve_table(
    entries: &[VarEntry],
    seed: Option<&str>,
) -> Result<std::collections::BTreeMap<String, f64>, VariableTableError> {
    let mut resolved: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    let mut remaining: Vec<VarEntry> = entries.to_vec();
    loop {
        let mut progressed = false;
        let mut next: Vec<VarEntry> = Vec::new();
        for e in remaining.drain(..) {
            let deps = referenced_names(&e.expr);
            // A variable is ready only when every name it references is
            // already resolved. A name can only satisfy itself by being
            // RESOLVED - a reference to itself (directly or through a loop)
            // can never become ready, which is exactly what a cycle means.
            let ready = deps.iter().all(|d| resolved.contains_key(d));
            if ready {
                let vars = varset_of(&resolved);
                let value = dimensions::resolve_expression(&vars, &e.expr)
                    .map_err(|err| engine_error(e.name.clone(), err))?;
                if !value.is_finite() {
                    return Err(VariableTableError::NotFinite(value));
                }
                resolved.insert(e.name.clone(), value);
                progressed = true;
            } else {
                next.push(e);
            }
        }
        remaining = next;
        if !progressed {
            break;
        }
    }
    if remaining.is_empty() {
        return Ok(resolved);
    }
    // The table is invalid: either an expression names a variable that is not
    // defined at all (it can never be resolved), or the defined references
    // form a cycle.
    let defined: std::collections::BTreeSet<&str> =
        entries.iter().map(|e| e.name.as_str()).collect();
    for e in &remaining {
        for dep in referenced_names(&e.expr) {
            if dep != e.name && !defined.contains(dep.as_str()) {
                return Err(VariableTableError::UnknownVariable(dep));
            }
        }
    }
    let start = seed.unwrap_or(remaining[0].name.as_str());
    let cycle = find_cycle(&remaining, start).ok_or_else(|| {
        VariableTableError::MalformedDocument("an unresolvable table without a cycle".into())
    })?;
    Err(VariableTableError::CircularReference(cycle))
}

/// Map a B7 surface error onto the table's error vocabulary, preserving the
/// variable context where the surface reports an unknown identifier.
fn engine_error(variable: String, err: dimensions::DimensionError) -> VariableTableError {
    match err {
        dimensions::DimensionError::InvalidExpression(msg) => {
            VariableTableError::InvalidExpression(format!("{variable}: {msg}"))
        }
        dimensions::DimensionError::UnknownVariable(name) => {
            VariableTableError::UnknownVariable(name)
        }
        dimensions::DimensionError::NotFinite(v) => VariableTableError::NotFinite(v),
        e => VariableTableError::InvalidExpression(e.to_string()),
    }
}

/// Find a dependency cycle reachable from `start` among `entries`, deterministically
/// (each node advances to its lexicographically smallest defined dependency),
/// and name it as a closed path (`a -> b -> a`). `None` when `start` cannot
/// reach itself.
fn find_cycle(entries: &[VarEntry], start: &str) -> Option<Vec<String>> {
    let by_name: std::collections::BTreeMap<&str, &VarEntry> =
        entries.iter().map(|e| (e.name.as_str(), e)).collect();
    let defined: std::collections::BTreeSet<&str> =
        entries.iter().map(|e| e.name.as_str()).collect();
    let mut path: Vec<String> = Vec::new();
    let mut cur = start.to_string();
    let mut steps = 0;
    loop {
        if let Some(pos) = path.iter().position(|n| n == &cur) {
            let mut cycle = path[pos..].to_vec();
            cycle.push(cur);
            return Some(cycle);
        }
        path.push(cur.clone());
        let deps = by_name.get(cur.as_str())?;
        let mut next_dep: Option<String> = None;
        for dep in referenced_names(&deps.expr) {
            if defined.contains(dep.as_str()) {
                next_dep = Some(match next_dep {
                    Some(existing) if existing <= dep => existing,
                    Some(_) => dep,
                    None => dep,
                });
            }
        }
        let next_dep = next_dep?;
        cur = next_dep;
        steps += 1;
        if steps > entries.len() + 1 {
            return None;
        }
    }
}

/// How many places in `document` USE the variable `name`: every expression
/// place in the document - each variable's `var:expr:<name>` expression and
/// every string-form feature parameter on non-table features - that references
/// the name as an identifier. Computed from the document each call; nothing is
/// counted by hand.
pub fn usage_of(document: &oep_import::Document, name: &str) -> usize {
    let mut uses = 0usize;
    for feature in &document.features {
        for (key, value) in &feature.parameters {
            // The ordered name list and a row's unit are not expression places.
            if key == VARIABLES_LIST_PARAM_KEY || key.starts_with(UNIT_PARAM_PREFIX) {
                continue;
            }
            if let Some(text) = value.as_str() {
                if referenced_names(text).contains(&name.to_string()) {
                    uses += 1;
                }
            }
        }
    }
    uses
}

/// The driven model's volume from the resolved table (the B7 drive lane:
/// [`dimensions::driven_volume`] builds the 2:1 profile from `width`, extrudes
/// and returns the document's own mass property). `Ok(None)` when the table
/// does not define `width` (nothing to drive).
pub fn driven_volume(
    resolved: &std::collections::BTreeMap<String, f64>,
) -> Result<Option<f64>, VariableTableError> {
    if !resolved.contains_key(dimensions::WIDTH_NAME) {
        return Ok(None);
    }
    let vars = varset_of(resolved);
    dimensions::driven_volume(&vars)
        .map(Some)
        .map_err(|e| VariableTableError::Drive(e.to_string()))
}

/// One routed mutation to the table's feature record: validate the new rows
/// against the whole table, then write them through the journal ONLY on
/// success (a refused edit leaves the document untouched).
fn apply_table_edit(
    document: &mut oep_import::Document,
    history: &mut doc_io::EditHistory,
    token: &'static str,
    entries: &[VarEntry],
) {
    let entries = entries.to_vec();
    history.apply(document, token, None, |d| {
        if let Some(feature) = d
            .features
            .iter_mut()
            .find(|f| f.kind == VARIABLES_FEATURE_KIND)
        {
            let rebuilt = build_table_feature(&entries);
            feature.parameters = rebuilt.parameters;
        }
    });
}

/// Add a variable (`name`, `unit`, `expr`) to the table. The new row must
/// resolve with the rest of the table or the add is refused (named error) and
/// the document is untouched.
pub fn add_variable(
    document: &mut oep_import::Document,
    history: &mut doc_io::EditHistory,
    name: &str,
    unit: &str,
    expr: &str,
) -> Result<String, VariableTableError> {
    if !is_valid_name(name) {
        return Err(VariableTableError::InvalidName(name.to_string()));
    }
    let feature = variable_feature(document)?;
    let mut entries = parse_variables(feature)?;
    if entries.iter().any(|e| e.name == name) {
        return Err(VariableTableError::DuplicateVariable(name.to_string()));
    }
    entries.push(VarEntry {
        name: name.to_string(),
        unit: unit.to_string(),
        expr: expr.to_string(),
    });
    let resolved = resolve_table(&entries, Some(name))?;
    apply_table_edit(document, history, ADD_TOKEN, &entries);
    let value = resolved[name];
    let unit = if unit.trim().is_empty() { "" } else { unit };
    Ok(format!("added {name} ({unit}) = {value:.6}"))
}

/// Edit one variable's expression. The tentative table must resolve (a
/// circular reference introduced by the edit is refused with the named cycle,
/// an unknown reference or malformed expression with its own named error), and
/// the document is untouched unless the edit is legal.
pub fn edit_variable(
    document: &mut oep_import::Document,
    history: &mut doc_io::EditHistory,
    name: &str,
    expr: &str,
) -> Result<String, VariableTableError> {
    let feature = variable_feature(document)?;
    let mut entries = parse_variables(feature)?;
    let idx = entries
        .iter()
        .position(|e| e.name == name)
        .ok_or_else(|| VariableTableError::UnknownVariable(name.to_string()))?;
    let old_expr = entries[idx].expr.clone();
    entries[idx].expr = expr.to_string();
    // Name the cycle through the edited variable precisely: any cycle a new
    // expression introduces must pass through the variable whose edge changed.
    resolve_table(&entries, Some(name))?;
    apply_table_edit(document, history, EDIT_TOKEN, &entries);
    Ok(format!("edit {name}: {old_expr} -> {expr}"))
}

/// Remove one variable. Removing a variable that another expression still
/// references leaves the rest of the table unresolvable, so the removal is
/// refused with the dangling reference named (the document is untouched).
pub fn remove_variable(
    document: &mut oep_import::Document,
    history: &mut doc_io::EditHistory,
    name: &str,
) -> Result<String, VariableTableError> {
    let feature = variable_feature(document)?;
    let entries = parse_variables(feature)?;
    let idx = entries
        .iter()
        .position(|e| e.name == name)
        .ok_or_else(|| VariableTableError::UnknownVariable(name.to_string()))?;
    let mut tentative = entries.clone();
    tentative.remove(idx);
    resolve_table(&tentative, None)?;
    apply_table_edit(document, history, REMOVE_TOKEN, &tentative);
    Ok(format!("removed {name}"))
}

/// Whether `name` is a usable expression identifier (non-empty, starts with a
/// letter or underscore, only letters/digits/underscores) - the same lexical
/// rule the B7 surface and [`referenced_names`] share.
fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ---------------------------------------------------------------------------
// Widget geometry (the test clicks the same rects via the exported helpers)
// ---------------------------------------------------------------------------

/// The interactive rects of one table row (expression field, Apply, Remove).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowRects {
    /// The expression `TextEdit`.
    pub expr: egui::Rect,
    /// The "Apply" button (resolves the expression draft as the row's expr).
    pub apply: egui::Rect,
    /// The "Remove" button.
    pub remove: egui::Rect,
}

/// The interactive rects of the add-variable form (name, unit, expr, Add).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AddRects {
    /// The new variable's name field.
    pub name: egui::Rect,
    /// The new variable's unit field.
    pub unit: egui::Rect,
    /// The new variable's expression field.
    pub expr: egui::Rect,
    /// The "Add" button.
    pub add: egui::Rect,
}

fn row_top(region: egui::Rect, index: usize) -> f32 {
    region.min.y + 40.0 + index as f32 * ROW_H
}

/// The interactive rects of `index`-th row in the table laid out in `region`.
pub fn row_rects(region: egui::Rect, index: usize) -> RowRects {
    let x0 = region.min.x + 8.0;
    let top = row_top(region, index) + 1.0;
    RowRects {
        expr: egui::Rect::from_min_size(egui::pos2(x0 + 96.0, top), egui::vec2(EXPR_W, LABEL_H)),
        apply: egui::Rect::from_min_size(egui::pos2(x0 + 470.0, top), egui::vec2(BTN_W, LABEL_H)),
        remove: egui::Rect::from_min_size(egui::pos2(x0 + 528.0, top), egui::vec2(BTN_W, LABEL_H)),
    }
}

/// The interactive rects of the add-variable form at the foot of `rows` rows
/// laid out in `region`.
pub fn add_rects(region: egui::Rect, rows: usize) -> AddRects {
    let x0 = region.min.x + 8.0;
    let top = row_top(region, rows) + 10.0;
    AddRects {
        name: egui::Rect::from_min_size(egui::pos2(x0, top), egui::vec2(NAME_W, LABEL_H)),
        unit: egui::Rect::from_min_size(egui::pos2(x0 + 96.0, top), egui::vec2(UNIT_W, LABEL_H)),
        expr: egui::Rect::from_min_size(
            egui::pos2(x0 + 142.0, top),
            egui::vec2(EXPR_W - 56.0, LABEL_H),
        ),
        add: egui::Rect::from_min_size(egui::pos2(x0 + 300.0, top), egui::vec2(BTN_W, LABEL_H)),
    }
}

// ---------------------------------------------------------------------------
// The panel
// ---------------------------------------------------------------------------

/// One UI gesture collected during the frame, applied to the model afterwards
/// (immediate-mode: the panel paints first, then applies one journaled edit).
enum Action {
    None,
    Add,
    Edit(String),
    Remove(String),
}

/// Draw the variable table panel content inside its already-positioned
/// region. Rows are derived from the live document every frame; an edit
/// applies through the journal and the resolved values / usage counts /
/// driven volume are published in the oracle on the same frame.
pub fn show(ui: &mut egui::Ui, _app: &mut App) {
    let mode = theme::mode_of(ui);
    ui.label(theme::heading(mode, "Variables").size(13.0));
    ui.separator();
    ui.add_space(6.0);

    let ctx = ui.ctx().clone();
    let region = ui.max_rect();

    let mut model = read_model(&ctx);
    if !model.seeded {
        seed_model(&ctx, &mut model);
    }

    // 1. Live rows from the document's VariableSet feature.
    let mut entries: Vec<VarEntry> = Vec::new();
    if let Some(doc) = model.document.as_ref() {
        match variable_feature(doc).and_then(parse_variables) {
            Ok(rows) => entries = rows,
            Err(e) => model.error = Some(e),
        }
    }

    // Keep a live draft per row (the expression field shows the row's expr);
    // drop drafts for rows that were removed.
    for e in &entries {
        model.drafts.entry(e.name.clone()).or_insert(e.expr.clone());
    }
    model
        .drafts
        .retain(|name, _| entries.iter().any(|e| &e.name == name));

    // 2. Resolution of the whole table (the model it drives).
    let resolved = resolve_table(&entries, None);
    let resolved_map: std::collections::BTreeMap<String, f64> =
        resolved.clone().unwrap_or_default();
    if let Err(e) = &resolved {
        model.error = Some(e.clone());
    }

    if model.document.is_none() {
        ui.label(
            egui::RichText::new("No variable table document")
                .weak()
                .italics(),
        );
    } else {
        // 3. Column header.
        ui.add_space(2.0);
        let x0 = region.min.x + 8.0;
        let muted = oep_ui_theme::Palette::resolve(mode).text_muted;
        for (label, x) in [
            ("Name", x0),
            ("Expression", x0 + 96.0),
            ("Value", x0 + 302.0),
            ("Unit", x0 + 378.0),
            ("Uses", x0 + 424.0),
        ] {
            ui.painter().text(
                egui::pos2(x, region.min.y + 30.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(9.0),
                muted,
            );
        }

        // 4. One row per variable.
        let mut action = Action::None;
        for (i, entry) in entries.iter().enumerate() {
            let name = entry.name.clone();
            let value = resolved_map.get(&name).copied();
            let uses = {
                let doc = model.document.as_ref();
                doc.map(|d| usage_of(d, &name)).unwrap_or(0)
            };

            let rects = row_rects(region, i);
            // The expression field binds DIRECTLY to the model's draft for
            // this row, so a value the user (or a test's assistive-tech
            // SetValue) puts in the field lands in the model the same frame -
            // no copy round-trip to lose it.
            let draft = model
                .drafts
                .entry(name.clone())
                .or_insert_with(|| entry.expr.clone());
            let input = ui.put(
                rects.expr,
                egui::TextEdit::singleline(draft)
                    .hint_text("expr")
                    .desired_width(EXPR_W - 6.0)
                    .id(egui::Id::unique(("ui_c3_variable_row_expr", i))),
            );
            let submitted = input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));

            // Value / unit / uses readouts.
            let value_text = value
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "-".to_string());
            ui.painter().text(
                rects.expr.min + egui::vec2(EXPR_W + 6.0, 0.0) + egui::vec2(0.0, 3.0),
                egui::Align2::LEFT_TOP,
                value_text,
                egui::FontId::proportional(11.0),
                oep_ui_theme::Palette::resolve(mode).text_primary,
            );
            ui.painter().text(
                rects.expr.min + egui::vec2(EXPR_W + 82.0, 0.0) + egui::vec2(0.0, 3.0),
                egui::Align2::LEFT_TOP,
                &entry.unit,
                egui::FontId::proportional(11.0),
                muted,
            );
            ui.painter().text(
                rects.expr.min + egui::vec2(EXPR_W + 128.0, 0.0) + egui::vec2(0.0, 3.0),
                egui::Align2::LEFT_TOP,
                uses.to_string(),
                egui::FontId::proportional(11.0),
                muted,
            );
            // The row name (also the a11y label a test can query).
            ui.painter().text(
                egui::pos2(region.min.x + 8.0, rects.expr.min.y + 3.0),
                egui::Align2::LEFT_TOP,
                &name,
                egui::FontId::proportional(11.0),
                oep_ui_theme::Palette::resolve(mode).text_primary,
            );

            if ui.put(rects.apply, egui::Button::new("Apply")).clicked() || submitted {
                action = Action::Edit(name.clone());
            }
            if ui.put(rects.remove, egui::Button::new("Remove")).clicked() {
                action = Action::Remove(name.clone());
            }
        }

        // 5. Add form.
        let add = add_rects(region, entries.len());
        let name_edit = egui::TextEdit::singleline(&mut model.add_name)
            .hint_text("name")
            .desired_width(NAME_W - 6.0);
        let unit_edit = egui::TextEdit::singleline(&mut model.add_unit)
            .hint_text("unit")
            .desired_width(UNIT_W - 6.0);
        let expr_edit = egui::TextEdit::singleline(&mut model.add_expr)
            .hint_text("expr")
            .desired_width(add.expr.width() - 6.0);
        ui.put(add.name, name_edit);
        ui.put(add.unit, unit_edit);
        ui.put(add.expr, expr_edit);
        if ui.put(add.add, egui::Button::new("Add")).clicked() {
            action = Action::Add;
        }

        // 6. Apply the collected gesture: edits happen ONLY inside
        //    `EditHistory::apply`.
        let mut doc_changed = false;
        match action {
            Action::None => {}
            Action::Add => {
                if let Some(doc) = model.document.as_mut() {
                    let name = model.add_name.trim().to_string();
                    let unit = model.add_unit.trim().to_string();
                    let expr = model.add_expr.trim().to_string();
                    match add_variable(doc, &mut model.history, &name, &unit, &expr) {
                        Ok(msg) => {
                            model.add_name.clear();
                            model.add_unit.clear();
                            model.add_expr.clear();
                            model.error = None;
                            model.last = Some(msg);
                            doc_changed = true;
                        }
                        Err(e) => {
                            model.error = Some(e);
                            model.last = None;
                        }
                    }
                }
            }
            Action::Edit(name) => {
                if let Some(doc) = model.document.as_mut() {
                    let draft = model
                        .drafts
                        .get(&name)
                        .cloned()
                        .unwrap_or_default()
                        .trim()
                        .to_string();
                    match edit_variable(doc, &mut model.history, &name, &draft) {
                        Ok(msg) => {
                            model.error = None;
                            model.last = Some(msg);
                            doc_changed = true;
                        }
                        Err(e) => {
                            model.error = Some(e);
                            model.last = None;
                        }
                    }
                }
            }
            Action::Remove(name) => {
                if let Some(doc) = model.document.as_mut() {
                    match remove_variable(doc, &mut model.history, &name) {
                        Ok(msg) => {
                            model.error = None;
                            model.last = Some(msg);
                            doc_changed = true;
                        }
                        Err(e) => {
                            model.error = Some(e);
                            model.last = None;
                        }
                    }
                }
            }
        }
        if doc_changed {
            entries.clear();
            if let Some(doc) = model.document.as_ref() {
                if let Ok(rows) = variable_feature(doc).and_then(parse_variables) {
                    entries = rows;
                }
            }
        }

        // 7. Honest state: the last named error and the last action.
        ui.add_space(12.0);
        if let Some(err) = &model.error {
            let danger = oep_ui_theme::Palette::resolve(mode).danger;
            ui.colored_label(danger, format!("variable table error: {err}"));
        }
        if let Some(last) = &model.last {
            ui.label(egui::RichText::new(last).weak().italics());
        }
    }

    // 8. Publish: the shared resolved set, the report oracle and the model.
    //    The LAST refused edit's named error persists until a later action
    //    succeeds (the action phase clears it); a genuinely unresolvable
    //    table always reports its own error here.
    let resolved_map = match resolve_table(&entries, None) {
        Ok(map) => map,
        Err(e) => {
            model.error = Some(e);
            std::collections::BTreeMap::new()
        }
    };
    let volume = match (&model.document, driven_volume(&resolved_map)) {
        (Some(_), Ok(Some(v))) => Some(v),
        (_, Err(e)) => {
            model.error = Some(e);
            None
        }
        _ => None,
    };
    publish_resolved(&ctx, &resolved_map);
    publish_report(&ctx, &model, &entries, &resolved_map, volume);
    write_model(&ctx, &model);
}

/// Publish the shared resolved variable set (sorted `name=value` lines) under
/// [`VARIABLES_RESOLVED_KEY`] - the seam a driving consumer reads.
fn publish_resolved(ctx: &egui::Context, resolved: &std::collections::BTreeMap<String, f64>) {
    let body = resolved
        .iter()
        .map(|(name, value)| format!("{name}={value:.6}"))
        .collect::<Vec<_>>()
        .join("\n");
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(VARIABLES_RESOLVED_KEY), body);
    });
}

/// Publish the plain-`String` report oracle with the live table truth.
fn publish_report(
    ctx: &egui::Context,
    model: &VariableModel,
    entries: &[VarEntry],
    resolved: &std::collections::BTreeMap<String, f64>,
    volume: Option<f64>,
) {
    let mut out = String::new();
    out.push_str(&format!("open={}\n", model.document.is_some() as u8));
    out.push_str(&format!("rows={}\n", entries.len()));
    let hash = model
        .document
        .as_ref()
        .map(doc_io::document_hash)
        .unwrap_or(0);
    out.push_str(&format!("hash={hash}\n"));
    for (i, e) in entries.iter().enumerate() {
        let value = resolved
            .get(&e.name)
            .map(|v| format!("{v:.6}"))
            .unwrap_or_else(|| "-".to_string());
        let uses = model
            .document
            .as_ref()
            .map(|d| usage_of(d, &e.name))
            .unwrap_or(0);
        out.push_str(&format!("v{i}_name={}\n", e.name));
        out.push_str(&format!("v{i}_value={value}\n"));
        out.push_str(&format!("v{i}_unit={}\n", e.unit));
        out.push_str(&format!("v{i}_expr={}\n", e.expr));
        out.push_str(&format!("v{i}_uses={uses}\n"));
    }
    match volume {
        Some(v) => out.push_str(&format!("volume={v:.6}\nvolume_ok=1\n")),
        None => out.push_str("volume=-\nvolume_ok=0\n"),
    }
    match &model.error {
        Some(e) => out.push_str(&format!("error={e}\n")),
        None => out.push_str("error=-\n"),
    }
    out.push_str(&format!("last={}\n", model.last.as_deref().unwrap_or("-")));
    ctx.data_mut(|d| {
        d.insert_temp::<String>(egui::Id::unique(VARIABLES_REPORT_KEY), out);
    });
}
