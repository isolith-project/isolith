//! P6-M2-T01: vendored vector/matrix/quaternion math for `oep-geo`.
//!
//! Design decisions (see docs/p6-m2-oep-geo-math.md and the PR body):
//!
//! - **Vendored, not `nalgebra`.** std-only, zero new dependencies. No
//!   `Cargo.toml`/`Cargo.lock` churn (avoids collision with the concurrent
//!   P6-M1-T01 serde addition in this crate), no crates.io fetch risk on CI,
//!   fully auditable.
//! - **All scalars `f64`** (P6-M1-T03: double everywhere, tolerance documented).
//! - **Column-major storage** for `Mat3` (`[f64;9]`, entry `(r,c)` at `c*3+r`)
//!   and `Mat4` (`[f64;16]`, entry `(r,c)` at `c*4+r`) to match the P6-M1-T01
//!   instance `transform: [f64;16]` column-major contract. Translation lives in
//!   the last column: `arr[12..15]`.
//! - **Non-panicking error model:** [`MathError`] for degenerate/singular/
//!   non-finite inputs; constructors never propagate NaN/Inf silently.
//! - Right-hand rule for axis-angle rotations; `from_axis_angle(axis, angle)`
//!   does NOT pre-normalize the axis (a non-unit axis is normalized internally,
//!   a zero axis is rejected as [`MathError::DegenerateAxis`]).

/// Rejection/safety tolerance for "is this a unit-ish vector / non-singular matrix".
const EPS: f64 = 1e-12;

/// Typed, non-panicking math errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathError {
    /// Vector has (near) zero norm and cannot be normalized.
    InsufficientNorm,
    /// Rotation axis is degenerate (zero norm).
    DegenerateAxis,
    /// Input contains NaN or +/-Inf.
    NonFiniteInput,
    /// Matrix determinant is (near) zero; not invertible.
    SingularMatrix,
}

fn is_finite_scalar(x: f64) -> bool {
    x.is_finite()
}

/// Validate + normalize a rotation axis: non-finite -> [`MathError::NonFiniteInput`],
/// (near) zero -> [`MathError::DegenerateAxis`] (distinct from `Vec3::normalize`'s
/// `InsufficientNorm` so rotation callers get the axis-specific error).
fn checked_unit_axis(axis: Vec3) -> Result<Vec3, MathError> {
    if !is_finite_scalar(axis.x) || !is_finite_scalar(axis.y) || !is_finite_scalar(axis.z) {
        return Err(MathError::NonFiniteInput);
    }
    let n = axis.norm();
    if n <= EPS {
        return Err(MathError::DegenerateAxis);
    }
    Ok(axis.div(n))
}

// --------------------------------------------------------------------------
// Vec3
// --------------------------------------------------------------------------

/// 3D vector of `f64` coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
}

impl Vec3 {
    /// Build a vector from three coordinates.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Zero vector.
    pub fn zero() -> Self {
        Self::new(0.0, 0.0, 0.0)
    }

    /// Component-wise addition.
    pub fn add(&self, o: &Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }

    /// Component-wise subtraction.
    pub fn sub(&self, o: &Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }

    /// Negation.
    pub fn neg(&self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }

    /// Scalar multiplication.
    pub fn mul(&self, s: f64) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }

    /// Scalar division.
    pub fn div(&self, s: f64) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }

    /// Dot (scalar) product.
    pub fn dot(&self, o: &Vec3) -> f64 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Cross (vector) product. `a.cross(&b)` is anti-symmetric: `a x b = -(b x a)`.
    pub fn cross(&self, o: &Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }

    /// Euclidean norm (length).
    pub fn norm(&self) -> f64 {
        self.norm_squared().sqrt()
    }

    /// Squared Euclidean norm.
    pub fn norm_squared(&self) -> f64 {
        self.dot(self)
    }

    /// Unit vector in the same direction.
    ///
    /// Returns [`MathError::InsufficientNorm`] when the norm is (near) zero and
    /// [`MathError::NonFiniteInput`] when a component is NaN/Inf.
    pub fn normalize(&self) -> Result<Vec3, MathError> {
        if !(is_finite_scalar(self.x) && is_finite_scalar(self.y) && is_finite_scalar(self.z)) {
            return Err(MathError::NonFiniteInput);
        }
        let n = self.norm();
        if n <= EPS {
            return Err(MathError::InsufficientNorm);
        }
        Ok(Vec3::new(self.x / n, self.y / n, self.z / n))
    }

    /// Euclidean distance to another vector.
    pub fn distance_to(&self, o: &Vec3) -> f64 {
        self.sub(o).norm()
    }

    /// Linear interpolation towards `o` by factor `t` (0 = self, 1 = o).
    pub fn lerp(&self, o: &Vec3, t: f64) -> Vec3 {
        self.add(&o.sub(self).mul(t))
    }
}

