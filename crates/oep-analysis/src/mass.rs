//! Mass properties for solid bodies: volume, mass, centroid, moments of
//! inertia — verified against the analytical solid within a committed
//! tolerance.
//!
//! At the P6-M1 milestone a solid body's only geometry slot is its
//! axis-aligned bounding box ([`oep_geo::BoundingBox`]), so the exact kernel
//! path here integrates the **box** — an analytical solid with closed-form
//! volume, centroid, and moments of inertia. Every result is checked against
//! that closed form with the committed [`oep_geo::tolerance::Tolerance`]
//! model (default `(1e-9, 1e-9)` absolute+relative), so a regression in any
//! term is caught by the golden tests in `tests/mass_golden.rs`.
//!
//! Units: geometry is stored in the document's length unit (canonical base
//! millimetre); volume is reported in mm³; density enters in g/cm³ (the
//! engineering convention, steel ≈ 7.85) and mass is reported in grams.
//! Length conversion uses [`oep_geo::units::LengthUnit`] so a document in,
//! say, inches still computes correctly.

use oep_geo::units::LengthUnit;
use oep_geo::{BodyKind, BoundingBox, Document};

/// Error type for mass-property computation. Total: never panics, every
/// failure path is a typed variant.
#[derive(Debug, Clone, PartialEq)]
pub enum MassError {
    /// The body id does not exist in the document.
    BodyNotFound(String),
    /// The body is not a volumetric solid (only solids have mass).
    NotSolid { body: String, kind: BodyKind },
    /// The solid body carries no bounding box (no geometry slot yet).
    NoBoundingBox(String),
    /// The bounding box is malformed (non-finite or min > max).
    InvalidBoundingBox { body: String, bbox: BoundingBox },
    /// The density is missing (no material metadata) or not finite/positive.
    InvalidDensity { body: String, density: Option<f64> },
}

impl std::fmt::Display for MassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MassError::BodyNotFound(id) => write!(f, "body not found: {id}"),
            MassError::NotSolid { body, kind } => {
                write!(f, "body {body:?} is not a solid (kind {kind:?})")
            }
            MassError::NoBoundingBox(id) => {
                write!(
                    f,
                    "solid body {id:?} has no bounding box (no geometry slot)"
                )
            }
            MassError::InvalidBoundingBox { body, .. } => {
                write!(f, "solid body {body:?} has an invalid bounding box")
            }
            MassError::InvalidDensity { body, density } => match density {
                Some(_) => write!(
                    f,
                    "density for body {body:?} is not a positive finite number"
                ),
                None => write!(f, "no density metadata for body {body:?}"),
            },
        }
    }
}

impl std::error::Error for MassError {}

/// The committed tolerance model (P6-M1-T03 defaults: absolute 1e-9,
/// relative 1e-9 in the model's length units).
pub fn mass_tolerance() -> oep_geo::tolerance::Tolerance {
    oep_geo::tolerance::Tolerance::default()
}

/// The result of evaluating mass properties of one solid body.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassProperties {
    /// Volume of the solid in cubic millimetres (mm³).
    pub volume_mm3: f64,
    /// Mass in grams (g).
    pub mass_g: f64,
    /// Centroid in the body's local frame (document units).
    pub centroid: [f64; 3],
    /// Moments of inertia about the centroid, in the body's local frame, in
    /// g·mm². Layout is the symmetric tensor lower triangle:
    /// `[Ixx, Iyy, Izz, Ixy, Ixz, Iyz]`. For an axis-aligned box the
    /// products are exactly zero.
    pub inertia: [f64; 6],
}

impl MassProperties {
    /// Principal moments about the centroid (g·mm²), ordered [Ixx, Iyy, Izz].
    pub fn principal_moments(&self) -> [f64; 3] {
        [self.inertia[0], self.inertia[1], self.inertia[2]]
    }

    /// Verify the computed properties against an analytical reference within
    /// the committed tolerance: every scalar term must match. Returns the
    /// first mismatching term name + the measured absolute error, else `None`.
    ///
    /// Both periodic distinction between `volume`/`mass`/`centroid` and the
    /// six inertia terms is checked.
    pub fn verify_against(&self, reference: &MassProperties) -> Option<(&'static str, f64)> {
        let tol = mass_tolerance();
        let check = |name: &'static str, a: f64, b: f64| -> Option<(&'static str, f64)> {
            if tol.approx_eq(a, b) {
                None
            } else {
                Some((name, (a - b).abs()))
            }
        };
        check("volume", self.volume_mm3, reference.volume_mm3)
            .or_else(|| check("mass", self.mass_g, reference.mass_g))
            .or_else(|| {
                for i in 0..3 {
                    if !tol.approx_eq(self.centroid[i], reference.centroid[i]) {
                        return Some((
                            "centroid",
                            (self.centroid[i] - reference.centroid[i]).abs(),
                        ));
                    }
                }
                None
            })
            .or_else(|| {
                for (i, name) in ["Ixx", "Iyy", "Izz", "Ixy", "Ixz", "Iyz"]
                    .iter()
                    .enumerate()
                {
                    if let Some(m) = check(name, self.inertia[i], reference.inertia[i]) {
                        return Some(m);
                    }
                }
                None
            })
    }
}

