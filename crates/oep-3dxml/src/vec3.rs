//! Minimal 3-D double-precision vector math for surface decoding.
//!
//! Kept deliberately dependency-free (no nalgebra): the workspace builds
//! offline on the NAS, and P3-M2-T01 needs only add/sub/scale/dot/cross and
//! a couple of distance helpers. P6-M2-T01 owns the general math library;
//! this module is the smallest surface-decode-usable subset.
#![deny(missing_docs)]

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// A 3-D point/vector in double precision.
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Vec3 {
    /// First coordinate.
    pub x: f64,
    /// Second coordinate.
    pub y: f64,
    /// Third coordinate.
    pub z: f64,
}

/// The zero vector.
pub const ZERO: Vec3 = Vec3 {
    x: 0.0,
    y: 0.0,
    z: 0.0,
};

impl Vec3 {
    /// Build a vector from components.
    #[inline]
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Dot product.
    #[inline]
    pub fn dot(self, o: Self) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Cross product.
    #[inline]
    pub fn cross(self, o: Self) -> Self {
        Self::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Squared Euclidean norm.
    #[inline]
    pub fn norm_sq(self) -> f64 {
        self.dot(self)
    }

    /// Euclidean norm.
    #[inline]
    pub fn norm(self) -> f64 {
        self.norm_sq().sqrt()
    }

    /// Unit vector; returns `ZERO` when the input has (near-)zero length.
    #[inline]
    pub fn normalized(self) -> Self {
        let n = self.norm();
        if n <= f64::EPSILON {
            ZERO
        } else {
            self / n
        }
    }

    /// True if every component is finite.
    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for Vec3 {
    type Output = Self;
    #[inline]
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    #[inline]
    fn sub(self, o: Self) -> Self {
        Self::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}

impl Neg for Vec3 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

impl Mul<f64> for Vec3 {
    type Output = Self;
    #[inline]
    fn mul(self, s: f64) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Div<f64> for Vec3 {
    type Output = Self;
    #[inline]
    fn div(self, s: f64) -> Self {
        Self::new(self.x / s, self.y / s, self.z / s)
    }
}

impl AddAssign for Vec3 {
    #[inline]
    fn add_assign(&mut self, o: Self) {
        *self = *self + o;
    }
}

impl fmt::Debug for Vec3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:.6}, {:.6}, {:.6}]", self.x, self.y, self.z)
    }
}
