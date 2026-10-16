//! 4x4 row-major affine transforms used to place [`crate::Instance`]s inside
//! assemblies.
//!
//! The transform is our own documented convention (see
//! `docs/p6-m1-oep-geo-model-schema.md`): a row-major 4x4 matrix of 16
//! IEEE-754 doubles, translation in elements 12/13/14 and the homogeneous row
//! element 15 fixed to `1.0` for a valid affine transform. Identity maps the
//! child onto the parent frame unchanged.

use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

/// A 4x4 row-major affine transform (16 doubles, translation at [12]/[13]/[14],
/// affine `w` at [15] = 1.0).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Transform(pub [f64; 16]);

impl Transform {
    /// Number of stored components (16 for the 4x4 matrix).
    pub const COMPONENT_COUNT: usize = 16;

    /// The identity transform: no rotation, scale, or translation.
    pub fn identity() -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    /// A pure translation by `(x, y, z)` (grams-compatible with identity).
    pub fn translation(x: f64, y: f64, z: f64) -> Self {
        let mut m = Self::identity();
        m.0[12] = x;
        m.0[13] = y;
        m.0[14] = z;
        m
    }

    /// Borrow the 16 raw components.
    pub fn components(&self) -> &[f64; 16] {
        &self.0
    }
}

impl Index<usize> for Transform {
    type Output = f64;
    fn index(&self, i: usize) -> &f64 {
        &self.0[i]
    }
}

impl IndexMut<usize> for Transform {
    fn index_mut(&mut self, i: usize) -> &mut f64 {
        &mut self.0[i]
    }
}

impl PartialEq<[f64; 16]> for Transform {
    fn eq(&self, other: &[f64; 16]) -> bool {
        self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use super::Transform;

    #[test]
    fn identity_layout() {
        assert_eq!(
            Transform::identity().components(),
            &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
        );
    }

    #[test]
    fn translation_layout() {
        assert_eq!(Transform::translation(2.0, 3.0, 4.0).components()[12], 2.0);
    }
}