// --------------------------------------------------------------------------
// Mat3
// --------------------------------------------------------------------------

/// 3x3 matrix, **column-major** storage; entry `(r, c)` is stored at `c*3 + r`.
#[derive(Debug, Clone, Copy)]
pub struct Mat3 {
    data: [f64; 9],
}

impl Mat3 {
    /// Identity matrix.
    pub fn identity() -> Self {
        Self {
            data: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        }
    }

    /// Zero matrix.
    pub fn zero() -> Self {
        Self { data: [0.0; 9] }
    }

    /// Build from three column vectors.
    pub fn from_cols(c0: Vec3, c1: Vec3, c2: Vec3) -> Self {
        Self {
            data: [c0.x, c0.y, c0.z, c1.x, c1.y, c1.z, c2.x, c2.y, c2.z],
        }
    }

    /// Entry `(row, col)`, 0-indexed.
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.data[col * 3 + row]
    }

    /// Transpose.
    pub fn transpose(&self) -> Mat3 {
        let mut d = [0.0; 9];
        for c in 0..3 {
            for r in 0..3 {
                d[r * 3 + c] = self.get(r, c);
            }
        }
        Mat3 { data: d }
    }

    /// Determinant.
    pub fn det(&self) -> f64 {
        let a = self.get(0, 0);
        let b = self.get(0, 1);
        let c = self.get(0, 2);
        let d = self.get(1, 0);
        let e = self.get(1, 1);
        let f = self.get(1, 2);
        let g = self.get(2, 0);
        let h = self.get(2, 1);
        let i = self.get(2, 2);
        a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g)
    }

    /// Matrix product `self * o`.
    pub fn mul(&self, o: &Mat3) -> Mat3 {
        let mut out = Mat3::zero();
        for c in 0..3 {
            for r in 0..3 {
                let mut s = 0.0;
                for k in 0..3 {
                    s += self.get(r, k) * o.get(k, c);
                }
                out.data[c * 3 + r] = s;
            }
        }
        out
    }

    /// Right-multiply by a column vector.
    pub fn mul_vec3(&self, v: &Vec3) -> Vec3 {
        Vec3::new(
            self.get(0, 0) * v.x + self.get(0, 1) * v.y + self.get(0, 2) * v.z,
            self.get(1, 0) * v.x + self.get(1, 1) * v.y + self.get(1, 2) * v.z,
            self.get(2, 0) * v.x + self.get(2, 1) * v.y + self.get(2, 2) * v.z,
        )
    }

    /// Inverse via adjugate/determinant. Returns [`MathError::SingularMatrix`]
    /// when `|det| <= EPS`.
    pub fn inverse(&self) -> Result<Mat3, MathError> {
        let det = self.det();
        if !det.is_finite() {
            return Err(MathError::NonFiniteInput);
        }
        if det.abs() <= EPS {
            return Err(MathError::SingularMatrix);
        }
        let inv = 1.0 / det;
        // adjugate (cofactor transpose)
        let mut d = [0.0; 9];
        for r in 0..3 {
            for c in 0..3 {
                let m00 = self.get((r + 1) % 3, (c + 1) % 3);
                let m01 = self.get((r + 1) % 3, (c + 2) % 3);
                let m10 = self.get((r + 2) % 3, (c + 1) % 3);
                let m11 = self.get((r + 2) % 3, (c + 2) % 3);
                let cof = (m00 * m11 - m01 * m10) * if (r + c) % 2 == 0 { 1.0 } else { -1.0 };
                d[c * 3 + r] = cof * inv; // adjugate is cofactor transposed
            }
        }
        Ok(Mat3 { data: d })
    }

    /// Rotation matrix about `axis` by `angle` (radians), right-hand rule.
    /// The axis is normalized internally; a zero axis yields
    /// [`MathError::DegenerateAxis`].
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Result<Mat3, MathError> {
        let u = checked_unit_axis(axis)?;
        if !angle.is_finite() {
            return Err(MathError::NonFiniteInput);
        }
        let (x, y, z) = (u.x, u.y, u.z);
        let s = angle.sin();
        let c = angle.cos();
        let c1 = 1.0 - c;
        Ok(Mat3 {
            data: [
                c + x * x * c1,
                y * x * c1 + z * s,
                z * x * c1 - y * s,
                x * y * c1 - z * s,
                c + y * y * c1,
                z * y * c1 + x * s,
                x * z * c1 + y * s,
                y * z * c1 - x * s,
                c + z * z * c1,
            ],
        })
    }
}

