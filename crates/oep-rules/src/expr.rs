//! The `oep-rules` expression engine: our own documented grammar.
//!
//! This is deliberately our own language, NOT CATIA V5 formula syntax
//! (documented not parity in `workstreams/P7-M4/rules-checks-sandbox.md`). It is a small
//! recursive-descent parser and evaluator with no external dependencies.
//!
//! Grammar (loosest to tightest binding):
//!
//! ```text
//! expression   := or
//! or           := and      ( "or"  and      )*
//! and          := equality ( "and" equality )*
//! equality     := comparison ( "==" | "!=" ) comparison )*
//! comparison   := additive ( "<=" | ">=" | "<" | ">" ) additive )*
//! additive     := unary    ( "+" | "-" ) unary )*
//! unary        := ( "-" | "not" ) unary | postfix
//! postfix      := primary ( "." ident | "[" ( number | string ) "]" )*
//! primary      := number | string | "true" | "false"
//!               | ident | ident "(" expr ( "," expr )* ")"
//!               | "(" expr ")"
//! ```
//!
//! Identifiers are `[A-Za-z_][A-Za-z0-9_]*` plus, after a dot, the segment may
//! be any identifier. Feature ids and parameter keys with hyphens are reached
//! through string indexes: `part.features["oep-feature-bracket-hole"].parameters["diameter"]`.
//!
//! Functions: `abs`, `round`, `floor`, `ceil`, `min`, `max`, `sqrt`, `pow`.
//! `+` is arithmetic for two numbers and concatenation when either operand is
//! a string. `and`/`or` short-circuit. Division by zero and `sqrt` of a
//! negative are typed errors. Every fallible path returns
//! [`RulesError`] - nothing panics on bad input.

use crate::error::{RulesError, ValueMismatch};

/// Re-export the runtime value type so `expr` is self-contained.
pub use crate::value::Value;

/// An index applied to a member path (after `[` ... `]`).
#[derive(Clone, Debug, PartialEq)]
pub enum Index {
    /// Integer index (e.g. `part.bbox.min[0]`).
    Int(i64),
    /// String index (e.g. `part.features["..."]`).
    Str(String),
}

/// One segment of a dotted `part.*` (or plain parameter) path.
#[derive(Clone, Debug, PartialEq)]
pub enum PathSeg {
    /// A named member (dot-separated identifier).
    Field(String),
    /// An indexed member (bracket).
    Index(Index),
}

/// A parsed expression in the documented grammar.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// A numeric literal.
    Num(f64),
    /// A string literal.
    Str(String),
    /// A boolean literal.
    Bool(bool),
    /// A member path (parameter reference or `part.*` reference).
    Path(Vec<PathSeg>),
    /// A function call.
    Call(String, Vec<Expr>),
    /// Unary negation / logical not.
    Unary {
        /// Negation (`-`) or logical not (`not`).
        op: UnaryOp,
        /// The operand.
        expr: Box<Expr>,
    },
    /// A binary operation.
    Binary {
        /// The operator.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnaryOp {
    /// Arithmetic negation.
    Neg,
    /// Logical not.
    Not,
}

/// Binary operators, from lowest to highest precedence in the grammar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BinOp {
    /// `or`
    Or,
    /// `and`
    And,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
}

/// Parse `src` into an [`Expr`]. Syntax errors are typed ([`RulesError::Parse`]).
pub fn parse(src: &str) -> Result<Expr, RulesError> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0, src };
    let e = p.parse_expr()?;
    if p.pos < p.toks.len() {
        return Err(RulesError::Parse {
            expr: src.to_string(),
            message: format!("unexpected trailing token {:?}", p.toks[p.pos]),
        });
    }
    Ok(e)
}

