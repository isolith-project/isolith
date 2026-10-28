//! The crank-slider demo fixture for the P7-M3-T02 mates/kinematics recording.
//!
//! The fixture is a constrained assembly solved and animated with the own
//! DMU-like semantics: an **inline crank-slider** (crank + connecting rod +
//! piston) constrained to a real `oep-geo` base model borrowed from the
//! program corpus (the committed golden bracket document). One driven revolute
//! (the crank command) + two passive revolutes + one prismatic groove yield a
//! 1-DOF mechanism whose motion has a closed-form analytic solution, which the
//! tests verify the solver against frame by frame.
//!
//! Geometry (mm, motion plane z = 8): pivot O = (50, 50, 8), crank length
//! R = 30, rod length L = 70. Piston slides along +X on the groove y = 50.

use std::collections::BTreeMap;

use oep_geo::math::{Mat3, Vec3};
use oep_geo::{Assembly, Body, BodyKind, BoundingBox, Document, Instance, InstanceSource, Part};

use crate::mate::{Mate, Prismatic, Revolute};
use crate::mechanism::Mechanism;
use crate::pose::Pose;
use crate::raster::{Camera, Mesh, SceneItem};
use crate::solver::Solver;
use crate::GROUND;

/// Real corpus material: the committed golden `oep-geo` model, byte-locked to
/// `crates/oep-geo/tests/golden/example-bracket.oepgeo.json` (a test re-checks
/// the two copies stay identical).
pub const GOLDEN_OEPGEO: &str = include_str!("../tests/golden/example-bracket.oepgeo.json");

/// Crank (input arm) length in mm.
pub const CRANK_R: f64 = 30.0;
/// Connecting rod length in mm.
pub const ROD_L: f64 = 70.0;
/// Fixed pivot of the crank (mm).
pub const PIVOT: [f64; 3] = [50.0, 50.0, 8.0];

/// Movable instance ids in the demo assembly.
pub const MOVABLES: [&str; 3] = ["inst-crank", "inst-rod", "inst-piston"];
/// Fixed instance ids imported from the real golden model + the fixture plate.
/// (Distinct from the golden's own instance ids: one document = one id space.)
pub const FIXED: [&str; 3] = [
    "demo-inst-bracket",
    "demo-inst-fastener-set",
    "inst-crank-base",
];

/// Build the combined demo `oep-geo` document: the real golden bracket model
/// plus the mechanism parts, all instanced into one top assembly.
pub fn build_demo_document() -> Document {
    let mut doc = Document::from_json_pretty(GOLDEN_OEPGEO).expect("golden corpus parses");
    // Fixture parts (bbox-only bodies; display is derived in the renderer).
    let base_body = Body::new("oep-body-crank-base", "base-plate", BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [160.0, 80.0, 8.0]));
    doc.push_body(base_body);
    doc.push_part(
        Part::new("oep-part-crank-base", "crank-base")
            .with_part_number("ISO-CB-000")
            .with_body("oep-body-crank-base"),
    );

    let crank_body = Body::new("oep-body-crank", "crank-arm", BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [CRANK_R, 8.0, 4.0]));
    doc.push_body(crank_body);
    doc.push_part(
        Part::new("oep-part-crank", "crank")
            .with_part_number("ISO-CR-001")
            .with_body("oep-body-crank"),
    );

    let rod_body = Body::new("oep-body-rod", "connecting-rod", BodyKind::Solid)
        .with_bbox(BoundingBox::new([0.0, 0.0, 0.0], [ROD_L, 6.0, 4.0]));
    doc.push_body(rod_body);
    doc.push_part(
        Part::new("oep-part-rod", "rod")
            .with_part_number("ISO-RD-001")
            .with_body("oep-body-rod"),
    );

    let piston_body = Body::new("oep-body-piston", "piston", BodyKind::Solid)
        .with_bbox(BoundingBox::new([-8.0, -7.0, 4.0], [8.0, 7.0, 12.0]));
    doc.push_body(piston_body);
    doc.push_part(
        Part::new("oep-part-piston", "piston")
            .with_part_number("ISO-PS-001")
            .with_body("oep-body-piston"),
    );

    // Top assembly: the real bracket + fixture plate fixed, mechanism movable.
    let pin0 = [PIVOT[0] + CRANK_R, PIVOT[1], PIVOT[2]];
    let piston0 = [PIVOT[0] + CRANK_R + ROD_L, PIVOT[1], PIVOT[2]];
    let assembly = Assembly::new("oep-assembly-demo", "crank-slider-fixture")
        .with_instance(Instance::new(
            "demo-inst-bracket",
            "bracket",
            InstanceSource::Part {
                id: "oep-part-bracket".into(),
            },
        ))
        .with_instance(Instance::new(
            "demo-inst-fastener-set",
            "fastener-set",
            InstanceSource::Assembly {
                id: "oep-assembly-fastener-set".into(),
            },
        ))
        .with_instance(Instance::new(
            "inst-crank-base",
            "crank-base",
            InstanceSource::Part {
                id: "oep-part-crank-base".into(),
            },
        ))
        .with_instance(
            Instance::new(
                "inst-crank",
                "crank",
                InstanceSource::Part {
                    id: "oep-part-crank".into(),
                },
            )
            .with_transform(oep_geo::Transform::translation(
                PIVOT[0], PIVOT[1], PIVOT[2],
            )),
        )
        .with_instance(
            Instance::new(
                "inst-rod",
                "rod",
                InstanceSource::Part {
                    id: "oep-part-rod".into(),
                },
            )
            .with_transform(oep_geo::Transform::translation(pin0[0], pin0[1], pin0[2])),
        )
        .with_instance(
            Instance::new(
                "inst-piston",
                "piston",
                InstanceSource::Part {
                    id: "oep-part-piston".into(),
                },
            )
            .with_transform(oep_geo::Transform::translation(
                piston0[0], piston0[1], piston0[2],
            )),
        );
    doc.push_assembly(assembly);

    // Workspace the full assembly including the golden sub-assembly.
    let problems = doc.validate();
    assert!(
        problems.is_empty(),
        "demo document must validate; got {problems:?}"
    );
    doc
}