/// The analytical closed form for a box solid, used as the verification
/// reference. `bbox` is in arbitrary units; `density_g_cm3` in g/cm³.
///
/// For a box of extents `(lx, ly, lz)` and mass `m` about its centroid:
///
/// - `Ixx = m/12 (ly² + lz²)`, `Iyy = m/12 (lx² + lz²)`, `Izz = m/12 (lx² + ly²)`;
/// - products of inertia are exactly zero (axes aligned with the box).
pub fn box_analytical(bbox: &BoundingBox, density_g_cm3: f64) -> MassProperties {
    let [x0, y0, z0] = bbox.min;
    let [x1, y1, z1] = bbox.max;
    let lx = x1 - x0;
    let ly = y1 - y0;
    let lz = z1 - z0;
    // Volume in mm³ (bbox in mm) then to cm³ = /1000.
    let volume_cm3 = (lx * ly * lz) / 1000.0;
    let mass_g = density_g_cm3 * volume_cm3;
    let volume_mm3 = lx * ly * lz;
    let inertia = [
        mass_g / 12.0 * (ly * ly + lz * lz),
        mass_g / 12.0 * (lx * lx + lz * lz),
        mass_g / 12.0 * (lx * lx + ly * ly),
        0.0,
        0.0,
        0.0,
    ];
    MassProperties {
        volume_mm3,
        mass_g,
        centroid: [(x0 + x1) / 2.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0],
        inertia,
    }
}

/// Compute the mass properties of a solid body in a document, sourcing
/// density from the material metadata table.
///
/// Errors:
/// - [`MassError::BodyNotFound`]: no body with that id.
/// - [`MassError::NotSolid`]: body exists but `kind != solid`.
/// - [`MassError::NoBoundingBox`] / [`MassError::InvalidBoundingBox`].
/// - [`MassError::InvalidDensity`]: no material assignment / no density.
pub fn body_mass_properties(
    doc: &Document,
    density_for_body: &dyn Fn(&str) -> Option<f64>,
    body_id: &str,
) -> Result<MassProperties, MassError> {
    let body = doc
        .bodies
        .iter()
        .find(|b| b.id == body_id)
        .ok_or_else(|| MassError::BodyNotFound(body_id.to_string()))?;
    if body.kind != BodyKind::Solid {
        return Err(MassError::NotSolid {
            body: body_id.to_string(),
            kind: body.kind,
        });
    }
    let bbox = body
        .bbox
        .ok_or_else(|| MassError::NoBoundingBox(body_id.to_string()))?;
    if !bbox.is_valid() {
        return Err(MassError::InvalidBoundingBox {
            body: body_id.to_string(),
            bbox,
        });
    }
    let density = density_for_body(body_id);
    let density = match density {
        Some(d) if d.is_finite() && d > 0.0 => d,
        _ => {
            return Err(MassError::InvalidDensity {
                body: body_id.to_string(),
                density,
            })
        }
    };
    Ok(box_analytical(&bbox, density))
}

/// Convert a volume in the document's length unit to cubic millimetres.
/// Unknown units are treated as millimetres (the canonical base), never an
/// error — the canonical-base fallback is documented by [`LengthUnit::parse`].
pub fn volume_to_mm3(units: &str, volume_units3: f64) -> f64 {
    let unit = LengthUnit::parse(units).unwrap_or(LengthUnit::Millimetre);
    let factor = LengthUnit::to_mm(unit, 1.0);
    volume_units3 * factor * factor * factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_closed_form_verifies_itself() {
        let bbox = BoundingBox::new([0.0, 0.0, 0.0], [80.0, 40.0, 6.0]);
        let computed = box_analytical(&bbox, 7.85);
        // Volume = 80*40*6 mm³ = 19200; mass = 7.85 * 19.2 = 150.72 g.
        assert!(mass_tolerance().approx_eq(computed.volume_mm3, 19200.0));
        assert!(mass_tolerance().approx_eq(computed.mass_g, 150.72));
        assert!(mass_tolerance().approx_eq(computed.centroid[0], 40.0));
        assert!(mass_tolerance().approx_eq(computed.centroid[2], 3.0));
        // Izz = m/12 (80² + 40²) = 150.72/12 * 8000 = 100480.
        assert!(mass_tolerance().approx_eq(computed.inertia[2], 100480.0));
        assert!(computed.verify_against(&computed).is_none());
    }

    #[test]
    fn known_analytical_solid_within_tolerance() {
        // A 10×10×10 mm cube of steel is the canonical analytical solid.
        let bbox = BoundingBox::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        let computed = box_analytical(&bbox, 7.85);
        let closed = MassProperties {
            volume_mm3: 1000.0,
            mass_g: 7.85,
            centroid: [5.0, 5.0, 5.0],
            inertia: [
                7.85 / 12.0 * (100.0 + 100.0),
                7.85 / 12.0 * (100.0 + 100.0),
                7.85 / 12.0 * (100.0 + 100.0),
                0.0,
                0.0,
                0.0,
            ],
        };
        assert!(computed.verify_against(&closed).is_none());
    }

    #[test]
    fn volume_unit_conversion() {
        assert!(mass_tolerance().approx_eq(volume_to_mm3("mm", 1000.0), 1000.0));
        // 1 cm³ = 1000 mm³.
        assert!(mass_tolerance().approx_eq(volume_to_mm3("cm", 1.0), 1000.0));
    }
}