impl Expr {
    /// Evaluate this expression.
    ///
    /// `lookup` resolves every member path the expression references. Paths are
    /// passed as parsed segments; a plain single-field path is a parameter
    /// reference, a `part.*` path is resolved by the part context.
    pub fn eval(
        &self,
        lookup: &dyn Fn(&[PathSeg]) -> Result<Value, RulesError>,
    ) -> Result<Value, RulesError> {
        match self {
            Expr::Num(n) => Ok(Value::Number(*n)),
            Expr::Str(s) => Ok(Value::Str(s.clone())),
            Expr::Bool(b) => Ok(Value::Bool(*b)),
            Expr::Path(segs) => lookup(segs),
            Expr::Call(name, args) => eval_call(name, args, lookup),
            Expr::Unary { op, expr } => {
                let v = expr.eval(lookup)?;
                match op {
                    UnaryOp::Neg => match v.as_f64() {
                        Some(n) => Ok(Value::Number(-n)),
                        None => Err(ty("-", "number operand", &v)),
                    },
                    UnaryOp::Not => match v.as_bool() {
                        Some(b) => Ok(Value::Bool(!b)),
                        None => Err(ty("not", "boolean operand", &v)),
                    },
                }
            }
            Expr::Binary { op, lhs, rhs } => match op {
                BinOp::And => {
                    let l = require_bool(lhs.eval(lookup)?, "and")?;
                    if !l {
                        return Ok(Value::Bool(false));
                    }
                    let r = require_bool(rhs.eval(lookup)?, "and")?;
                    Ok(Value::Bool(r))
                }
                BinOp::Or => {
                    let l = require_bool(lhs.eval(lookup)?, "or")?;
                    if l {
                        return Ok(Value::Bool(true));
                    }
                    let r = require_bool(rhs.eval(lookup)?, "or")?;
                    Ok(Value::Bool(r))
                }
                _ => {
                    let l = lhs.eval(lookup)?;
                    let r = rhs.eval(lookup)?;
                    eval_binary(*op, &l, &r)
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    True,
    False,
    And,
    Or,
    Not,
    Eq,
    Ne,
    Le,
    Ge,
    Lt,
    Gt,
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    LBrack,
    RBrack,
    Comma,
    Dot,
}

fn lex(src: &str) -> Result<Vec<Tok>, RulesError> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i < chars.len()
                && chars[i] == '.'
                && i + 1 < chars.len()
                && chars[i + 1].is_ascii_digit()
            {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            let n: f64 = text.parse().map_err(|_| RulesError::Parse {
                expr: src.to_string(),
                message: format!("bad number literal {text:?}"),
            })?;
            out.push(Tok::Num(n));
            continue;
        }
        if c == '"' {
            i += 1;
            let mut s = String::new();
            while i < chars.len() && chars[i] != '"' {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    let nxt = chars[i + 1];
                    match nxt {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        other => {
                            return Err(RulesError::Parse {
                                expr: src.to_string(),
                                message: format!("unsupported escape \\{other}"),
                            })
                        }
                    }
                    i += 2;
                } else {
                    s.push(chars[i]);
                    i += 1;
                }
            }
            if i >= chars.len() {
                return Err(RulesError::Parse {
                    expr: src.to_string(),
                    message: "unterminated string literal".to_string(),
                });
            }
            i += 1; // closing quote
            out.push(Tok::Str(s));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            out.push(match word.as_str() {
                "true" => Tok::True,
                "false" => Tok::False,
                "and" => Tok::And,
                "or" => Tok::Or,
                "not" => Tok::Not,
                _ => Tok::Ident(word),
            });
            continue;
        }
        let (tok, adv) = match c {
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    (Tok::Eq, 2)
                } else {
                    return Err(parse_err(src, "unexpected '=' (use '==' for equality)"));
                }
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    (Tok::Ne, 2)
                } else {
                    return Err(parse_err(src, "unexpected '!' (use '!=' for inequality)"));
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    (Tok::Le, 2)
                } else {
                    (Tok::Lt, 1)
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    (Tok::Ge, 2)
                } else {
                    (Tok::Gt, 1)
                }
            }
            '+' => (Tok::Plus, 1),
            '-' => (Tok::Minus, 1),
            '*' => (Tok::Star, 1),
            '/' => (Tok::Slash, 1),
            '(' => (Tok::LParen, 1),
            ')' => (Tok::RParen, 1),
            '[' => (Tok::LBrack, 1),
            ']' => (Tok::RBrack, 1),
            ',' => (Tok::Comma, 1),
            '.' => (Tok::Dot, 1),
            other => {
                return Err(parse_err(src, &format!("unexpected character {other:?}")));
            }
        };
        out.push(tok);
        i += adv;
    }
    Ok(out)
}