/// Build the solved [`Mechanism`] for the demo document (seeded at theta = 0,
/// pivots/mates as documented at module level).
pub fn build_demo_mechanism() -> Mechanism {
    let doc = build_demo_document();
    let assembly = doc
        .assemblies
        .iter()
        .find(|a| a.id == "oep-assembly-demo")
        .expect("demo assembly present");
    let mut mech = Mechanism::new();
    for inst in &assembly.instances {
        let pose = Pose::from_transform(&inst.transform);
        if MOVABLES.contains(&inst.id.as_str()) {
            mech.add_movable(inst.id.clone(), pose);
        } else if FIXED.contains(&inst.id.as_str()) {
            mech.add_fixed(inst.id.clone(), pose);
        } else {
            panic!("unexpected instance {}", inst.id);
        }
    }

    mech.add_mate(Mate::Revolute(Revolute::new(
        "inst-crank",
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        GROUND,
        Vec3::new(PIVOT[0], PIVOT[1], PIVOT[2]),
        Vec3::new(0.0, 0.0, 1.0),
    )));
    mech.add_mate(Mate::Revolute(Revolute::new(
        "inst-rod",
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        "inst-crank",
        Vec3::new(CRANK_R, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    )));
    mech.add_mate(Mate::Revolute(Revolute::new(
        "inst-piston",
        Vec3::zero(),
        Vec3::new(0.0, 0.0, 1.0),
        "inst-rod",
        Vec3::new(ROD_L, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    )));
    mech.add_mate(Mate::Prismatic(Prismatic::new(
        "inst-piston",
        GROUND,
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, PIVOT[1], PIVOT[2]),
    )));
    mech
}

/// The commanded crank pose for a target crank angle `theta` (radians).
pub fn crank_command(theta: f64) -> Pose {
    let rot = Mat3::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), theta).expect("valid axis");
    Pose {
        position: Vec3::new(PIVOT[0], PIVOT[1], PIVOT[2]),
        orientation: rot,
    }
}

/// Analytic (closed-form) solution of the inline crank-slider for `theta`:
/// the ground truth the solver is verified against. Returns poses keyed by
/// instance id (crank, rod, piston).
pub fn analytic_solution(theta: f64) -> BTreeMap<String, Pose> {
    let (ct, st) = (theta.cos(), theta.sin());
    let pin = Vec3::new(PIVOT[0] + CRANK_R * ct, PIVOT[1] + CRANK_R * st, PIVOT[2]);
    let disc = (ROD_L * ROD_L - (CRANK_R * st) * (CRANK_R * st)).sqrt();
    let piston_x = PIVOT[0] + CRANK_R * ct + disc;

    // rod direction from crank pin toward piston pin
    let dx = disc / ROD_L;
    let dy = -CRANK_R * st / ROD_L;
    let rod_angle = dy.atan2(dx);
    let mut map = BTreeMap::new();
    map.insert("inst-crank".to_string(), crank_command(theta));
    map.insert(
        "inst-rod".to_string(),
        Pose {
            position: pin,
            orientation: Mat3::from_axis_angle(Vec3::new(0.0, 0.0, 1.0), rod_angle)
                .expect("valid axis"),
        },
    );
    map.insert(
        "inst-piston".to_string(),
        Pose {
            position: Vec3::new(piston_x, PIVOT[1], PIVOT[2]),
            orientation: Mat3::identity(),
        },
    );
    map
}

