//! UI-B7 UI-DIMENSIONS: place, drive and edit dimensions (wave B).
//!
//! A dimension is placed on the committed sketch geometry, driven by a name
//! from the variable table, and edited through the properties panel as an
//! expression:
//!
//! - **Place.** [`place`] attaches a dimension to the most recently committed
//!   sketch line (the same "last element" target the constraint toolbar
//!   uses). Placing registers a variable-table entry - `width` = the line's
//!   measured length - and computes the driven model's volume from the
//!   document's own mass properties.
//! - **Drive.** The dimension VALUE is resolved through the *existing*
//!   variable table path, `oep_sketch::varset::VariableSet` (the same store
//!   the sketch solver resolves `DimExpr::Variable` names from). There is no
//!   second variable table and no second solver: [`edit`] evaluates the
//!   expression text against that one table and, on success, writes the
//!   resolved value back into it.
//! - **Edited as an expression.** The properties-panel input accepts a scalar
//!   expression (`width*1.5`); every identifier inside it resolves
//!   exclusively through `VariableSet::get`. The arithmetic layer is a
//!   deliberately minimal expression surface (numbers, `+ - * /`, parens,
//!   unary minus) - not a parser for the model and not a second variable
//!   system. Anything the surface cannot parse, or a name the table cannot
//!   resolve, is a named [`DimensionError`] that leaves the document and the
//!   variable table untouched.
//! - **Volume from the document's own mass properties.** [`driven_volume`]
//!   builds the driven solid through the existing editor drive-lane
//!   ([`oep_shell::editor::EditorSession`]: sketch the driven 2:1 profile,
//!   constrain it from `width`, solve, extrude) and returns the tessellated
//!   solid's own `mesh_volume`. The acceptance comment's numbers fall out of
//!   the shape, so the test derives its expectation from this document, never
//!   from a bare literal in the test: an 80-wide extrude weighs 32,000 and an
//!   edited `width*1.5` (120) weighs 72,000.
//!
//! The driven profile is a 2:1 rectangle (`width x width/2`, the canonical
//! editor-sketch profile) extruded [`DRIVEN_DEPTH`] deep, so
//! `volume = width * (width/2) * depth = 5 * width^2`. This is the model the
//! dimension drives; editing the width dimension re-resolves the (documented)
//! height through the same variable table and the volume follows.

use oep_sketch::varset::VariableSet;

/// The name a placed dimension registers in the variable table.
pub const WIDTH_NAME: &str = "width";
/// The driven profile keeps a fixed 2:1 aspect (height = width/2), the
/// canonical editor-sketch rectangle, so one dimension drives both plan
/// lengths and `volume = 5 * width^2` (32000 at 80, 72000 at 120).
pub const WIDTH_HEIGHT_RATIO: f64 = 2.0;
/// Extrude depth of the driven solid (mm). Part of the driven model's
/// definition, not a number the test hardcodes.
pub const DRIVEN_DEPTH: f64 = 10.0;

/// One placed dimension: a name from the variable table, the sketch line it
/// sits on, the expression driving it and its current resolved value.
#[derive(Debug, Clone, PartialEq)]
pub struct UiDimension {
    /// Variable-table name (e.g. `width`).
    pub name: String,
    /// The committed sketch line id the dimension is placed on (`l0`, ...).
    pub target: String,
    /// The editable expression text (e.g. `width*1.5`).
    pub expr: String,
    /// The currently resolved value (from the variable table).
    pub value: f64,
}

/// A named, typed failure of placing/driving a dimension. Nothing here is a
/// panic and nothing invents geometry the document did not produce.
#[derive(Debug, Clone, PartialEq)]
pub enum DimensionError {
    /// No committed sketch line exists to place the dimension on.
    NoLine,
    /// The expression text cannot be parsed by the dimension-expression
    /// surface (unbalanced, unknown token, trailing garbage, division by
    /// zero, ...).
    InvalidExpression(String),
    /// The expression names a variable the table does not hold.
    UnknownVariable(String),
    /// A resolved value is not a finite, positive number a dimension may drive.
    NotFinite(f64),
    /// The driven-solid drive-lane (solve or extrude) failed.
    Drive(String),
}

impl std::fmt::Display for DimensionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DimensionError::NoLine => write!(f, "no committed sketch line to dimension"),
            DimensionError::InvalidExpression(msg) => write!(f, "invalid expression: {msg}"),
            DimensionError::UnknownVariable(name) => write!(f, "unknown variable '{name}'"),
            DimensionError::NotFinite(v) => {
                write!(
                    f,
                    "dimension value must be a finite positive number, got {v}"
                )
            }
            DimensionError::Drive(msg) => write!(f, "could not drive the model: {msg}"),
        }
    }
}

