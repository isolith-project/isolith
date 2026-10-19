//! P6-M3-T02: concrete serializable topology state record.
//!
//! [`TopologyState`] is the state record the undo/redo history
//! ([`History`](crate::history::History)) persists. It is a snapshot of the
//! topological content of an edit: a revision label, an operation label, and
//! the set of bodies (kind + bounding box) and features (kind + parameters)
//! present at that point of the edit session.
//!
//! The text form is our own documented record format (see [`TopologyState::encode`]):
//!
//! ```text
//! revision 3
//! op hole
//! bodies 2
//! body oep-body-bracket solid 0.0;0.0;0.0;80.0;40.0;6.0
//! body oep-body-lip solid 0.0;0.0;0.0;30.0;10.0;6.0
//! features 1
//! feature oep-feature-hole Hole diameter=6.0;depth=6.0
//! ```
//!
//! `encode` sorts deterministically (bodies and features by id, parameters by
//! name), so identical content always yields identical bytes. `decode` is
//! strict: unknown body kinds, missing lines, mismatched counts, or non-finite
//! or inverted bounding boxes all yield `None` (never panic).

use crate::history::HistoryState;

/// The coarse classification of a body in the snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Volumetric solid body.
    Solid,
    /// Sheet/surface body with open boundaries.
    Sheet,
    /// Wire/curve body.
    Wire,
}

impl BodyKind {
    /// The lowercase canonical tag used in the serialized record.
    fn tag(&self) -> &'static str {
        match self {
            BodyKind::Solid => "solid",
            BodyKind::Sheet => "sheet",
            BodyKind::Wire => "wire",
        }
    }

    /// Parse a tag back into a [`BodyKind`]; `None` on anything else.
    fn from_tag(tag: &str) -> Option<Self> {
        match tag {
            "solid" => Some(BodyKind::Solid),
            "sheet" => Some(BodyKind::Sheet),
            "wire" => Some(BodyKind::Wire),
            _ => None,
        }
    }
}

/// One body in a topology snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyRecord {
    /// Document-wide body id.
    pub id: String,
    /// Coarse geometry classification.
    pub kind: BodyKind,
    /// Axis-aligned bounding box `[minx, miny, minz, maxx, maxy, maxz]`.
    pub bbox: [f64; 6],
}

/// Convenience input tuple for [`TopologyState::new`]: `(id, kind, bbox)`.
pub type BodySpec = (String, BodyKind, [f64; 6]);

/// Convenience input tuple for [`TopologyState::new`]:
/// `(id, kind, params)` where params is a `(name, value)` list.
pub type FeatureSpec = (String, String, Vec<(String, f64)>);

/// One feature in a topology snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct FeatureRecord {
    /// Document-wide feature id.
    pub id: String,
    /// Feature type name (e.g. `Hole`, `Fillet`, `CounterSink`).
    pub kind: String,
    /// Name-sorted parameters (`(name, value)`).
    pub params: Vec<(String, f64)>,
}

/// A serializable snapshot of the topological content at one edit.
#[derive(Clone, Debug, PartialEq)]
pub struct TopologyState {
    revision: u32,
    op: String,
    bodies: Vec<BodyRecord>,
    features: Vec<FeatureRecord>,
}

impl TopologyState {
    /// A state snapshot. Bodies/features are normalized (sorted by id) and
    /// parameter lists sorted by name in the serialized form.
    pub fn new(
        revision: u32,
        op: impl Into<String>,
        bodies: Vec<BodySpec>,
        features: Vec<FeatureSpec>,
    ) -> Self {
        let mut bs: Vec<BodyRecord> = bodies
            .into_iter()
            .map(|(id, kind, bbox)| BodyRecord { id, kind, bbox })
            .collect();
        bs.sort_by(|a, b| a.id.cmp(&b.id));
        let mut fs: Vec<FeatureRecord> = features
            .into_iter()
            .map(|(id, kind, mut params)| {
                params.sort_by(|a, b| a.0.cmp(&b.0));
                FeatureRecord { id, kind, params }
            })
            .collect();
        fs.sort_by(|a, b| a.id.cmp(&b.id));
        Self {
            revision,
            op: op.into(),
            bodies: bs,
            features: fs,
        }
    }

    /// The revision label (edit sequence number).
    pub fn revision(&self) -> u32 {
        self.revision
    }

    /// The operation label.
    pub fn op(&self) -> &str {
        &self.op
    }