/// A single recorded frame.
#[derive(Debug, Clone)]
pub struct FrameRecord {
    /// Crank command angle (radians).
    pub theta: f64,
    /// Max-abs joint residual of the solved state (independent verify).
    pub residual_max: f64,
    /// Max translation error vs the analytic solution over movable bodies (mm).
    pub error_translation: f64,
    /// Max orientation error vs the analytic solution over movable bodies (rad).
    pub error_orientation: f64,
    /// Solved poses keyed by instance id (movables overwritten by the solve).
    pub poses: BTreeMap<String, Pose>,
}

/// Result of a full demo run.
#[derive(Debug, Clone)]
pub struct DemoResult {
    /// The mechanism as solved after the run (last frame).
    pub mechanism: Mechanism,
    /// Per-frame records.
    pub frames: Vec<FrameRecord>,
    /// Worst residual over all frames.
    pub worst_residual: f64,
    /// Worst translation error over all frames.
    pub worst_translation_error: f64,
    /// Worst orientation error over all frames.
    pub worst_orientation_error: f64,
    /// Free DOF of the mechanism when the crank is NOT driven (undriven solve).
    pub free_dof_undriven: usize,
}

/// Solve one frame: command the crank to `theta`, solve the rest, verify the
/// residual independently, and return (solved poses asserted against the
/// analytic solution, residual, errors).
pub fn solve_frame(
    mech: &mut Mechanism,
    theta: f64,
    solver: &Solver,
) -> (BTreeMap<String, Pose>, f64, f64, f64) {
    let mut cmd = BTreeMap::new();
    cmd.insert("inst-crank".to_string(), crank_command(theta));
    let report = mech
        .solve_with(&cmd, solver)
        .expect("solve must not fail for a well-posed fixture");
    assert!(
        report.converged,
        "solver did not converge at theta={theta}: {report:?}"
    );
    let residual = mech.verify_residual(&cmd);
    let analytic = analytic_solution(theta);
    let (mut t_err, mut o_err) = (0.0_f64, 0.0_f64);
    for (id, apos) in &analytic {
        let spos = mech.state.get(id).expect("movable state present");
        t_err = t_err.max(spos.position.distance_to(&apos.position));
        let orient = crate::pose::orientation_residual(&spos.orientation, &apos.orientation);
        o_err = o_err.max(orient.norm());
    }
    (mech.state.clone(), residual, t_err, o_err)
}

/// Run `frames` crank revolutions-worth of steps and record everything.
pub fn run_demo(frames: usize) -> DemoResult {
    // Mechanism analysis: free DOF with the crank NOT commanded (the classic
    // DMU "mechanism" question: how many independent inputs does it take?).
    // The probe starts at a generic crank angle (the theta=0 pose is a
    // collinear dead-center configuration where the Jacobian is singular).
    let mut probe = build_demo_mechanism();
    probe
        .state
        .insert("inst-crank".to_string(), crank_command(1.0));
    let undriven = probe
        .solve_with(&BTreeMap::new(), &Solver::coarse())
        .expect("undriven solve ok");
    let free_dof_undriven = undriven.free_dof;

    // Fresh mechanism, seeded at the documented assembled pose, for the driven
    // animation (so the recorded frames start from the exact theta = 0 pose).
    let mut mech = build_demo_mechanism();
    let solver = Solver::default();
    let mut records = Vec::with_capacity(frames);
    let mut worst_r = 0.0_f64;
    let mut worst_te = 0.0_f64;
    let mut worst_oe = 0.0_f64;
    for i in 0..frames {
        let theta = 2.0 * std::f64::consts::PI * i as f64 / frames as f64;
        let (poses, residual, te, oe) = solve_frame(&mut mech, theta, &solver);
        worst_r = worst_r.max(residual);
        worst_te = worst_te.max(te);
        worst_oe = worst_oe.max(oe);
        records.push(FrameRecord {
            theta,
            residual_max: residual,
            error_translation: te,
            error_orientation: oe,
            poses,
        });
    }
    DemoResult {
        mechanism: mech,
        frames: records,
        worst_residual: worst_r,
        worst_translation_error: worst_te,
        worst_orientation_error: worst_oe,
        free_dof_undriven,
    }
}