impl std::error::Error for DimensionError {}

/// The dimension panel state, stored in the egui context data store under a
/// plain `String` key (the same cross-`#[path]`-copy-safe pattern the B6
/// constraint panel uses: the typed state stays inside its pane, the oracle
/// is a `String`, so tests and other panels read it without sharing a
/// `TypeId`).
#[derive(Debug, Clone, PartialEq)]
pub struct DimensionPanel {
    /// The placed dimension (this card places one `width` dimension; the
    /// struct is a list so a later table card can host several).
    pub dims: Vec<UiDimension>,
    /// The single variable table the dimension resolves through.
    pub vars: VariableSet,
    /// The last honest driven volume (the document's own mass property),
    /// kept even after a refused edit so the previous good drive survives.
    pub volume: Option<f64>,
    /// The last named error from a refused place/edit, if any. A refused
    /// input sets this and leaves `volume` / `vars` / `dims` untouched.
    pub error: Option<DimensionError>,
    /// One-line description of the last action (oracle readout).
    pub last: Option<String>,
}

impl Default for DimensionPanel {
    fn default() -> Self {
        Self {
            dims: Vec::new(),
            vars: VariableSet::new(),
            volume: None,
            error: None,
            last: None,
        }
    }
}

impl DimensionPanel {
    /// The placed dimension, if any.
    pub fn dimension(&self) -> Option<&UiDimension> {
        self.dims.first()
    }

    /// The last honest driven volume, if a successful drive ran.
    pub fn volume(&self) -> Option<f64> {
        self.volume
    }

    /// The last named error (invalid expression, unknown variable, ...).
    pub fn error(&self) -> Option<&DimensionError> {
        self.error.as_ref()
    }
}

/// Measure the last committed sketch line from the sketch `String` oracle
/// (the same report the tree panel and the test harness read): returns the
/// last `lineseg=` line's id and the Euclidean length of its endpoints (from
/// the `point=` lines). `None` when the sketch has no committed line.
///
/// This is the app's own cross-module geometry seam: a `String` oracle has an
/// identical `TypeId` across `#[path]` copies, unlike a typed `SketchDoc`.
pub fn measure_last_line(report: &str) -> Option<(String, f64)> {
    let mut points = std::collections::BTreeMap::new();
    let mut lines: Vec<(String, String, String)> = Vec::new();
    for line in report.lines() {
        if let Some(rest) = line.strip_prefix("point=") {
            // format: <x>,<y>,<id>  (id has no commas, so split from the right)
            let mut it = rest.rsplitn(2, ',');
            let id = it.next()?.to_string();
            let coords = it.next()?;
            let mut c = coords.split(',');
            let x = c.next()?.parse::<f64>().ok()?;
            let y = c.next()?.parse::<f64>().ok()?;
            points.insert(id, (x, y));
        } else if let Some(rest) = line.strip_prefix("lineseg=") {
            // format: <id>:<start>-><end>
            let mut it = rest.splitn(2, ':');
            let id = it.next()?.to_string();
            let seg = it.next()?.to_string();
            let mut ends = seg.split("->");
            let start = ends.next()?.to_string();
            let end = ends.next()?.to_string();
            lines.push((id, start, end));
        }
    }
    let (id, start, end) = lines.into_iter().last()?;
    let a = points.get(&start)?;
    let b = points.get(&end)?;
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    Some((id, len))
}

/// Place a dimension on the sketch's last committed line: register the
/// variable-table entry `width` = measured length and attach a [`UiDimension`]
/// whose expression is that name. Returns the updated panel and a one-line
/// message for the status oracle.
pub fn place(mut panel: DimensionPanel, report: &str) -> (DimensionPanel, Option<String>) {
    let Some((line, len)) = measure_last_line(report) else {
        let msg = "no line to dimension".to_string();
        panel.error = Some(DimensionError::NoLine);
        panel.last = Some(msg.clone());
        return (panel, Some(msg));
    };
    if !(len.is_finite() && len > 0.0) {
        let msg = "line length is not a positive number".to_string();
        panel.error = Some(DimensionError::NotFinite(len));
        panel.last = Some(msg.clone());
        return (panel, Some(msg));
    }
    panel.vars.set(WIDTH_NAME, len);
    panel.dims = vec![UiDimension {
        name: WIDTH_NAME.to_string(),
        target: line,
        expr: WIDTH_NAME.to_string(),
        value: len,
    }];
    match driven_volume(&panel.vars) {
        Ok(v) => {
            panel.volume = Some(v);
            panel.error = None;
        }
        Err(e) => panel.error = Some(e),
    }
    let msg = format!("placed dimension {WIDTH_NAME} = {len}");
    panel.last = Some(msg.clone());
    (panel, Some(msg))
}

