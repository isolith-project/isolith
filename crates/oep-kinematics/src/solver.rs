//! The own constraint solver: damped Gauss-Newton least squares.
//!
//! This is a dependency-free reference solver written for the P7-M3-T02
//! mates/kinematics demo. It solves the mechanism position problem
//!
//!   minimize_x  || F(x) ||^2
//!
//! where `x` packs the 6 position-analysis unknowns per movable body and `F`
//! is the concatenated mate residual over the mechanism's joints. It is NOT
//! the production assembly constraint solver (that choice is P7-M3-T01's ADR);
//! it exists to make the mate semantics executable, testable and recordable.
//!
//! Implementation notes:
//! - Jacobian by central finite differences (analytic Jacobians are not needed
//!   at demo scale; correctness is what matters here).
//! - Normal equations `(J^T J + lambda * diag(J^T J)) * dx = -J^T F` solved by
//!   our own Gaussian elimination with partial pivoting.
//! - Levenberg-style damping: a step is accepted only when it lowers the
//!   squared residual; `lambda` rises on rejection and falls on acceptance.
//! - A rank report from row-echelon reduction gives the mechanism's free DOF
//!   (`unknowns - rank(J)`), the DMU-like mechanism-analysis number.

/// Tuning parameters for the Gauss-Newton solver.
#[derive(Debug, Clone)]
pub struct Solver {
    /// Maximum Gauss-Newton iterations per call.
    pub max_iterations: usize,
    /// Convergence threshold on the max-abs residual (metric units).
    pub tol: f64,
    /// Initial Levenberg damping.
    pub lambda0: f64,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            max_iterations: 300,
            tol: 1e-10,
            lambda0: 1e-4,
        }
    }
}

impl Solver {
    /// A looser solver useful for coarse warm-start stages.
    pub fn coarse() -> Self {
        Self {
            max_iterations: 80,
            tol: 1e-6,
            lambda0: 1e-2,
        }
    }
}

/// Outcome of a solve call.
#[derive(Debug, Clone, PartialEq)]
pub struct SolveReport {
    /// Iterations taken.
    pub iterations: usize,
    /// Final 2-norm of the residual vector.
    pub residual_norm: f64,
    /// Final max-abs residual component.
    pub max_residual: f64,
    /// Free DOF of the solved state: `unknowns - rank(J)`.
    pub free_dof: usize,
    /// True when a max-abs residual below [`Solver::tol`] was reached.
    pub converged: bool,
}

/// Solve `min ||F(x)||` from the warm start `x0`. Returns the converged state
/// and a [`SolveReport`].
pub fn solve_least_squares<F>(
    residual: &mut F,
    x0: &[f64],
    solver: &Solver,
) -> (Vec<f64>, SolveReport)
where
    F: FnMut(&[f64]) -> Vec<f64>,
{
    let n = x0.len();
    let mut x = x0.to_vec();
    let mut lambda = solver.lambda0;

    let mut f = residual(&x);
    let mut f_norm = norm2(&f);
    let mut report = SolveReport {
        iterations: 0,
        residual_norm: f_norm,
        max_residual: max_abs(&f),
        free_dof: 0,
        converged: false,
    };
    if report.max_residual < solver.tol {
        report.converged = true;
        return (x, report);
    }

    for _iter in 0..solver.max_iterations {
        let jac = fd_jacobian(&mut *residual, &x);
        let m = f.len();
        report.free_dof = n.saturating_sub(matrix_rank(&jac, m, n, None));

        let mut a = vec![0.0_f64; n * n];
        let mut g = vec![0.0_f64; n];
        mul_ata(&jac, &f, &mut a, &mut g, m, n);
        for i in 0..n {
            a[i * n + i] += lambda * a[i * n + i].max(1e-12);
        }
        let rhs: Vec<f64> = g.iter().map(|v| -v).collect();
        let dx = match gaussian_solve(&a, &rhs, n) {
            Some(d) => d,
            None => {
                lambda *= 10.0;
                continue;
            }
        };

        let mut xt = x.clone();
        for (i, d) in dx.iter().enumerate() {
            xt[i] += d;
        }
        let ft = residual(&xt);
        let ft_norm = norm2(&ft);
        if ft_norm.is_finite() && ft_norm < f_norm {
            x = xt;
            f = ft;
            f_norm = ft_norm;
            lambda = (lambda * 0.3).max(1e-12);
            report.iterations = _iter + 1;
            report.residual_norm = f_norm;
            report.max_residual = max_abs(&f);
            if report.max_residual < solver.tol {
                report.converged = true;
                break;
            }
        } else {
            lambda *= 4.0;
            if lambda > 1e12 {
                break;
            }
        }
    }

    report.max_residual = max_abs(&f);
    report.residual_norm = norm2(&f);
    (x, report)
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|x| x * x).sum::<f64>().sqrt()
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |m, x| m.max(x.abs()))
}

