//! [`Mechanism`]: a collection of movable instances, fixed base frames and
//! DMU-like mates solved as a constrained assembly.
//!
//! The mechanism operates over the `oep-geo` model: each movable body maps to
//! an [`AssemblyInstance`](oep_geo::Instance) id whose placement [`Transform`]
//! seeds the initial [`Pose`]; fixed base frames (e.g. the real bracket model
//! instanced in the same assembly) keep their `oep-geo` transforms untouched.
//! Mates reference body ids by name, so the whole mechanism is expressed over
//! real `oep-geo` documents.
//!
//! Solvers take the form `solve_with(&overrides)`: a driven body (the crank in
//! the demo) is excluded from the free unknowns and instead pinned at the
//! commanded pose, which is how a kinematic *command* drives the mechanism
//! (DMU-style: a command fixes one joint DOF in time; the rest follow).

use std::collections::BTreeMap;

use oep_geo::math::Vec3;

use crate::mate::{Mate, PoseMap};
use crate::pose::{mat3_to_rotvec, rotvec_to_mat3, Pose};
use crate::solver::{self, SolveReport, Solver};

/// A movable instance + its current solved pose.
#[derive(Debug, Clone)]
pub struct BodyState {
    /// The `oep-geo` instance id this body represents.
    pub id: String,
    /// Current pose (world frame).
    pub pose: Pose,
}

/// A constrained assembly mechanism over `oep-geo` instance ids.
#[derive(Debug, Clone)]
pub struct Mechanism {
    /// Movable bodies, in solver-unknown order (6 unknowns each).
    pub movables: Vec<String>,
    /// Fixed base frames (e.g. real parts instanced from an `oep-geo` doc),
    /// keyed by instance id -> world pose.
    pub fixed: BTreeMap<String, Pose>,
    /// The DMU-like mates.
    pub mates: Vec<Mate>,
    /// Current pose of every movable body (index-parallel to `movables`).
    pub state: BTreeMap<String, Pose>,
}

impl Mechanism {
    /// An empty mechanism.
    pub fn new() -> Self {
        Self {
            movables: Vec::new(),
            fixed: BTreeMap::new(),
            mates: Vec::new(),
            state: BTreeMap::new(),
        }
    }

    /// Add a movable body seeded at `pose`.
    pub fn add_movable(&mut self, id: impl Into<String>, pose: Pose) {
        let id = id.into();
        self.movables.push(id.clone());
        self.state.insert(id, pose);
    }

    /// Add a fixed base frame (not solved).
    pub fn add_fixed(&mut self, id: impl Into<String>, pose: Pose) {
        self.fixed.insert(id.into(), pose);
    }

    /// Add a mate.
    pub fn add_mate(&mut self, mate: Mate) {
        self.mates.push(mate);
    }

    /// The pose of a frame: movable state, fixed base, or ground identity.
    pub fn pose_of(&self, id: &str) -> Pose {
        if id == crate::GROUND {
            Pose::identity()
        } else if let Some(p) = self.state.get(id) {
            *p
        } else if let Some(p) = self.fixed.get(id) {
            *p
        } else {
            panic!("mechanism references unknown frame {id:?}");
        }
    }

    fn pose_map(&self) -> PoseMap {
        let mut m = PoseMap::new();
        for (k, p) in &self.state {
            m.insert(k.clone(), *p);
        }
        for (k, p) in &self.fixed {
            m.insert(k.clone(), *p);
        }
        m
    }