// --------------------------------------------------------------------------
// Mat4
// --------------------------------------------------------------------------

/// 4x4 matrix, **column-major** storage; entry `(r, c)` stored at `c*4 + r`.
///
/// Used for affine transforms (rotation/scale in the top-left 3x3, translation
/// in the last column `data[12..15]`). `data[16]`-equivalent bottom row is
/// `[0,0,0,1]` for everything this core constructs.
#[derive(Debug, Clone, Copy)]
pub struct Mat4 {
    data: [f64; 16],
}

impl Mat4 {
    /// Identity matrix.
    pub fn identity() -> Self {
        Self {
            data: [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        }
    }

    /// Zero matrix.
    pub fn zero() -> Self {
        Self { data: [0.0; 16] }
    }

    /// Pure translation by vector `t`.
    pub fn from_translation(t: Vec3) -> Self {
        let mut m = Self::identity();
        m.data[12] = t.x;
        m.data[13] = t.y;
        m.data[14] = t.z;
        m
    }

    /// Embed a 3x3 linear (rotation/scale) matrix, zero translation.
    pub fn from_rotation(r: Mat3) -> Self {
        let mut m = Self::identity();
        for c in 0..3 {
            for r_ in 0..3 {
                m.data[c * 4 + r_] = r.get(r_, c);
            }
        }
        m
    }

    /// Uniform scale.
    pub fn from_scale(s: f64) -> Self {
        let mut m = Self::identity();
        m.data[0] = s;
        m.data[5] = s;
        m.data[10] = s;
        m
    }

    /// Rotation about `axis` by `angle` (radians), right-hand rule, zero translation.
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Result<Mat4, MathError> {
        Ok(Self::from_rotation(Mat3::from_axis_angle(axis, angle)?))
    }

    /// Entry `(row, col)`, 0-indexed.
    #[allow(dead_code)]
    pub fn get(&self, row: usize, col: usize) -> f64 {
        self.data[col * 4 + row]
    }

    /// Matrix product `self * o`.
    pub fn mul(&self, o: &Mat4) -> Mat4 {
        let mut out = Mat4::zero();
        for c in 0..4 {
            for r in 0..4 {
                let mut s = 0.0;
                for k in 0..4 {
                    s += self.data[k * 4 + r] * o.data[c * 4 + k];
                }
                out.data[c * 4 + r] = s;
            }
        }
        out
    }

    /// Transform a point (affine, w=1): `R*p + t`.
    pub fn transform_point(&self, p: &Vec3) -> Vec3 {
        let x = p.x;
        let y = p.y;
        let z = p.z;
        Vec3::new(
            self.data[0] * x + self.data[4] * y + self.data[8] * z + self.data[12],
            self.data[1] * x + self.data[5] * y + self.data[9] * z + self.data[13],
            self.data[2] * x + self.data[6] * y + self.data[10] * z + self.data[14],
        )
    }

    /// Transform a direction/vector (affine, w=0): `R*v`, translation ignored.
    pub fn transform_vector(&self, v: &Vec3) -> Vec3 {
        let x = v.x;
        let y = v.y;
        let z = v.z;
        Vec3::new(
            self.data[0] * x + self.data[4] * y + self.data[8] * z,
            self.data[1] * x + self.data[5] * y + self.data[9] * z,
            self.data[2] * x + self.data[6] * y + self.data[10] * z,
        )
    }

    /// Transpose.
    pub fn transpose(&self) -> Mat4 {
        let mut d = [0.0; 16];
        for c in 0..4 {
            for r in 0..4 {
                d[r * 4 + c] = self.data[c * 4 + r];
            }
        }
        Mat4 { data: d }
    }

    /// Determinant.
    pub fn det(&self) -> f64 {
        // cofactor expansion along row 0 (general 4x4)
        let mut s = 0.0;
        for c in 0..4 {
            let minor = self.minor(0, c);
            let cof = minor * if c % 2 == 0 { 1.0 } else { -1.0 };
            s += self.data[c * 4] * cof;
        }
        s
    }

    fn minor(&self, row: usize, col: usize) -> f64 {
        let mut sub = [[0.0; 3]; 3];
        let mut sr = 0;
        for r in 0..4 {
            if r == row {
                continue;
            }
            let mut sc = 0;
            for c in 0..4 {
                if c == col {
                    continue;
                }
                sub[sr][sc] = self.data[c * 4 + r];
                sc += 1;
            }
            sr += 1;
        }
        sub[0][0] * (sub[1][1] * sub[2][2] - sub[1][2] * sub[2][1])
            - sub[0][1] * (sub[1][0] * sub[2][2] - sub[1][2] * sub[2][0])
            + sub[0][2] * (sub[1][0] * sub[2][1] - sub[1][1] * sub[2][0])
    }

    /// Inverse via adjugate/determinant. Returns [`MathError::SingularMatrix`]
    /// when `|det| <= EPS`, [`MathError::NonFiniteInput`] if the determinant is
    /// NaN/Inf.
    pub fn inverse(&self) -> Result<Mat4, MathError> {
        let det = self.det();
        if !det.is_finite() {
            return Err(MathError::NonFiniteInput);
        }
        if det.abs() <= EPS {
            return Err(MathError::SingularMatrix);
        }
        let inv = 1.0 / det;
        let mut d = [0.0; 16];
        // inverse[r][c] = cofactor(c, r) / det = minor(c, r) * sign(c+r) / det,
        // where minor(c,r) removes row c, column r. Stored column-major at c*4+r.
        for c in 0..4 {
            for r in 0..4 {
                let m = self.minor(c, r);
                let cof = m * if (c + r) % 2 == 0 { 1.0 } else { -1.0 };
                d[c * 4 + r] = cof * inv;
            }
        }
        Ok(Mat4 { data: d })
    }

    /// Whether every entry is finite.
    pub fn is_finite(&self) -> bool {
        self.data.iter().all(|x| x.is_finite())
    }

    /// Column-major `[f64;16]`, matching the P6-M1-T01 instance-transform contract.
    pub fn to_array16(&self) -> [f64; 16] {
        self.data
    }

    /// Build from a column-major `[f64;16]`.
    ///
    /// Returns [`MathError::NonFiniteInput`] if any entry is NaN/Inf.
    pub fn from_array16(a: [f64; 16]) -> Result<Mat4, MathError> {
        if a.iter().any(|x| !x.is_finite()) {
            return Err(MathError::NonFiniteInput);
        }
        Ok(Mat4 { data: a })
    }
}

// --------------------------------------------------------------------------
// Quat
// --------------------------------------------------------------------------

/// Unit quaternion `w + x*i + y*j + z*k` storing `(w, x, y, z)`.
///
/// Constructors from axis/angle or rotation matrices produce a unit quaternion;
/// [`Quat::inverse`] = conjugate for a unit quaternion (handles near-zero norm
/// defensively by attempting normalization first).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quat {
    /// Real part.
    pub w: f64,
    /// Imaginary i component.
    pub x: f64,
    /// Imaginary j component.
    pub y: f64,
    /// Imaginary k component.
    pub z: f64,
}