/// Central finite-difference Jacobian `m x n` stored row-major (`m` residuals,
/// `n` unknowns).
fn fd_jacobian<F>(residual: &mut F, x: &[f64]) -> Vec<f64>
where
    F: FnMut(&[f64]) -> Vec<f64>,
{
    let n = x.len();
    let f0 = residual(x);
    let m = f0.len();
    let mut jac = vec![0.0_f64; m * n];
    for j in 0..n {
        let h = 1e-7_f64 * (1.0 + x[j].abs());
        let mut xp = x.to_vec();
        let mut xm = x.to_vec();
        xp[j] += h;
        xm[j] -= h;
        let fp = residual(&xp);
        let fm = residual(&xm);
        for i in 0..m {
            jac[i * n + j] = (fp[i] - fm[i]) / (2.0 * h);
        }
    }
    jac
}

/// `A = J^T J`, `g = J^T f` for an `m x n` Jacobian (`J` row-major).
fn mul_ata(jac: &[f64], f: &[f64], a: &mut [f64], g: &mut [f64], m: usize, n: usize) {
    for r in 0..n {
        for c in 0..n {
            let mut s = 0.0;
            for k in 0..m {
                s += jac[k * n + r] * jac[k * n + c];
            }
            a[r * n + c] = s;
        }
    }
    for r in 0..n {
        let mut s = 0.0;
        for k in 0..m {
            s += jac[k * n + r] * f[k];
        }
        g[r] = s;
    }
}

/// Solve `A x = b` (`A` row-major `n x n`) by Gaussian elimination with
/// partial pivoting. Returns `None` when the matrix is numerically singular.
fn gaussian_solve(a: &[f64], b: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut m = a.to_vec();
    let mut rhs = b.to_vec();
    for col in 0..n {
        let mut p = col;
        let mut best = m[col * n + col].abs();
        for row in (col + 1)..n {
            let v = m[row * n + col].abs();
            if v > best {
                best = v;
                p = row;
            }
        }
        if best < 1e-14 {
            return None;
        }
        if p != col {
            for c in 0..n {
                m.swap(col * n + c, p * n + c);
            }
            rhs.swap(col, p);
        }
        let pivot = m[col * n + col];
        for row in (col + 1)..n {
            let factor = m[row * n + col] / pivot;
            if factor == 0.0 {
                continue;
            }
            for c in col..n {
                m[row * n + c] -= factor * m[col * n + c];
            }
            rhs[row] -= factor * rhs[col];
        }
    }
    let mut x = vec![0.0_f64; n];
    for row in (0..n).rev() {
        let mut s = rhs[row];
        for c in (row + 1)..n {
            s -= m[row * n + c] * x[c];
        }
        x[row] = s / m[row * n + row];
    }
    Some(x)
}

/// Rank of a row-major `m x n` matrix by row-echelon reduction with a
/// relative tolerance (`eps` defaults to `1e-9 * max|entry|`).
pub fn matrix_rank(mat: &[f64], m: usize, n: usize, eps: Option<f64>) -> usize {
    let mut w = mat.to_vec();
    let max_entry = mat.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
    let tol = eps.unwrap_or(1e-9 * max_entry.max(1.0));
    let mut rank = 0;
    let mut row = 0;
    for col in 0..n {
        if row >= m {
            break;
        }
        let mut p = row;
        let mut best = w[row * n + col].abs();
        for r in (row + 1)..m {
            let v = w[r * n + col].abs();
            if v > best {
                best = v;
                p = r;
            }
        }
        if best <= tol {
            continue;
        }
        for c in 0..n {
            w.swap(row * n + c, p * n + c);
        }
        let pivot = w[row * n + col];
        for r in (row + 1)..m {
            let factor = w[r * n + col] / pivot;
            if factor == 0.0 {
                continue;
            }
            for c in col..n {
                w[r * n + c] -= factor * w[row * n + c];
            }
        }
        rank += 1;
        row += 1;
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_solve_simple() {
        // 2x2: 2x + y = 4 ; x + 3y = 7  => (x=1, y=2)
        let a = vec![2.0, 1.0, 1.0, 3.0];
        let b = vec![4.0, 7.0];
        let x = gaussian_solve(&a, &b, 2).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12);
        assert!((x[1] - 2.0).abs() < 1e-12);
    }

    #[test]
    fn ls_solves_intersection() {
        // Two lines through the origin: y = x and y = -x + 2 meet at (1,1).
        // Solve min || (y - x, y + x - 2) ||^2 with unknown (x, y).
        let mut f = |st: &[f64]| {
            let (x, y) = (st[0], st[1]);
            vec![y - x, y + x - 2.0]
        };
        let (x, rep) = solve_least_squares(&mut f, &[0.0, 0.0], &Solver::default());
        assert!(rep.converged, "report: {rep:?}");
        assert!((x[0] - 1.0).abs() < 1e-8, "x={x:?}");
        assert!((x[1] - 1.0).abs() < 1e-8, "x={x:?}");
    }

    #[test]
    fn rank_free_dof() {
        // 2x3 matrix of full rank 2 => free DOF 1.
        let jac = vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let r = matrix_rank(&jac, 2, 3, None);
        assert_eq!(r, 2);
    }
}