fn fmt_f64(v: f64) -> String {
    format!("{v:.12e}")
}

/// Write the recorded demo trace (`trace.json`) with std only (deterministic).
pub fn write_trace(result: &DemoResult, frames: &[FrameRecord], path: &str) -> std::io::Result<()> {
    let mut body = String::with_capacity(64 * 1024);
    body.push_str("{\n");
    body.push_str("  \"program\": \"P7-M3-T02\",\n");
    body.push_str("  \"fixture\": \"crank-slider\",\n");
    body.push_str("  \"units\": \"mm\",\n");
    body.push_str(&format!(
        "  \"params\": {{\"crank_r\": {}, \"rod_l\": {}, \"pivot\": [{}, {}, {}]}},\n",
        fmt_f64(CRANK_R),
        fmt_f64(ROD_L),
        fmt_f64(PIVOT[0]),
        fmt_f64(PIVOT[1]),
        fmt_f64(PIVOT[2])
    ));
    body.push_str(&format!(
        "  \"worst_residual_max\": {},\n",
        fmt_f64(result.worst_residual)
    ));
    body.push_str(&format!(
        "  \"worst_translation_error\": {},\n",
        fmt_f64(result.worst_translation_error)
    ));
    body.push_str(&format!(
        "  \"worst_orientation_error\": {},\n",
        fmt_f64(result.worst_orientation_error)
    ));
    body.push_str(&format!(
        "  \"free_dof_undriven\": {},\n",
        result.free_dof_undriven
    ));
    body.push_str("  \"frames\": [\n");
    for (i, fr) in frames.iter().enumerate() {
        body.push_str("    {\n");
        body.push_str(&format!("      \"frame\": {},\n", i));
        body.push_str(&format!("      \"theta\": {},\n", fmt_f64(fr.theta)));
        body.push_str(&format!(
            "      \"residual_max\": {},\n",
            fmt_f64(fr.residual_max)
        ));
        body.push_str(&format!(
            "      \"error_translation\": {},\n",
            fmt_f64(fr.error_translation)
        ));
        body.push_str(&format!(
            "      \"error_orientation\": {},\n",
            fmt_f64(fr.error_orientation)
        ));
        body.push_str("      \"bodies\": {\n");
        for (k, (id, pose)) in fr.poses.iter().enumerate() {
            let t = pose.to_transform();
            let c = t.components();
            let arr = c.iter().map(|v| fmt_f64(*v)).collect::<Vec<_>>().join(", ");
            body.push_str(&format!("        \"{id}\": [{}]", arr));
            body.push_str(if k + 1 == fr.poses.len() { "\n" } else { ",\n" });
        }
        body.push_str("      }\n");
        body.push_str(if i + 1 == frames.len() {
            "    }\n"
        } else {
            "    },\n"
        });
    }
    body.push_str("  ]\n");
    body.push_str("}\n");
    std::fs::write(path, body)
}