    /// The bodies in this snapshot, sorted by id.
    pub fn bodies(&self) -> &[BodyRecord] {
        &self.bodies
    }

    /// The features in this snapshot, sorted by id.
    pub fn features(&self) -> &[FeatureRecord] {
        &self.features
    }
}

impl HistoryState for TopologyState {
    fn revision(&self) -> u32 {
        TopologyState::revision(self)
    }

    fn op(&self) -> &str {
        TopologyState::op(self)
    }

    fn encode(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("revision {}\n", self.revision));
        out.push_str(&format!("op {}\n", self.op));
        out.push_str(&format!("bodies {}\n", self.bodies.len()));
        let fmt = fmt_value;
        for b in &self.bodies {
            let [minx, miny, minz, maxx, maxy, maxz] = b.bbox;
            out.push_str(&format!(
                "body {} {} {};{};{};{};{};{}\n",
                b.id,
                b.kind.tag(),
                fmt(minx),
                fmt(miny),
                fmt(minz),
                fmt(maxx),
                fmt(maxy),
                fmt(maxz)
            ));
        }
        out.push_str(&format!("features {}\n", self.features.len()));
        for f in &self.features {
            out.push_str(&format!("feature {} {}", f.id, f.kind));
            let param_strs: Vec<String> = f
                .params
                .iter()
                .map(|(name, value)| format!("{}={}", name, fmt(*value)))
                .collect();
            if !param_strs.is_empty() {
                out.push(' ');
                out.push_str(&param_strs.join(";"));
            }
            out.push('\n');
        }
        out
    }

    fn decode(text: &str) -> Option<Self> {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;

        let revision: u32 = lines.get(i)?.strip_prefix("revision ")?.parse().ok()?;
        i += 1;
        if revision == 0 {
            return None;
        }
        let op = lines.get(i)?.strip_prefix("op ")?.to_string();
        i += 1;
        if op.is_empty() {
            return None;
        }

        let nb: usize = lines.get(i)?.strip_prefix("bodies ")?.parse().ok()?;
        i += 1;
        let mut bodies: Vec<BodyRecord> = Vec::with_capacity(nb);
        for _ in 0..nb {
            let line = lines.get(i)?;
            let rest = line.strip_prefix("body ")?;
            let (id, tag, coords) = split3(rest)?;
            let kind = BodyKind::from_tag(tag)?;
            let mut vals = [0.0f64; 6];
            let parts: Vec<&str> = coords.split(';').collect();
            if parts.len() != 6 {
                return None;
            }
            for (k, part) in parts.iter().enumerate() {
                let v: f64 = part.parse().ok()?;
                if !v.is_finite() {
                    return None;
                }
                vals[k] = v;
            }
            for axis in 0..3 {
                if vals[axis] > vals[axis + 3] {
                    return None;
                }
            }
            bodies.push(BodyRecord {
                id: id.to_string(),
                kind,
                bbox: vals,
            });
            i += 1;
        }

        let nf: usize = lines.get(i)?.strip_prefix("features ")?.parse().ok()?;
        i += 1;
        let mut features: Vec<FeatureRecord> = Vec::with_capacity(nf);
        for _ in 0..nf {
            let line = lines.get(i)?;
            let rest = line.strip_prefix("feature ")?;
            let (id, kind, par) = split3(rest)?;
            let mut params: Vec<(String, f64)> = Vec::new();
            if !par.is_empty() {
                for pair in par.split(';') {
                    let (name, val_s) = pair.split_once('=')?;
                    let v: f64 = val_s.parse().ok()?;
                    if !v.is_finite() {
                        return None;
                    }
                    params.push((name.to_string(), v));
                }
            }
            params.sort_by(|a, b| a.0.cmp(&b.0));
            features.push(FeatureRecord {
                id: id.to_string(),
                kind: kind.to_string(),
                params,
            });
            i += 1;
        }

        if i != lines.len() {
            return None;
        }
        Some(TopologyState {
            revision,
            op,
            bodies,
            features,
        })
    }
}

/// Split a record line into its id token, its kind/tag token, and the rest.
fn split3(line: &str) -> Option<(&str, &str, &str)> {
    let mut it = line.splitn(3, ' ');
    let id = it.next()?;
    let tag = it.next()?;
    let rest = it.next()?;
    Some((id, tag, rest))
}

/// Deterministic number formatting: shortest round-trip, integral values get
/// `.0` so no value is ambiguous between int and float.
fn fmt_value(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{x:.1}")
    } else {
        format!("{x}")
    }
}
