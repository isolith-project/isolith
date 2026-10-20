//! Geometric anchors: named local-frame origin + direction points on parts.
//!
//! The canonical oep-geo schema (P6-M1-T01) models parts, bodies and a
//! bounding box per body, but no face geometry yet (real B-Rep faces land in
//! P5). The assembly workbench therefore attaches *anchors* to parts: a named
//! origin point and a named direction, both in part-local coordinates. A mate
//! references `(instance, anchor)`. Default anchors are derived deterministically
//! from the part's body-union bounding box so real oep-geo models can be
//! constrained without per-model anchor files; callers may override or extend
//! them with an [`AnchorRegistry`].

use oep_geo::Document;
use serde::{Deserialize, Serialize};

/// A part-local reference point and direction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Anchor {
    /// Anchor name (e.g. "top", "bottom", "origin").
    pub name: String,
    /// Part-local origin coordinates.
    pub origin: [f64; 3],
    /// Part-local direction.
    pub normal: [f64; 3],
}
impl Anchor {
    pub fn new(name: impl Into<String>, origin: [f64; 3], normal: [f64; 3]) -> Self {
        Self {
            name: name.into(),
            origin,
            normal,
        }
    }

    /// True when the anchor direction is along the part's local Z axis
    /// (plus or minus). Only such anchors may serve as orientation
    /// references for a *moving* instance (see `workstreams/modeler/assembly/solver-adr.md`,
    /// section "Workbench anchor model").
    pub fn is_z_axis(&self) -> bool {
        let n = &self.normal;
        let xz = n[0].abs();
        let yz = n[1].abs();
        let zz = n[2].abs();
        xz < 1e-9 && yz < 1e-9 && (zz - 1.0).abs() < 1e-9
    }

    /// Sign of the anchor direction along local Z: +1 (along +Z),
    /// -1 (along -Z), 0 (not a Z-axis anchor).
    pub fn z_sign(&self) -> f64 {
        if self.is_z_axis() {
            if self.normal[2] > 0.0 {
                1.0
            } else {
                -1.0
            }
        } else {
            0.0
        }
    }
}

/// Explicit per-part anchors that override or extend the derived defaults.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct AnchorRegistry {
    pub parts: Vec<PartAnchors>,
}

/// Anchors for one part id.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PartAnchors {
    pub part_id: String,
    pub anchors: Vec<Anchor>,
}

impl AnchorRegistry {
    pub fn new() -> Self {
        Self { parts: Vec::new() }
    }

    pub fn push(&mut self, part_id: impl Into<String>, anchors: Vec<Anchor>) {
        self.parts.push(PartAnchors {
            part_id: part_id.into(),
            anchors,
        });
    }

    fn anchors_for_part(&self, part_id: &str) -> Option<&[Anchor]> {
        self.parts
            .iter()
            .find(|p| p.part_id == part_id)
            .map(|p| p.anchors.as_slice())
    }
}