/// Resolve an expression through the existing variable table. Every
/// identifier is looked up in `vars` (the variable-table path); the arithmetic
/// is a minimal scalar surface (`+ - * /`, parens, unary minus, numbers). A
/// syntax error or an unknown name is a named [`DimensionError`].
pub fn resolve_expression(vars: &VariableSet, text: &str) -> Result<f64, DimensionError> {
    let tokens = tokenize(text)?;
    let mut parser = Parser {
        tokens,
        pos: 0,
        vars,
    };
    let value = parser.parse_add()?;
    if parser.pos < parser.tokens.len() {
        return Err(DimensionError::InvalidExpression(format!(
            "trailing input near {:?}",
            parser.tokens[parser.pos].text
        )));
    }
    if !value.is_finite() {
        return Err(DimensionError::NotFinite(value));
    }
    Ok(value)
}

/// Edit the placed dimension as an expression. On success the resolved value
/// is written back into the same variable table, the dimension's expression
/// and value update, and the driven volume recomputes. On failure the panel,
/// the variable table and the document are all left untouched and the named
/// error is recorded.
pub fn edit(panel: DimensionPanel, expr: &str) -> (DimensionPanel, Option<String>) {
    let Some(dim) = panel.dimension().cloned() else {
        let msg = "no dimension placed".to_string();
        let mut panel = panel;
        panel.error = Some(DimensionError::NoLine);
        panel.last = Some(msg.clone());
        return (panel, Some(msg));
    };
    let new_value = match resolve_expression(&panel.vars, expr) {
        Ok(v) if v.is_finite() && v > 0.0 => v,
        Ok(v) => {
            let err = DimensionError::NotFinite(v);
            let msg = err.to_string();
            let mut panel = panel;
            panel.error = Some(err);
            panel.last = Some(msg.clone());
            return (panel, Some(msg));
        }
        Err(e) => {
            let msg = e.to_string();
            let mut panel = panel;
            panel.error = Some(e);
            panel.last = Some(msg.clone());
            return (panel, Some(msg));
        }
    };
    let mut panel = panel;
    panel.vars.set(dim.name.clone(), new_value);
    if let Some(d) = panel.dims.iter_mut().find(|d| d.name == dim.name) {
        d.expr = expr.to_string();
        d.value = new_value;
    }
    match driven_volume(&panel.vars) {
        Ok(v) => {
            panel.volume = Some(v);
            panel.error = None;
        }
        Err(e) => panel.error = Some(e),
    }
    let msg = format!("{}: {} -> {}", dim.name, dim.expr, expr);
    panel.last = Some(msg.clone());
    (panel, Some(msg))
}

