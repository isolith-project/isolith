//! Transform/instance hierarchy model with a matrix stack (P2-M4-T02).
//!
//! This is the format-independent core for the "assembly CGR inside part"
//! scenario: a CGR payload that carries N placed instances forming a
//! hierarchy, each instance holding a 4x4 local transform. The matrix stack
//! is the standard scene-graph device (OpenGL-style push/pop/multiply-right)
//! that accumulates parent→child transforms into world transforms during a
//! depth-first traversal.
//!
//! Everything here is deterministic and dependency-free (no mesh coupling):
//! an instance's `mesh` field is an opaque `Option<u32>` index so the crate
//! builds on `main` and merges cleanly with the not-yet-landed `mesh`
//! module. The clean-room scanner that anchors this model to real CGR bytes
//! lives in [`crate::assembly`]; that module only ever reports classified
//! outcomes (P2-M1-T04 tolerant doctrine) and never fabricates geometry.

use std::fmt;

/// Errors produced by [`Transform`] construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    /// A matrix entry is NaN or infinite; transforms must be finite.
    NonFinite,
}

impl fmt::Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransformError::NonFinite => write!(f, "transform row contains a non-finite value"),
        }
    }
}

impl std::error::Error for TransformError {}

/// A row-major 4x4 matrix of `f32`. The 4th row is conventionally
/// `[0, 0, 0, 1]` for affine transforms; we do not enforce it at
/// construction (the mesh/geometry consumers may carry perspective), but
/// the scene-graph walks compose `[x, y, z, 1]` points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    rows: [[f32; 4]; 4],
}