/// The default anchor set for a part, derived from the body-union bounding
/// box of the oep-geo document.
pub fn derived_anchors(doc: &Document, part_id: &str) -> Vec<Anchor> {
    let Some(part) = doc.parts.iter().find(|p| p.id == part_id) else {
        return Vec::new();
    };
    // Union of the bounding boxes of the part's bodies.
    let mut lo = [f64::INFINITY; 3];
    let mut hi = [f64::NEG_INFINITY; 3];
    let mut any = false;
    for body in &doc.bodies {
        if !part.body_ids.iter().any(|id| id == &body.id) {
            continue;
        }
        let Some(bbox) = &body.bbox else {
            // Bodies without a bounding box cannot contribute anchors yet.
            continue;
        };
        for i in 0..3 {
            lo[i] = lo[i].min(bbox.min[i]);
            hi[i] = hi[i].max(bbox.max[i]);
        }
        any = true;
    }
    if !any {
        return Vec::new();
    }
    let cx = (lo[0] + hi[0]) / 2.0;
    let cy = (lo[1] + hi[1]) / 2.0;
    let cz = (lo[2] + hi[2]) / 2.0;
    vec![
        Anchor::new("origin", [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
        Anchor::new("center", [cx, cy, cz], [0.0, 0.0, 1.0]),
        Anchor::new("top", [cx, cy, hi[2]], [0.0, 0.0, 1.0]),
        Anchor::new("bottom", [cx, cy, lo[2]], [0.0, 0.0, -1.0]),
        Anchor::new("right", [hi[0], cy, cz], [1.0, 0.0, 0.0]),
        Anchor::new("left", [lo[0], cy, cz], [-1.0, 0.0, 0.0]),
        Anchor::new("front", [cx, lo[1], cz], [0.0, -1.0, 0.0]),
        Anchor::new("back", [cx, hi[1], cz], [0.0, 1.0, 0.0]),
        // Inward-facing face planes: the anchor normal points INTO the part's
        // bounding box, so a signed plane-distance with a positive value puts
        // the constrained point on the normal's side (matches the adopted
        // solver's signed-distance convention exactly).
        Anchor::new("inX", [lo[0], cy, cz], [1.0, 0.0, 0.0]),
        Anchor::new("inY", [cx, lo[1], cz], [0.0, 1.0, 0.0]),
        Anchor::new("inZ", [cx, cy, lo[2]], [0.0, 0.0, 1.0]),
        Anchor::new("outX", [hi[0], cy, cz], [-1.0, 0.0, 0.0]),
        Anchor::new("outY", [cx, hi[1], cz], [0.0, -1.0, 0.0]),
        Anchor::new("outZ", [cx, cy, hi[2]], [0.0, 0.0, -1.0]),
    ]
}

/// Anchors for a part: explicit registry entries win if present, otherwise
/// the derived defaults. Explicit entries REPLACE the derived set for that
/// part (they are an escape hatch for parts with meaningful directions).
pub fn anchors_for(doc: &Document, registry: &AnchorRegistry, part_id: &str) -> Vec<Anchor> {
    match registry.anchors_for_part(part_id) {
        Some(anchors) => anchors.to_vec(),
        None => derived_anchors(doc, part_id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny_doc() -> Document {
        let mut d = Document::new("mm");
        d.push_body(oep_geo::Body {
            id: "b1".into(),
            name: "box".into(),
            kind: oep_geo::BodyKind::Solid,
            bbox: Some(oep_geo::BoundingBox {
                min: [0.0, 0.0, 0.0],
                max: [40.0, 30.0, 10.0],
            }),
            feature_ids: Vec::new(),
        });
        d.push_part(oep_geo::Part {
            id: "p1".into(),
            name: "plate".into(),
            part_number: Some("P-1".into()),
            body_ids: vec!["b1".into()],
            feature_ids: Vec::new(),
        });
        d
    }

    #[test]
    fn derived_anchors_from_bounds() {
        let d = tiny_doc();
        let a = derived_anchors(&d, "p1");
        let by = |n: &str| a.iter().find(|x| x.name == n).unwrap();
        assert_eq!(by("origin").origin, [0.0, 0.0, 0.0]);
        assert_eq!(by("center").origin, [20.0, 15.0, 5.0]);
        assert_eq!(by("top").origin, [20.0, 15.0, 10.0]);
        assert_eq!(by("bottom").origin, [20.0, 15.0, 0.0]);
        assert_eq!(by("right").origin, [40.0, 15.0, 5.0]);
        assert!(by("bottom").is_z_axis());
        assert_eq!(by("bottom").z_sign(), -1.0);
        assert_eq!(by("top").z_sign(), 1.0);
        assert!(!by("right").is_z_axis());
    }

    #[test]
    fn registry_overrides_derived() {
        let d = tiny_doc();
        let mut reg = AnchorRegistry::new();
        reg.push(
            "p1",
            vec![Anchor::new("custom", [1.0, 2.0, 3.0], [0.0, 0.0, 1.0])],
        );
        let a = anchors_for(&d, &reg, "p1");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].name, "custom");
    }

    #[test]
    fn missing_part_yields_empty() {
        let d = tiny_doc();
        assert!(derived_anchors(&d, "nope").is_empty());
    }
}