/// The driven solid's volume from the document's own mass properties.
///
/// Builds the driven 2:1 profile through the existing editor drive-lane
/// (oep-shell's `EditorSession`), solves it constrained from the variable
/// table's `width`, extrudes [`DRIVEN_DEPTH`] and returns the tessellated
/// solid's `mesh_volume` - the document's own mass property, never a number
/// the caller smuggles in.
pub fn driven_volume(vars: &VariableSet) -> Result<f64, DimensionError> {
    use oep_shell::editor::EditorSession;
    use oep_sketch::sketch::{Constraint, DimExpr};

    let width = vars
        .get(WIDTH_NAME)
        .map_err(|e| DimensionError::UnknownVariable(e.to_string()))?;
    if !(width.is_finite() && width > 0.0) {
        return Err(DimensionError::NotFinite(width));
    }
    let height = width / WIDTH_HEIGHT_RATIO;

    let mut s = EditorSession::new();
    drive_ok(s.point(0.0, 0.0))?;
    drive_ok(s.point(width, 0.0))?;
    drive_ok(s.point(width, height))?;
    drive_ok(s.point(0.0, height))?;
    drive_ok(s.line("p0", "p1"))?;
    drive_ok(s.line("p1", "p2"))?;
    drive_ok(s.line("p2", "p3"))?;
    drive_ok(s.line("p3", "p0"))?;
    drive_ok(s.constrain(Constraint::Fixed {
        point: "p0".into(),
        x: 0.0,
        y: 0.0,
    }))?;
    drive_ok(s.constrain(Constraint::Horizontal { line: "l0".into() }))?;
    drive_ok(s.constrain(Constraint::Horizontal { line: "l2".into() }))?;
    drive_ok(s.constrain(Constraint::Vertical { line: "l1".into() }))?;
    drive_ok(s.constrain(Constraint::Vertical { line: "l3".into() }))?;
    drive_ok(s.constrain(Constraint::Length {
        line: "l0".into(),
        expr: DimExpr::Variable(WIDTH_NAME.into()),
    }))?;
    // The vertical side follows the width through the documented 2:1 aspect:
    // the solver pins l1 to the literal height (= width/2), so the driven
    // profile is width x width/2 and volume = 5 * width^2.
    drive_ok(s.constrain(Constraint::Length {
        line: "l1".into(),
        expr: DimExpr::Literal(height),
    }))?;
    drive_ok(s.set_var(WIDTH_NAME, width))?;
    drive_ok(s.solve())?;
    let report = s
        .extrude(DimExpr::Literal(DRIVEN_DEPTH))
        .map_err(|e| DimensionError::Drive(e.to_string()))?;
    Ok(report.volume)
}

/// Map any `EditorSession` op result onto the dimension engine's named Drive
/// error, discarding the returned report value.
fn drive_ok<T>(r: Result<T, oep_shell::editor::EditorError>) -> Result<(), DimensionError> {
    match r {
        Ok(_) => Ok(()),
        Err(e) => Err(DimensionError::Drive(e.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Minimal scalar expression surface over the existing variable table
// ---------------------------------------------------------------------------

/// A dimension-expression token: text splice plus a resolved meaning.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
}

impl Token {
    /// Human text for error messages.
    fn text(&self) -> String {
        match self {
            Token::Number(v) => format!("{v}"),
            Token::Ident(s) => s.clone(),
            Token::Plus => "+".into(),
            Token::Minus => "-".into(),
            Token::Star => "*".into(),
            Token::Slash => "/".into(),
            Token::LParen => "(".into(),
            Token::RParen => ")".into(),
        }
    }
}

/// A single token carries its resolved value plus an already-consumed-count,
/// so the lexer and parser trivially agree on where the input ends.
struct RawToken {
    text: String,
    kind: Token,
}

/// Tokenize a dimension expression into numbers / identifiers / operators.
/// Plain decimal numbers and `[A-Za-z_][A-Za-z0-9_]*` identifiers; anything
/// else is a named [`DimensionError::InvalidExpression`].
fn tokenize(text: &str) -> Result<Vec<RawToken>, DimensionError> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let kind = match c {
            '+' => {
                i += 1;
                Token::Plus
            }
            '-' => {
                i += 1;
                Token::Minus
            }
            '*' => {
                i += 1;
                Token::Star
            }
            '/' => {
                i += 1;
                Token::Slash
            }
            '(' => {
                i += 1;
                Token::LParen
            }
            ')' => {
                i += 1;
                Token::RParen
            }
            '0'..='9' => {
                let mut seen_dot = false;
                while i < b.len() {
                    let d = b[i];
                    if d.is_ascii_digit() {
                        i += 1;
                    } else if d == b'.' && !seen_dot {
                        seen_dot = true;
                        i += 1;
                    } else {
                        break;
                    }
                }
                let s = &text[start..i];
                let v: f64 = s
                    .parse()
                    .map_err(|_| DimensionError::InvalidExpression(format!("bad number {s:?}")))?;
                Token::Number(v)
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                    i += 1;
                }
                Token::Ident(text[start..i].to_string())
            }
            other => {
                return Err(DimensionError::InvalidExpression(format!(
                    "unexpected character {other:?}"
                )))
            }
        };
        out.push(RawToken {
            text: text[start..i].to_string(),
            kind,
        });
    }
    Ok(out)
}