/// Build the render scene for one frame from the solved poses (visual only).
pub fn build_scene(mech: &Mechanism, poses: &BTreeMap<String, Pose>) -> Vec<SceneItem> {
    const SLATE: [u8; 3] = [150, 158, 166];
    const RAIL: [u8; 3] = [118, 128, 138];
    const BRACKET: [u8; 3] = [168, 150, 96];
    const FASTENER: [u8; 3] = [208, 208, 214];
    const CRANKC: [u8; 3] = [34, 96, 214];
    const RODC: [u8; 3] = [54, 160, 74];
    const PISTONC: [u8; 3] = [217, 79, 48];
    let mut scene: Vec<SceneItem> = Vec::new();

    let plate = Mesh::box_mesh([0.0, 0.0, 0.0], [160.0, 80.0, 8.0]);
    scene.push(SceneItem {
        mesh: plate,
        color: SLATE,
    });
    // slider guide rails flanking the piston path y in [43, 57]
    for (y0, y1) in [(40.0, 43.0), (57.0, 60.0)] {
        let rail = Mesh::box_mesh([0.0, y0, 8.0], [162.0, y1, 10.0]);
        scene.push(SceneItem {
            mesh: rail,
            color: RAIL,
        });
    }

    for id in FIXED {
        let pose = *mech.fixed.get(id).unwrap_or(&Pose::identity());
        let color = match id {
            "demo-inst-bracket" => BRACKET,
            "demo-inst-fastener-set" => FASTENER,
            _ => SLATE,
        };
        let bbox = match id {
            "demo-inst-bracket" => [0.0, 0.0, 0.0, 80.0, 40.0, 6.0],
            "inst-crank-base" => [0.0, 0.0, 0.0, 160.0, 80.0, 8.0],
            _ => [0.0, 0.0, 0.0, 10.0, 10.0, 20.0],
        };
        let m = Mesh::box_mesh([bbox[0], bbox[1], bbox[2]], [bbox[3], bbox[4], bbox[5]]);
        scene.push(SceneItem {
            mesh: m.transformed(
                [pose.position.x, pose.position.y, pose.position.z],
                &pose_to_rot9(&pose),
            ),
            color,
        });
    }

    // movables
    let crank = poses.get("inst-crank").expect("crank pose");
    let rod = poses.get("inst-rod").expect("rod pose");
    let piston = poses.get("inst-piston").expect("piston pose");
    let crank_arm = Mesh::box_mesh([0.0, -5.0, 0.0], [CRANK_R, 5.0, 4.0]);
    let hub = Mesh::prism_cylinder(9.0, 8.0, 20);
    scene.push(SceneItem {
        mesh: crank_arm.transformed(
            [crank.position.x, crank.position.y, crank.position.z],
            &pose_to_rot9(crank),
        ),
        color: CRANKC,
    });
    scene.push(SceneItem {
        mesh: hub.transformed(
            [crank.position.x, crank.position.y, crank.position.z],
            &pose_to_rot9(crank),
        ),
        color: CRANKC,
    });
    let rodm = Mesh::box_mesh([0.0, -3.0, 0.0], [ROD_L, 3.0, 4.0]);
    scene.push(SceneItem {
        mesh: rodm.transformed(
            [rod.position.x, rod.position.y, rod.position.z],
            &pose_to_rot9(rod),
        ),
        color: RODC,
    });
    let pism = Mesh::box_mesh([-8.0, -7.0, 4.0], [8.0, 7.0, 12.0]);
    scene.push(SceneItem {
        mesh: pism.transformed(
            [piston.position.x, piston.position.y, piston.position.z],
            &pose_to_rot9(piston),
        ),
        color: PISTONC,
    });
    scene
}

/// Render the demo frames to PPM and return the list of written files.
pub fn render_frames(
    mech: &Mechanism,
    frames: &[FrameRecord],
    out_frames: &str,
) -> std::io::Result<Vec<String>> {
    std::fs::create_dir_all(out_frames)?;
    let cam = Camera {
        eye: [140.0, 170.0, 210.0],
        target: [80.0, 45.0, 4.0],
        up: [0.0, 1.0, 0.0],
        fov_deg: 38.0,
        width: 640,
        height: 480,
    };
    let mut paths = Vec::with_capacity(frames.len());
    for (i, fr) in frames.iter().enumerate() {
        let scene = build_scene(mech, &fr.poses);
        let p = format!("{out_frames}/frame_{i:04}.ppm");
        cam.render_ppm(&scene, &p)?;
        paths.push(p);
    }
    Ok(paths)
}

fn pose_to_rot9(p: &Pose) -> [f64; 9] {
    let o = p.orientation;
    [
        o.get(0, 0),
        o.get(1, 0),
        o.get(2, 0),
        o.get(0, 1),
        o.get(1, 1),
        o.get(2, 1),
        o.get(0, 2),
        o.get(1, 2),
        o.get(2, 2),
    ]
}

/// Convenience: the demo acceptance gate used by the binary and the pytest --
/// returns the worst numbers as a small struct to assert against.
pub struct DemoGates {
    /// Worst residual across frames (must be ~0).
    pub worst_residual: f64,
    /// Worst translation error vs analytic (mm).
    pub worst_translation_error: f64,
    /// Worst orientation error vs analytic (rad).
    pub worst_orientation_error: f64,
    /// Free DOF with the input not driven (must be 1 for this fixture).
    pub free_dof_undriven: usize,
}

impl DemoResult {
    /// Extract the gate numbers (for tests/pytest/manifest).
    pub fn gates(&self) -> DemoGates {
        DemoGates {
            worst_residual: self.worst_residual,
            worst_translation_error: self.worst_translation_error,
            worst_orientation_error: self.worst_orientation_error,
            free_dof_undriven: self.free_dof_undriven,
        }
    }
}

/// Placeholder re-export so the bin can print the profile report clearly.
pub fn summarize(result: &DemoResult) -> String {
    format!(
        "frames={} worst_residual={:.3e} worst_translation_err={:.3e} mm \
         worst_orientation_err={:.3e} rad free_dof_undriven={}",
        result.frames.len(),
        result.worst_residual,
        result.worst_translation_error,
        result.worst_orientation_error,
        result.free_dof_undriven
    )
}