    /// Concatenated residual over all mates for the current `overrides`
    /// applied on top of `self.state`.
    ///
    /// `x` packs the rotation-vector + position unknowns of the **free**
    /// movable bodies (`free_ids`, excluding overridden/driven bodies).
    fn residual(
        &self,
        free_ids: &[String],
        overrides: &BTreeMap<String, Pose>,
        x: &[f64],
    ) -> Vec<f64> {
        let mut poses = self.pose_map();
        for (i, id) in free_ids.iter().enumerate() {
            let pos = Vec3::new(x[i * 6], x[i * 6 + 1], x[i * 6 + 2]);
            let rot = rotvec_to_mat3(&Vec3::new(x[i * 6 + 3], x[i * 6 + 4], x[i * 6 + 5]));
            poses.insert(
                id.clone(),
                Pose {
                    position: pos,
                    orientation: rot,
                },
            );
        }
        for (id, p) in overrides {
            poses.insert(id.clone(), *p);
        }
        let mut out = Vec::new();
        for mate in &self.mates {
            mate.push_residual(&poses, &mut out);
        }
        out
    }

    /// Solve the mechanism with `overrides` pinning driven bodies. Free bodies
    /// are all movables minus the override keys (the driven set). The solve
    /// warm-starts from the current `self.state` of those bodies.
    pub fn solve_with(
        &mut self,
        overrides: &BTreeMap<String, Pose>,
        solver: &Solver,
    ) -> Result<SolveReport, String> {
        let free: Vec<String> = self
            .movables
            .iter()
            .filter(|id| !overrides.contains_key(*id))
            .cloned()
            .collect();
        let mut x0 = Vec::with_capacity(free.len() * 6);
        for id in &free {
            let p = self
                .state
                .get(id)
                .ok_or_else(|| format!("missing state for {id}"))?;
            x0.push(p.position.x);
            x0.push(p.position.y);
            x0.push(p.position.z);
            let rv = mat3_to_rotvec(&p.orientation);
            x0.push(rv.x);
            x0.push(rv.y);
            x0.push(rv.z);
        }
        let capture = self.clone();
        let mut residual_closure = |x: &[f64]| capture.residual(&free, overrides, x);
        let (x, report) = solver::solve_least_squares(&mut residual_closure, &x0, solver);
        // Commit the solved state.
        for (i, id) in free.iter().enumerate() {
            let pos = Vec3::new(x[i * 6], x[i * 6 + 1], x[i * 6 + 2]);
            let rot = rotvec_to_mat3(&Vec3::new(x[i * 6 + 3], x[i * 6 + 4], x[i * 6 + 5]));
            let entry = self.state.entry(id.clone()).or_insert_with(Pose::identity);
            entry.position = pos;
            entry.orientation = rot;
        }
        for (id, p) in overrides {
            let entry = self.state.entry(id.clone()).or_insert_with(Pose::identity);
            *entry = *p;
        }
        Ok(report)
    }

    /// The max-abs residual of the current committed state with `overrides`
    /// (post-solve verification, independent of the solver internals).
    pub fn verify_residual(&self, overrides: &BTreeMap<String, Pose>) -> f64 {
        let free: Vec<String> = self
            .movables
            .iter()
            .filter(|id| !overrides.contains_key(*id))
            .cloned()
            .collect();
        let mut x = Vec::with_capacity(free.len() * 6);
        for id in &free {
            let p = self.state[id];
            x.push(p.position.x);
            x.push(p.position.y);
            x.push(p.position.z);
            let rv = mat3_to_rotvec(&p.orientation);
            x.push(rv.x);
            x.push(rv.y);
            x.push(rv.z);
        }
        self.residual(&free, overrides, &x)
            .into_iter()
            .fold(0.0_f64, |m, v| m.max(v.abs()))
    }

    /// Total unknowns if nothing were driven (all movables free).
    pub fn unknowns(&self) -> usize {
        self.movables.len() * 6
    }

    /// Total residuals contributed by all mates.
    pub fn residual_count(&self) -> usize {
        self.mates.iter().map(|m| m.residual_len()).sum()
    }
}

impl Default for Mechanism {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_mechanism_counts() {
        let m = Mechanism::new();
        assert_eq!(m.unknowns(), 0);
        assert_eq!(m.residual_count(), 0);
        assert_eq!(m.pose_of(crate::GROUND).position, Vec3::zero());
    }
}