fn parse_err(src: &str, message: &str) -> RulesError {
    RulesError::Parse {
        expr: src.to_string(),
        message: message.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    toks: Vec<Tok>,
    pos: usize,
    src: &'a str,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Result<Tok, RulesError> {
        let t = self
            .toks
            .get(self.pos)
            .cloned()
            .ok_or_else(|| parse_err(self.src, "unexpected end of expression"))?;
        self.pos += 1;
        Ok(t)
    }

    fn expect(&mut self, expected: &[Tok]) -> Result<Tok, RulesError> {
        let t = self.next()?;
        if expected.contains(&t) {
            Ok(t)
        } else {
            Err(parse_err(
                self.src,
                &format!("expected {expected:?} but found {t:?}"),
            ))
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, RulesError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, RulesError> {
        let mut lhs = self.parse_and()?;
        while self.peek() == Some(&Tok::Or) {
            self.next()?;
            let rhs = self.parse_and()?;
            lhs = Expr::Binary {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> Result<Expr, RulesError> {
        let mut lhs = self.parse_equality()?;
        while self.peek() == Some(&Tok::And) {
            self.next()?;
            let rhs = self.parse_equality()?;
            lhs = Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_equality(&mut self) -> Result<Expr, RulesError> {
        let mut lhs = self.parse_comparison()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Eq) => Some(BinOp::Eq),
                Some(Tok::Ne) => Some(BinOp::Ne),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.next()?;
                    let rhs = self.parse_comparison()?;
                    lhs = Expr::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                None => break,
            }
        }
        Ok(lhs)
    }

    fn parse_comparison(&mut self) -> Result<Expr, RulesError> {
        let mut lhs = self.parse_additive()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Le) => Some(BinOp::Le),
                Some(Tok::Ge) => Some(BinOp::Ge),
                Some(Tok::Lt) => Some(BinOp::Lt),
                Some(Tok::Gt) => Some(BinOp::Gt),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.next()?;
                    let rhs = self.parse_additive()?;
                    lhs = Expr::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                None => break,
            }
        }
        Ok(lhs)
    }

    fn parse_additive(&mut self) -> Result<Expr, RulesError> {
        let mut lhs = self.parse_multiplicative()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Plus) => Some(BinOp::Add),
                Some(Tok::Minus) => Some(BinOp::Sub),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.next()?;
                    let rhs = self.parse_multiplicative()?;
                    lhs = Expr::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                None => break,
            }
        }
        Ok(lhs)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, RulesError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Star) => Some(BinOp::Mul),
                Some(Tok::Slash) => Some(BinOp::Div),
                _ => None,
            };
            match op {
                Some(op) => {
                    self.next()?;
                    let rhs = self.parse_unary()?;
                    lhs = Expr::Binary {
                        op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    };
                }
                None => break,
            }
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<Expr, RulesError> {
        match self.peek() {
            Some(Tok::Minus) => {
                self.next()?;
                let e = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    expr: Box::new(e),
                })
            }
            Some(Tok::Not) => {
                self.next()?;
                let e = self.parse_unary()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    expr: Box::new(e),
                })
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, RulesError> {
        let base = self.parse_primary()?;
        let mut segs: Vec<PathSeg> = match base {
            Expr::Path(segs) => segs,
            _ => {
                if matches!(self.peek(), Some(Tok::Dot) | Some(Tok::LBrack)) {
                    return Err(parse_err(self.src, "member/index on a literal value"));
                }
                return Ok(base);
            }
        };
        loop {
            match self.peek() {
                Some(Tok::Dot) => {
                    self.next()?;
                    let name = match self.next()? {
                        Tok::Ident(name) => name,
                        other => {
                            return Err(parse_err(
                                self.src,
                                &format!("expected a member name after '.', found {other:?}"),
                            ))
                        }
                    };
                    segs.push(PathSeg::Field(name));
                }
                Some(Tok::LBrack) => {
                    self.next()?;
                    let idx = match self.next()? {
                        Tok::Num(n) => Index::Int(n as i64),
                        Tok::Str(s) => Index::Str(s),
                        other => {
                            return Err(parse_err(
                                self.src,
                                &format!("expected a number or string index, found {other:?}"),
                            ))
                        }
                    };
                    self.expect(&[Tok::RBrack])?;
                    segs.push(PathSeg::Index(idx));
                }
                _ => break,
            }
        }
        Ok(Expr::Path(segs))
    }

    fn parse_primary(&mut self) -> Result<Expr, RulesError> {
        let tok = self.next()?;
        match tok {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::True => Ok(Expr::Bool(true)),
            Tok::False => Ok(Expr::Bool(false)),
            Tok::Ident(name) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.next()?;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            match self.next()? {
                                Tok::Comma => continue,
                                Tok::RParen => break,
                                other => {
                                    return Err(parse_err(
                                        self.src,
                                        &format!("expected ',' or ')', found {other:?}"),
                                    ))
                                }
                            }
                        }
                    } else {
                        self.next()?;
                    }
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Path(vec![PathSeg::Field(name)]))
                }
            }
            Tok::LParen => {
                let e = self.parse_expr()?;
                self.expect(&[Tok::RParen])?;
                Ok(e)
            }
            other => Err(parse_err(
                self.src,
                &format!("expected a value, found {other:?}"),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// Evaluator
// ---------------------------------------------------------------------------

/// Evaluate a function call.
fn eval_call(
    name: &str,
    args: &[Expr],
    lookup: &dyn Fn(&[PathSeg]) -> Result<Value, RulesError>,
) -> Result<Value, RulesError> {
    let mut vs = Vec::with_capacity(args.len());
    for a in args {
        vs.push(a.eval(lookup)?);
    }
    match name {
        "abs" => {
            let n = one_number(name, &vs)?;
            Ok(Value::Number(n.abs()))
        }
        "round" => {
            let n = one_number(name, &vs)?;
            Ok(Value::Number(n.round()))
        }
        "floor" => {
            let n = one_number(name, &vs)?;
            Ok(Value::Number(n.floor()))
        }
        "ceil" => {
            let n = one_number(name, &vs)?;
            Ok(Value::Number(n.ceil()))
        }
        "sqrt" => {
            let n = one_number(name, &vs)?;
            if n < 0.0 {
                return Err(RulesError::NegativeSqrt {
                    expr: format!("sqrt({n})"),
                });
            }
            Ok(Value::Number(n.sqrt()))
        }
        "pow" => {
            if vs.len() != 2 {
                return Err(arity(name, &vs));
            }
            let (a, b) = (num_arg(name, 0, &vs)?, num_arg(name, 1, &vs)?);
            Ok(Value::Number(a.powf(b)))
        }
        "min" => {
            at_least_two(name, &vs)?;
            let mut m = f64::INFINITY;
            for v in &vs {
                m = m.min(num_arg(name, 0, std::slice::from_ref(v))?);
            }
            Ok(Value::Number(m))
        }
        "max" => {
            at_least_two(name, &vs)?;
            let mut m = f64::NEG_INFINITY;
            for v in &vs {
                m = m.max(num_arg(name, 0, std::slice::from_ref(v))?);
            }
            Ok(Value::Number(m))
        }
        other => Err(RulesError::TypeMismatch(ValueMismatch {
            expr: other.to_string(),
            context: "function call",
            got: Value::Str(String::new()),
            wanted: "known function",
        })),
    }
}

fn one_number(name: &str, vs: &[Value]) -> Result<f64, RulesError> {
    if vs.len() != 1 {
        return Err(arity(name, vs));
    }
    num_arg(name, 0, vs)
}

fn arity(name: &str, vs: &[Value]) -> RulesError {
    RulesError::TypeMismatch(ValueMismatch {
        expr: format!("{name}({})", vs.len()),
        context: "function arity",
        got: Value::Number(vs.len() as f64),
        wanted: "call with correct arity",
    })
}

fn at_least_two(name: &str, vs: &[Value]) -> Result<(), RulesError> {
    if vs.len() < 2 {
        return Err(arity(name, vs));
    }
    Ok(())
}

fn num_arg(name: &str, idx: usize, vs: &[Value]) -> Result<f64, RulesError> {
    match vs.get(idx).and_then(Value::as_f64) {
        Some(n) => Ok(n),
        None => Err(RulesError::TypeMismatch(ValueMismatch {
            expr: name.to_string(),
            context: "function argument",
            got: vs.get(idx).cloned().unwrap_or(Value::Number(0.0)),
            wanted: "number",
        })),
    }
}

fn eval_binary(op: BinOp, l: &Value, r: &Value) -> Result<Value, RulesError> {
    use BinOp::*;
    match op {
        Eq => Ok(Value::Bool(l == r)),
        Ne => Ok(Value::Bool(l != r)),
        Lt | Le | Gt | Ge => {
            let ord = compare(l, r, op)?;
            Ok(Value::Bool(ord))
        }
        Add => {
            if let (Some(a), Some(b)) = (l.as_f64(), r.as_f64()) {
                return Ok(Value::Number(a + b));
            }
            if l.as_str().is_some() || r.as_str().is_some() {
                return Ok(Value::Str(format!("{l}{r}")));
            }
            Err(ty("+", "two numbers or at least one string", l))
        }
        Sub | Mul => {
            let (a, b) = two_numbers(l, r)?;
            let v = match op {
                Sub => a - b,
                Mul => a * b,
                _ => unreachable!(),
            };
            Ok(Value::Number(v))
        }
        Div => {
            let (a, b) = two_numbers(l, r)?;
            if b == 0.0 {
                return Err(RulesError::DivisionByZero {
                    expr: format!("{l} / {r}"),
                });
            }
            Ok(Value::Number(a / b))
        }
        And | Or => unreachable!("and/or short-circuit before this point"),
    }
}

fn two_numbers(l: &Value, r: &Value) -> Result<(f64, f64), RulesError> {
    match (l.as_f64(), r.as_f64()) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err(ty("binary number operation", "number operands", l)),
    }
}

fn compare(l: &Value, r: &Value, op: BinOp) -> Result<bool, RulesError> {
    use BinOp::*;
    match (l, r) {
        (Value::Number(a), Value::Number(b)) => Ok(match op {
            Lt => a < b,
            Le => a <= b,
            Gt => a > b,
            Ge => a >= b,
            _ => unreachable!(),
        }),
        (Value::Str(a), Value::Str(b)) => Ok(match op {
            Lt => a < b,
            Le => a <= b,
            Gt => a > b,
            Ge => a >= b,
            _ => unreachable!(),
        }),
        _ => Err(RulesError::TypeMismatch(ValueMismatch {
            expr: "comparison".to_string(),
            context: "comparison",
            got: l.clone(),
            wanted: "two numbers or two strings of comparable type",
        })),
    }
}

fn require_bool(v: Value, what: &'static str) -> Result<bool, RulesError> {
    match v.as_bool() {
        Some(b) => Ok(b),
        None => Err(RulesError::TypeMismatch(ValueMismatch {
            expr: what.to_string(),
            context: what,
            got: v,
            wanted: "boolean",
        })),
    }
}

fn ty(what: &str, wanted: &'static str, got: &Value) -> RulesError {
    RulesError::TypeMismatch(ValueMismatch {
        expr: what.to_string(),
        context: "expression",
        got: got.clone(),
        wanted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_path(_: &[PathSeg]) -> Result<Value, RulesError> {
        unreachable!("no paths expected")
    }

    #[test]
    fn precedence_and_simple_arithmetic() {
        assert_eq!(
            parse("2 + 3 * 4").unwrap().eval(&no_path).unwrap(),
            Value::from(14.0)
        );
        assert_eq!(
            parse("(2 + 3) * 4").unwrap().eval(&no_path).unwrap(),
            Value::from(20.0)
        );
        assert_eq!(
            parse("10 - 2 - 3").unwrap().eval(&no_path).unwrap(),
            Value::from(5.0)
        );
        assert_eq!(
            parse("7 / 2").unwrap().eval(&no_path).unwrap(),
            Value::from(3.5)
        );
    }

    #[test]
    fn unary_and_logic() {
        assert_eq!(
            parse("-2 * 3").unwrap().eval(&no_path).unwrap(),
            Value::from(-6.0)
        );
        assert_eq!(
            parse("not true or false").unwrap().eval(&no_path).unwrap(),
            Value::from(false)
        );
        assert_eq!(
            parse("1 < 2 and not false")
                .unwrap()
                .eval(&no_path)
                .unwrap(),
            Value::from(true)
        );
    }

    #[test]
    fn division_by_zero_is_typed_error() {
        let e = parse("1 / 0").unwrap();
        assert!(matches!(
            e.eval(&no_path),
            Err(RulesError::DivisionByZero { .. })
        ));
    }

    #[test]
    fn function_errors() {
        assert!(parse("sqrt(-1)").unwrap().eval(&no_path).is_err());
        assert!(parse("abs(\"x\")").unwrap().eval(&no_path).is_err());
        assert!(parse("true - 1").unwrap().eval(&no_path).is_err());
    }

    #[test]
    fn parse_errors_are_typed() {
        assert!(parse("2 *").is_err());
        assert!(parse("(1 + 2").is_err());
        assert!(parse("1 + + 2").is_err());
    }

    #[test]
    fn string_concat_and_compare() {
        assert_eq!(
            parse("\"a\" + \"b\"").unwrap().eval(&no_path).unwrap(),
            Value::from("ab")
        );
        assert_eq!(
            parse("\"a\" < \"b\"").unwrap().eval(&no_path).unwrap(),
            Value::from(true)
        );
        assert_eq!(
            parse("\"a\" == \"a\"").unwrap().eval(&no_path).unwrap(),
            Value::from(true)
        );
    }
}