impl Quat {
    /// Identity quaternion (no rotation).
    pub fn identity() -> Self {
        Self {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Build from explicit `(w, x, y, z)`. Not normalized automatically; call
    /// [`Quat::normalize`] if the source may be non-unit.
    pub fn new(w: f64, x: f64, y: f64, z: f64) -> Self {
        Self { w, x, y, z }
    }

    /// Unit quaternion representing a rotation of `angle` (radians) about
    /// `axis` (right-hand rule). `axis` is normalized internally; a zero axis
    /// yields [`MathError::DegenerateAxis`], a non-finite component yields
    /// [`MathError::NonFiniteInput`].
    pub fn from_axis_angle(axis: Vec3, angle: f64) -> Result<Quat, MathError> {
        let u = checked_unit_axis(axis)?;
        if !angle.is_finite() {
            return Err(MathError::NonFiniteInput);
        }
        let half = angle / 2.0;
        let s = half.sin();
        Ok(Quat {
            w: half.cos(),
            x: u.x * s,
            y: u.y * s,
            z: u.z * s,
        })
    }

    /// Extract the unit quaternion from a rotation matrix (Shepperd's method).
    pub fn from_rotation_matrix(m: Mat3) -> Quat {
        let trace = m.get(0, 0) + m.get(1, 1) + m.get(2, 2);
        if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            Quat {
                w: 0.25 * s,
                x: (m.get(2, 1) - m.get(1, 2)) / s,
                y: (m.get(0, 2) - m.get(2, 0)) / s,
                z: (m.get(1, 0) - m.get(0, 1)) / s,
            }
        } else if m.get(0, 0) > m.get(1, 1) && m.get(0, 0) > m.get(2, 2) {
            let s = (1.0 + m.get(0, 0) - m.get(1, 1) - m.get(2, 2)).sqrt() * 2.0;
            Quat {
                w: (m.get(2, 1) - m.get(1, 2)) / s,
                x: 0.25 * s,
                y: (m.get(0, 1) + m.get(1, 0)) / s,
                z: (m.get(0, 2) + m.get(2, 0)) / s,
            }
        } else if m.get(1, 1) > m.get(2, 2) {
            let s = (1.0 + m.get(1, 1) - m.get(0, 0) - m.get(2, 2)).sqrt() * 2.0;
            Quat {
                w: (m.get(0, 2) - m.get(2, 0)) / s,
                x: (m.get(0, 1) + m.get(1, 0)) / s,
                y: 0.25 * s,
                z: (m.get(1, 2) + m.get(2, 1)) / s,
            }
        } else {
            let s = (1.0 + m.get(2, 2) - m.get(0, 0) - m.get(1, 1)).sqrt() * 2.0;
            Quat {
                w: (m.get(1, 0) - m.get(0, 1)) / s,
                x: (m.get(0, 2) + m.get(2, 0)) / s,
                y: (m.get(1, 2) + m.get(2, 1)) / s,
                z: 0.25 * s,
            }
        }
    }

    /// Hamilton (quaternion) product `self * o`. Multiplication order matches
    /// composition of rotations for unit quaternions.
    pub fn mul(&self, o: &Quat) -> Quat {
        Quat {
            w: self.w * o.w - self.x * o.x - self.y * o.y - self.z * o.z,
            x: self.w * o.x + self.x * o.w + self.y * o.z - self.z * o.y,
            y: self.w * o.y - self.x * o.z + self.y * o.w + self.z * o.x,
            z: self.w * o.z + self.x * o.y - self.y * o.x + self.z * o.w,
        }
    }

    /// Conjugate (`w, -x, -y, -z`).
    pub fn conjugate(&self) -> Quat {
        Quat {
            w: self.w,
            x: -self.x,
            y: -self.y,
            z: -self.z,
        }
    }

    /// Norm (length).
    pub fn norm(&self) -> f64 {
        (self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Normalize to unit length. Returns [`MathError::InsufficientNorm`] on a
    /// (near) zero quaternion, [`MathError::NonFiniteInput`] on NaN/Inf.
    pub fn normalize(&self) -> Result<Quat, MathError> {
        if !is_finite_scalar(self.w)
            || !is_finite_scalar(self.x)
            || !is_finite_scalar(self.y)
            || !is_finite_scalar(self.z)
        {
            return Err(MathError::NonFiniteInput);
        }
        let n = self.norm();
        if n <= EPS {
            return Err(MathError::InsufficientNorm);
        }
        Ok(Quat::new(self.w / n, self.x / n, self.y / n, self.z / n))
    }

    /// Inverse. For a unit quaternion this is the conjugate; a slightly-off-unit
    /// quaternion is normalized defensively, and a (near) zero quaternion yields
    /// [`MathError::InsufficientNorm`].
    pub fn inverse(&self) -> Result<Quat, MathError> {
        let n2 = self.w * self.w + self.x * self.x + self.y * self.y + self.z * self.z;
        if !n2.is_finite() {
            return Err(MathError::NonFiniteInput);
        }
        if n2.abs() <= EPS {
            return Err(MathError::InsufficientNorm);
        }
        Ok(Quat {
            w: self.w / n2,
            x: -self.x / n2,
            y: -self.y / n2,
            z: -self.z / n2,
        })
    }

    /// Rotate a vector by this (assumed unit) quaternion: `v' = q v q*`.
    pub fn rotate_vector(&self, v: &Vec3) -> Vec3 {
        let qv = Vec3::new(self.x, self.y, self.z);
        let t = qv.cross(v).mul(2.0);
        v.add(&t.mul(self.w)).add(&qv.cross(&t))
    }

    /// Equivalent 3x3 rotation matrix.
    pub fn to_matrix3(&self) -> Mat3 {
        let (w, x, y, z) = (self.w, self.x, self.y, self.z);
        Mat3::from_cols(
            Vec3::new(
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y + w * z),
                2.0 * (x * z - w * y),
            ),
            Vec3::new(
                2.0 * (x * y - w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z + w * x),
            ),
            Vec3::new(
                2.0 * (x * z + w * y),
                2.0 * (y * z - w * x),
                1.0 - 2.0 * (x * x + y * y),
            ),
        )
    }

    /// Axis-angle representation `(axis, angle_rad)`, angle in `[0, PI]`.
    /// For the identity quaternion (or the `-q` double cover), the axis is
    /// arbitrary.
    pub fn to_axis_angle(&self) -> (Vec3, f64) {
        let n = self.norm();
        if n <= EPS {
            return (Vec3::new(1.0, 0.0, 0.0), 0.0);
        }
        let (w, x, y, z) = (self.w / n, self.x / n, self.y / n, self.z / n);
        let w_clamped = w.clamp(-1.0, 1.0);
        let angle = 2.0 * w_clamped.acos();
        let s = (angle / 2.0).sin();
        if s.abs() <= EPS {
            (Vec3::new(1.0, 0.0, 0.0), 0.0)
        } else {
            let axis = Vec3::new(x, y, z)
                .div(angle.sin())
                .normalize()
                .unwrap_or(Vec3::new(1.0, 0.0, 0.0));
            (axis, angle)
        }
    }

    /// Quaternion dot product.
    pub fn dot(&self, o: &Quat) -> f64 {
        self.w * o.w + self.x * o.x + self.y * o.y + self.z * o.z
    }

    /// Spherical linear interpolation toward `o` by `t` in `[0,1]`. Chooses the
    /// shortest path (`-q` handling) and falls back to nlerp near the endpoints
    /// where the sin(\*)/sin(\*) formula is ill-conditioned.
    pub fn slerp(&self, o: &Quat, t: f64) -> Quat {
        let start = *self;
        let mut end = *o;
        let mut cos = start.dot(&end);

        if cos < 0.0 {
            end = end.neg();
            cos = -cos;
        }

        if cos > 1.0 - 1e-6 {
            // near-parallel: linear interpolation + normalize
            let nlerp = Quat::new(
                start.w + (end.w - start.w) * t,
                start.x + (end.x - start.x) * t,
                start.y + (end.y - start.y) * t,
                start.z + (end.z - start.z) * t,
            );
            return nlerp.normalize().unwrap_or(Quat::identity());
        }

        let theta = cos.clamp(-1.0, 1.0).acos();
        let sin_theta = theta.sin();
        let a = ((1.0 - t) * theta).sin() / sin_theta;
        let b = (t * theta).sin() / sin_theta;
        Quat::new(
            start.w * a + end.w * b,
            start.x * a + end.x * b,
            start.y * a + end.y * b,
            start.z * a + end.z * b,
        )
    }

    fn neg(&self) -> Quat {
        Quat::new(-self.w, -self.x, -self.y, -self.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    #[test]
    fn identity_quat_is_identity_rotation() {
        let q = Quat::identity();
        let v = Vec3::new(1.0, 2.0, 3.0);
        let out = q.rotate_vector(&v);
        assert!((out.x - v.x).abs() < 1e-12);
        assert!((out.y - v.y).abs() < 1e-12);
        assert!((out.z - v.z).abs() < 1e-12);
    }

    #[test]
    fn quat_to_axis_angle_identity() {
        let (axis, angle) = Quat::identity().to_axis_angle();
        assert_eq!(angle, 0.0);
        assert_eq!(axis, Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn mat4_mul_by_identity_is_identity() {
        let m = Mat4::from_translation(Vec3::new(3.0, -1.0, 2.0));
        let id = m.mul(&Mat4::identity());
        for i in 0..16 {
            assert!((id.to_array16()[i] - m.to_array16()[i]).abs() < 1e-12);
        }
    }

    #[test]
    fn rotation_det_is_one() {
        let m = Mat3::from_axis_angle(Vec3::new(0.3, -0.7, 4.0).normalize().unwrap(), PI / 3.0)
            .unwrap();
        assert!((m.det() - 1.0).abs() < 1e-9);
    }
}