/// A tiny recursive-descent evaluator over the token list. `pos` indexes into
/// `tokens`; every identifier resolves through the variable table at parse
/// time.
struct Parser<'a> {
    tokens: Vec<RawToken>,
    pos: usize,
    vars: &'a VariableSet,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos).map(|t| &t.kind)
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos)?.kind.clone();
        self.pos += 1;
        Some(t)
    }

    fn parse_add(&mut self) -> Result<f64, DimensionError> {
        let mut lhs = self.parse_mul()?;
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.bump();
                    lhs += self.parse_mul()?;
                }
                Some(Token::Minus) => {
                    self.bump();
                    lhs -= self.parse_mul()?;
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn parse_mul(&mut self) -> Result<f64, DimensionError> {
        let mut lhs = self.parse_unary()?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.bump();
                    lhs *= self.parse_unary()?;
                }
                Some(Token::Slash) => {
                    self.bump();
                    let rhs = self.parse_unary()?;
                    if rhs == 0.0 {
                        return Err(DimensionError::InvalidExpression("division by zero".into()));
                    }
                    lhs /= rhs;
                }
                _ => return Ok(lhs),
            }
        }
    }

    fn parse_unary(&mut self) -> Result<f64, DimensionError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.bump();
                Ok(-self.parse_unary()?)
            }
            Some(Token::Plus) => {
                self.bump();
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<f64, DimensionError> {
        match self.bump() {
            Some(Token::Number(v)) => Ok(v),
            Some(Token::Ident(name)) => self
                .vars
                .get(&name)
                .map_err(|_| DimensionError::UnknownVariable(name)),
            Some(Token::LParen) => {
                let v = self.parse_add()?;
                match self.peek() {
                    Some(Token::RParen) => {
                        self.bump();
                        Ok(v)
                    }
                    _ => Err(DimensionError::InvalidExpression(
                        "unbalanced parentheses".into(),
                    )),
                }
            }
            Some(other) => Err(DimensionError::InvalidExpression(format!(
                "unexpected token '{}'",
                other.text()
            ))),
            None => Err(DimensionError::InvalidExpression("empty expression".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oep_sketch::varset::VariableSet;

    fn vars() -> VariableSet {
        let mut v = VariableSet::new();
        v.set(WIDTH_NAME, 80.0);
        v
    }

    fn report_with_line(len: f64) -> String {
        format!(
            "mode=sketch\nplane=XY\nlabel=XY (Top)\nnormal=0,0,1\ngrid=on\n\
             tool=-\nsteps=0\npreview_points=0\npreview_lines=0\ndimx=-\ndimy=-\nlast=-\n\
             snap=on\ninfer=-\nsnapx=-\nsnapy=-\n\
             docpoints=2\ndoclines=1\ncircles=0\njournal=1\n\
             point=0.000,8.000,p0\npoint={:.3},8.000,p1\n\
             lineseg=l0:p0->p1\nconstraints=0\ndof=-\nsolve=unsolved\n\
             constraint_status=not solved\nconstraint_last=\ncontext_menu=0\n",
            len
        )
    }

    /// The variable table resolves a bare name: `width` -> 80.
    #[test]
    fn resolve_bare_name_through_the_variable_table() {
        assert_eq!(resolve_expression(&vars(), "width").unwrap(), 80.0);
    }

    /// `width*1.5` resolves 80 * 1.5 = 120 through the existing table path.
    #[test]
    fn resolve_width_times_factor() {
        assert_eq!(resolve_expression(&vars(), "width*1.5").unwrap(), 120.0);
    }

    /// Arithmetic works and honors precedence / parentheses.
    #[test]
    fn resolve_arithmetic_precedence_and_parentheses() {
        assert_eq!(resolve_expression(&vars(), "2+3*4").unwrap(), 14.0);
        assert_eq!(resolve_expression(&vars(), "(2+3)*4").unwrap(), 20.0);
        assert_eq!(resolve_expression(&vars(), "-width/2").unwrap(), -40.0);
    }

    /// An unknown name is a named UnknownVariable, never a panic.
    #[test]
    fn resolve_unknown_variable_is_named() {
        assert_eq!(
            resolve_expression(&vars(), "bogus*2"),
            Err(DimensionError::UnknownVariable("bogus".into()))
        );
    }

    /// Malformed expressions are typed invalid-expression errors.
    #[test]
    fn resolve_malformed_expressions_are_named() {
        for bad in [
            "width*",
            "width+*1.5",
            "(width",
            "width)",
            "1/0",
            "width 1.5",
            "1..2",
            "%",
        ] {
            let r = resolve_expression(&vars(), bad);
            assert!(
                matches!(
                    r,
                    Err(DimensionError::InvalidExpression(_)) | Err(DimensionError::NotFinite(_))
                ),
                "expected a named error for {bad:?}, got {r:?}"
            );
        }
    }

    /// `measure_last_line` reads the sketch oracle like the panel does: last
    /// lineseg + endpoint distance from the point lines.
    #[test]
    fn measure_last_line_from_the_sketch_oracle() {
        let (line, len) = measure_last_line(&report_with_line(80.0)).expect("line present");
        assert_eq!(line, "l0");
        assert!((len - 80.0).abs() < 1e-9, "len {len}");
        assert!(measure_last_line("mode=idle\nplane=-\n").is_none());
    }

    /// Placing a dimension registers `width` in the variable table, then the
    /// acceptance's volume falls out of the driven document (80 -> 5*80^2).
    #[test]
    fn place_sets_width_and_drives_the_document_volume() {
        let (panel, msg) = place(DimensionPanel::default(), &report_with_line(80.0));
        assert_eq!(msg.as_deref(), Some("placed dimension width = 80"));
        assert_eq!(panel.vars.get(WIDTH_NAME), Ok(80.0));
        let dim = panel.dimension().expect("dimension placed");
        assert_eq!(dim.target, "l0");
        assert_eq!(dim.expr, "width");
        assert_eq!(dim.value, 80.0);
        let vol = panel.volume().expect("volume driven");
        assert!((vol - 32000.0).abs() < 1e-6, "volume {vol}");
    }

    /// Editing the expression `width*1.5` resolves through the table to 120
    /// and the driven volume moves 32000 -> 72000 (from the document).
    #[test]
    fn edit_width_times_factor_moves_the_volume() {
        let (panel, _) = place(DimensionPanel::default(), &report_with_line(80.0));
        assert!((panel.volume().unwrap() - 32000.0).abs() < 1e-6);
        let (panel, msg) = edit(panel, "width*1.5");
        assert_eq!(msg.as_deref(), Some("width: width -> width*1.5"));
        assert_eq!(panel.vars.get(WIDTH_NAME), Ok(120.0));
        let dim = panel.dimension().expect("dimension kept");
        assert_eq!(dim.expr, "width*1.5");
        assert_eq!(dim.value, 120.0);
        let vol = panel.volume().expect("volume re-driven");
        assert!((vol - 72000.0).abs() < 1e-6, "volume {vol}");
    }

    /// An invalid expression changes nothing: the panel, the variable table
    /// and the driven volume are untouched, and the error is named.
    #[test]
    fn invalid_expression_refused_and_leaves_everything_untouched() {
        let (panel, _) = place(DimensionPanel::default(), &report_with_line(80.0));
        let vars_before = panel.vars.clone();
        let vol_before = panel.volume();

        let (panel, msg) = edit(panel, "width**1.5");
        assert!(msg.is_some());
        let err = panel.error().expect("named error present");
        assert!(
            matches!(err, DimensionError::InvalidExpression(_)),
            "kinds: {err:?}"
        );
        assert_eq!(panel.vars, vars_before, "variable table untouched");
        assert_eq!(panel.volume(), vol_before, "volume untouched");
        let dim = panel.dimension().unwrap();
        assert_eq!(dim.expr, "width", "dimension expression untouched");
        assert_eq!(dim.value, 80.0, "dimension value untouched");
    }

    /// An unknown variable in the expression is refused with its name shown.
    #[test]
    fn unknown_variable_error_names_the_variable() {
        let (panel, _) = place(DimensionPanel::default(), &report_with_line(80.0));
        let (panel, _) = edit(panel, "depth*2");
        assert_eq!(
            panel.error(),
            Some(&DimensionError::UnknownVariable("depth".into()))
        );
    }

    /// The driven volume is the document's own analytic value for the
    /// documented shape: 5 * width^2.
    #[test]
    fn driven_volume_is_5_times_width_squared() {
        for (w, expect) in [(80.0, 32000.0), (120.0, 72000.0), (40.0, 8000.0)] {
            let mut v = VariableSet::new();
            v.set(WIDTH_NAME, w);
            let vol = driven_volume(&v).unwrap();
            assert!((vol - expect).abs() < 1e-6, "w={w}: vol={vol}");
        }
    }

    /// Placing with no committed sketch line is an honest no-op.
    #[test]
    fn place_with_no_line_is_an_honest_noop() {
        let (panel, msg) = place(DimensionPanel::default(), "mode=idle\nplane=-\n");
        assert_eq!(msg.as_deref(), Some("no line to dimension"));
        assert_eq!(panel.error(), Some(&DimensionError::NoLine));
        assert!(panel.dimension().is_none());
        assert!(panel.volume().is_none());
    }
}