impl Transform {
    /// The identity transform.
    pub fn identity() -> Self {
        Self {
            rows: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// Build a transform from its four rows. Every entry must be finite,
    /// otherwise [`TransformError::NonFinite`] is returned.
    pub fn from_rows(rows: [[f32; 4]; 4]) -> Result<Self, TransformError> {
        if rows.iter().flatten().any(|x| !x.is_finite()) {
            return Err(TransformError::NonFinite);
        }
        Ok(Self { rows })
    }

    /// A pure translation.
    pub fn from_translation(tx: f32, ty: f32, tz: f32) -> Self {
        Self {
            rows: [
                [1.0, 0.0, 0.0, tx],
                [0.0, 1.0, 0.0, ty],
                [0.0, 0.0, 1.0, tz],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// A rotation about the Z axis by `angle` radians (right-handed,
    /// matching a +X→+Y positive rotation). Deterministic.
    pub fn from_rotation_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self {
            rows: [
                [c, -s, 0.0, 0.0],
                [s, c, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    /// The four rows.
    pub fn rows(&self) -> &[[f32; 4]; 4] {
        &self.rows
    }

    /// Matrix composition: `self * rhs` (apply `rhs` first, then `self`).
    /// Guards against non-finite results by clamping nothing — callers are
    /// expected to keep inputs finite; the multiplication itself cannot
    /// panic.
    pub fn mul(&self, rhs: &Self) -> Self {
        let a = &self.rows;
        let b = &rhs.rows;
        let mut out = [[0.0f32; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        Self { rows: out }
    }

    /// Transform a 3D point with implicit w=1.
    pub fn apply_point(&self, p: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = p;
        let r = self.rows;
        let x2 = r[0][0] * x + r[0][1] * y + r[0][2] * z + r[0][3];
        let y2 = r[1][0] * x + r[1][1] * y + r[1][2] * z + r[1][3];
        let z2 = r[2][0] * x + r[2][1] * y + r[2][2] * z + r[2][3];
        [x2, y2, z2]
    }

    /// True when the matrix is exactly the identity.
    pub fn is_identity(&self) -> bool {
        self == &Self::identity()
    }
}

/// Errors produced by [`MatrixStack`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixStackError {
    /// `pop` was called while the stack holds only its base frame.
    Empty,
}

impl fmt::Display for MatrixStackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MatrixStackError::Empty => write!(f, "matrix stack already at its base frame"),
        }
    }
}

impl std::error::Error for MatrixStackError {}

/// A matrix stack (OpenGL-style scene-graph device).
///
/// The stack always holds at least one frame (the "current" transform,
/// initially identity). `mult_right` composes a matrix onto the current
/// frame (`current = current * m` — the child transform is applied in the
/// parent's frame, which is how scene graphs accumulate). `push` saves the
/// current frame, `pop` restores the previous one; popping the last frame
/// is [`MatrixStackError::Empty`].
#[derive(Debug, Clone)]
pub struct MatrixStack {
    stack: Vec<Transform>,
}

impl Default for MatrixStack {
    fn default() -> Self {
        Self::new()
    }
}

impl MatrixStack {
    /// A new stack containing only the identity frame.
    pub fn new() -> Self {
        Self {
            stack: vec![Transform::identity()],
        }
    }

    /// Depth of the stack (frames held), always >= 1.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// The current (top) transform.
    pub fn current(&self) -> &Transform {
        self.stack.last().expect("stack never empty")
    }

    /// Compose `m` onto the current frame: `current = current * m`.
    pub fn mult_right(&mut self, m: &Transform) {
        let cur = *self.current();
        let next = cur.mul(m);
        *self.stack.last_mut().expect("stack never empty") = next;
    }

    /// Save the current frame (duplicate it on the stack).
    pub fn push(&mut self) {
        let cur = *self.current();
        self.stack.push(cur);
    }

    /// Restore the previous frame. Errors when already at the base frame.
    pub fn pop(&mut self) -> Result<(), MatrixStackError> {
        if self.stack.len() <= 1 {
            return Err(MatrixStackError::Empty);
        }
        self.stack.pop();
        Ok(())
    }
}

/// One placed instance in a scene graph.
///
/// Mirrors the CATIA/Automotive-native notion of an "instance": a named
/// node carrying a local transform relative to its parent, with its own
/// children and an optional reference to geometry. `mesh` is deliberately an
/// opaque index so this module does not couple to the (un-landed) mesh pool.
#[derive(Debug, Clone)]
pub struct Instance {
    id: u32,
    name: Option<String>,
    local: Transform,
    children: Vec<Instance>,
    mesh: Option<u32>,
}

impl Instance {
    /// A new instance with the given numeric id and optional name, identity
    /// local transform, no children and no mesh.
    pub fn new(id: u32, name: Option<String>) -> Self {
        Self {
            id,
            name,
            local: Transform::identity(),
            children: Vec::new(),
            mesh: None,
        }
    }

    /// The instance's numeric id.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// The instance's optional name.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// The local (parent-relative) transform.
    pub fn local(&self) -> &Transform {
        &self.local
    }

    /// Overwrite the local transform.
    pub fn set_local(&mut self, t: Transform) {
        self.local = t;
    }

    /// Direct children, in order.
    pub fn children(&self) -> &[Instance] {
        &self.children
    }

    /// Append a child instance.
    pub fn add_child(&mut self, child: Instance) {
        self.children.push(child);
    }

    /// The optional mesh reference (opaque index into a mesh pool).
    pub fn mesh(&self) -> Option<u32> {
        self.mesh
    }

    /// Set the mesh reference.
    pub fn set_mesh(&mut self, mesh: Option<u32>) {
        self.mesh = mesh;
    }
}

/// A result of one depth-first visit of a scene graph: the instance, the
/// child index path from the scene root, and the accumulated world
/// transform (parent world * instance local).
#[derive(Debug, Clone)]
pub struct WorldInstance<'a> {
    /// Child indices from the scene root (empty for a root instance).
    pub path: Vec<usize>,
    /// The instance.
    pub instance: &'a Instance,
    /// The world transform (accumulated through the matrix stack).
    pub world: Transform,
}

/// A scene graph: an ordered set of root instances forming a forest.
#[derive(Debug, Clone, Default)]
pub struct SceneGraph {
    roots: Vec<Instance>,
}

impl SceneGraph {
    /// An empty scene graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a root instance.
    pub fn add_root(&mut self, root: Instance) {
        self.roots.push(root);
    }

    /// The root instances.
    pub fn roots(&self) -> &[Instance] {
        &self.roots
    }

    /// Depth-first, pre-order visit of the whole forest, accumulating every
    /// instance's world transform with a [`MatrixStack`].
    ///
    /// For each instance: the current stack frame is multiplied right by the
    /// instance's local transform (giving its world transform), the visit is
    /// recorded, then the frame is pushed and children are walked before the
    /// frame is popped. This yields true parent→child composition (a child's
    /// world = parent's world * child's local), i.e. the matrix-stack
    /// semantics required by the assembly-CGR-inside-a-part card.
    pub fn world_instances(&self) -> Vec<WorldInstance<'_>> {
        let mut out = Vec::new();
        for root in &self.roots {
            // A fresh stack per root: sibling roots must not accumulate
            // transforms onto each other (each tree starts in world space).
            let mut stack = MatrixStack::new();
            let mut path = Vec::new();
            walk(root, &mut path, &mut stack, &mut out);
        }
        out
    }
}

fn walk<'a>(
    inst: &'a Instance,
    path: &mut Vec<usize>,
    stack: &mut MatrixStack,
    out: &mut Vec<WorldInstance<'a>>,
) {
    stack.mult_right(inst.local());
    let world = *stack.current();
    out.push(WorldInstance {
        path: path.clone(),
        instance: inst,
        world,
    });
    for (child_idx, child) in inst.children().iter().enumerate() {
        // Path is child-indices leading from the scene root to this node, so
        // a root visit carries an empty path and each child appends its own
        // sibling index before recursion and removes it after.
        path.push(child_idx);
        stack.push();
        walk(child, path, stack, out);
        stack
            .pop()
            .expect("frame pushed exactly once per child walk");
        path.pop();
    }
}
