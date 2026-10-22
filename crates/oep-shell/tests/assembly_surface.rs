//! EDITOR-ASSEM-EDIT acceptance: the assembly solver is interactive and part
//! of the editor surface.
//!
//! Contract locked here (each maps to the card body):
//!
//! 1. The surface opens a committed 3-part fixture with the oep-assem mate
//!    vocabulary (fixed, coincident, on-plane, plane-distance, parallel) and
//!    every create/edit/delete path re-solves live.
//! 2. A mate edit re-solves through the vendor-neutral seam and the instances
//!    follow immediately; the viewport (rendered PPM, content-hashed) changes.
//! 3. A diverging solve returns a typed error and never fabricates a pose:
//!    the document's placements are byte-identical to the last converged
//!    solve.
//! 4. Mechanism motion plays deterministically through the kinematics lane
//!    (P7-M3-T02 cradle): two recordings of the crank-slider turn produce
//!    byte-identical traces and physical numbers (60 mm piston stroke, one
//!    free DOF, machine-tight residuals).

use oep_assem::mates::Mate;
use oep_assem::transform_math::{transform_apply, transform_rotate, V3};
use oep_shell::assem::AssemblySession;
use oep_shell::assem::{
    self, MateEditError, MateOutcome, SolveOutcome, FIXTURE_ASSEMBLY, SOLVE_TOLERANCE,
};

fn origin_of(session: &AssemblySession, id: &str) -> V3 {
    let a = session
        .document()
        .assemblies
        .iter()
        .find(|a| a.id == FIXTURE_ASSEMBLY)
        .expect("assembly present");
    let inst = a
        .instances
        .iter()
        .find(|i| i.id == id)
        .expect("instance present");
    transform_apply(&inst.transform, V3::new(0.0, 0.0, 0.0))
}

fn up_of(session: &AssemblySession, id: &str) -> V3 {
    let a = session
        .document()
        .assemblies
        .iter()
        .find(|a| a.id == FIXTURE_ASSEMBLY)
        .expect("assembly present");
    let inst = a.instances.iter().find(|i| i.id == id).expect("instance");
    transform_rotate(&inst.transform, V3::new(0.0, 0.0, 1.0)).normalized()
}

fn aref(instance: &str, anchor: &str) -> oep_assem::AnchorRef {
    oep_assem::AnchorRef {
        instance: instance.into(),
        anchor: anchor.into(),
    }
}

// ---------------------------------------------------------------------------
// 1. Fixture + vocabulary
// ---------------------------------------------------------------------------

#[test]
fn fixture_opens_with_three_instances_and_full_mate_vocabulary() {
    let session = AssemblySession::open_fixture().expect("fixture opens");
    assert_eq!(session.instances().len(), 3);
    let infos = session.instances();
    let ids: Vec<&str> = infos.iter().map(|i| i.id.as_str()).collect();
    assert!(ids.contains(&"base") && ids.contains(&"cap") && ids.contains(&"peg"));
    // The committed mate set carries the vocabulary kinds.
    assert_eq!(session.mates().len(), 9);
    let kinds: Vec<&str> = session
        .mates()
        .iter()
        .map(|m| match m {
            Mate::Fixed { .. } => "fixed",
            Mate::Coincident { .. } => "coincident",
            Mate::OnPlane { .. } => "on-plane",
            Mate::PlaneDistance { .. } => "plane-distance",
            Mate::Parallel { .. } => "parallel",
        })
        .collect();
    for kind in ["fixed", "on-plane", "plane-distance", "parallel"] {
        assert!(kinds.contains(&kind), "fixture carries {kind}");
    }
}

#[test]
fn mate_grammar_parses_all_five_kinds() {
    let fixed = assem::parse_mate("fixed", &["base"]).expect("fixed parses");
    assert!(matches!(fixed, Mate::Fixed { .. }));
    let co = assem::parse_mate("coincident", &["peg", "origin", "cap", "origin"]).expect("parses");
    assert!(matches!(co, Mate::Coincident { .. }));
    let op = assem::parse_mate("on-plane", &["cap", "origin", "base", "top"]).expect("parses");
    assert!(matches!(op, Mate::OnPlane { .. }));
    let pd = assem::parse_mate("plane-distance", &["cap", "origin", "base", "inX", "30"])
        .expect("parses");
    assert!(matches!(pd, Mate::PlaneDistance { .. }));
    let par = assem::parse_mate("parallel", &["cap", "top", "base", "top"]).expect("parses");
    assert!(matches!(par, Mate::Parallel { .. }));
    assert!(assem::parse_mate("bogus", &[]).is_err());
}

// ---------------------------------------------------------------------------
// 2. Live solve: instances follow
// ---------------------------------------------------------------------------

#[test]
fn live_solve_places_the_three_part_assembly() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    match session.solve() {
        SolveOutcome::Solved(report) => {
            assert!(report.status.is_success());
            assert!(
                report.max_distance_error < SOLVE_TOLERANCE,
                "max dist {:.3e}",
                report.max_distance_error
            );
            assert_eq!(report.moved.len(), 2, "cap + peg move");
        }
        SolveOutcome::Diverged(e) => panic!("solve diverged: {e}"),
    }
    let cap = origin_of(&session, "cap");
    assert!((cap.x - 36.0).abs() < 1e-6, "cap x {cap:?}");
    assert!((cap.y - 18.0).abs() < 1e-6, "cap y {cap:?}");
    assert!((cap.z - 10.0).abs() < 1e-6, "cap z {cap:?}");
    assert!(up_of(&session, "cap").dot(V3::new(0.0, 0.0, 1.0)) > 1.0 - 1e-9);
    let peg = origin_of(&session, "peg");
    assert!((peg.x - 18.0).abs() < 1e-6, "peg x {peg:?}");
    assert!((peg.y - 22.0).abs() < 1e-6, "peg y {peg:?}");
    assert!((peg.z - 10.0).abs() < 1e-6, "peg z {peg:?}");
    // The base is fixed.
    assert!((origin_of(&session, "base") - V3::new(0.0, 0.0, 0.0)).norm() < 1e-9);
}

#[test]
fn mate_edit_resolves_and_instances_follow_immediately() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    assert!(matches!(session.solve(), SolveOutcome::Solved(_)));
    // 36 mm -> 30 mm holds cap's x to the plane-distance mate.
    match session.edit_mate(
        2,
        Mate::PlaneDistance {
            point: aref("cap", "origin"),
            plane: aref("base", "inX"),
            distance: 30.0,
        },
    ) {
        MateOutcome::AppliedAndSolved { report, .. } => {
            assert!(report.status.is_success());
        }
        other => panic!("edit must solve live: {other:?}"),
    }
    let cap = origin_of(&session, "cap");
    assert!((cap.x - 30.0).abs() < 1e-6, "cap x follows to 30: {cap:?}");
    assert!((cap.y - 18.0).abs() < 1e-6, "cap y unchanged: {cap:?}");
}

#[test]
fn mate_delete_relaxes_and_re_resolves() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    // Delete cap's x plane-distance BEFORE the first solve, while the cap is
    // still mis-placed at x=10. The remaining mates pull it to y=18, z=10,
    // upright, but x stays a free degree of freedom at its initial value:
    // that is the evidence the delete relaxed the assembly.
    match session.remove_mate(2) {
        MateOutcome::AppliedAndSolved { report, .. } => {
            assert!(report.status.is_success());
            assert!(
                report.dof >= 1,
                "x is free after the delete: dof={}",
                report.dof
            );
        }
        other => panic!("delete must solve live: {other:?}"),
    }
    assert_eq!(session.mates().len(), 8);
    let cap = origin_of(&session, "cap");
    assert!(
        (cap.x - 10.0).abs() < 1e-6,
        "cap x stays free at its initial 10: {cap:?}"
    );
    assert!((cap.y - 18.0).abs() < 1e-6, "cap y is pinned: {cap:?}");
    assert!((cap.z - 10.0).abs() < 1e-6, "cap z is pinned: {cap:?}");
    assert!(up_of(&session, "cap").dot(V3::new(0.0, 0.0, 1.0)) > 1.0 - 1e-9);
}

// ---------------------------------------------------------------------------
// 3. Diverging solve: typed error, never a fabricated pose
// ---------------------------------------------------------------------------

#[test]
fn diverging_mate_returns_typed_error_and_never_touches_poses() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    assert!(matches!(session.solve(), SolveOutcome::Solved(_)));
    let cap_before = session
        .document()
        .assemblies
        .iter()
        .find(|a| a.id == FIXTURE_ASSEMBLY)
        .unwrap()
        .instances
        .iter()
        .find(|i| i.id == "cap")
        .unwrap()
        .transform
        .clone();
    let peg_before = session
        .document()
        .assemblies
        .iter()
        .find(|a| a.id == FIXTURE_ASSEMBLY)
        .unwrap()
        .instances
        .iter()
        .find(|i| i.id == "peg")
        .unwrap()
        .transform
        .clone();

    // A coincident mate pulling peg (18,22,10) onto cap (36,18,10) while both
    // are pinned by their plane-distances cannot be satisfied.
    match session.add_mate(Mate::Coincident {
        a: aref("peg", "origin"),
        b: aref("cap", "origin"),
    }) {
        MateOutcome::AppliedButDiverged { error, .. } => {
            // The typed error names the solver failure.
            assert_eq!(assem::workbench_error_kind(&error), "solver-failed");
        }
        other => panic!("coincident-over-pinned must diverge: {other:?}"),
    }
    // No fabricated pose: both movable placements are untouched.
    let cap_after = session
        .document()
        .assemblies
        .iter()
        .find(|a| a.id == FIXTURE_ASSEMBLY)
        .unwrap()
        .instances
        .iter()
        .find(|i| i.id == "cap")
        .unwrap()
        .transform
        .clone();
    let peg_after = session
        .document()
        .assemblies
        .iter()
        .find(|a| a.id == FIXTURE_ASSEMBLY)
        .unwrap()
        .instances
        .iter()
        .find(|i| i.id == "peg")
        .unwrap()
        .transform
        .clone();
    assert_eq!(cap_after, cap_before, "cap pose must be unchanged");
    assert_eq!(peg_after, peg_before, "peg pose must be unchanged");
    // The edit stays in the set (an engineer can fix it), and deleting it
    // returns to the solved placement.
    assert_eq!(session.mates().len(), 10);
    match session.remove_mate(9) {
        MateOutcome::AppliedAndSolved { .. } => {}
        other => panic!("removing the bad mate must solve: {other:?}"),
    }
    assert!((origin_of(&session, "cap").x - 36.0).abs() < 1e-6);
}

#[test]
fn consistent_coincident_create_and_delete_solve_live() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    assert!(matches!(session.solve(), SolveOutcome::Solved(_)));
    // First move peg onto cap's footprint by editing its plane-distances.
    assert!(matches!(
        session.edit_mate(
            6,
            Mate::PlaneDistance {
                point: aref("peg", "origin"),
                plane: aref("base", "inX"),
                distance: 36.0,
            }
        ),
        MateOutcome::AppliedAndSolved { .. }
    ));
    assert!(matches!(
        session.edit_mate(
            7,
            Mate::PlaneDistance {
                point: aref("peg", "origin"),
                plane: aref("base", "outY"),
                distance: 32.0,
            }
        ),
        MateOutcome::AppliedAndSolved { .. }
    ));
    let peg = origin_of(&session, "peg");
    assert!((peg.x - 36.0).abs() < 1e-6 && (peg.y - 18.0).abs() < 1e-6);
    // The coincident is now consistent (redundant with the distances).
    match session.add_mate(Mate::Coincident {
        a: aref("peg", "origin"),
        b: aref("cap", "origin"),
    }) {
        MateOutcome::AppliedAndSolved { report, .. } => {
            assert!(report.status.is_success(), "redundant-okay is success");
        }
        other => panic!("consistent coincident must solve: {other:?}"),
    }
    // Delete it again.
    match session.remove_mate(9) {
        MateOutcome::AppliedAndSolved { .. } => {}
        other => panic!("removing coincident must solve: {other:?}"),
    }
    assert_eq!(session.mates().len(), 9);
}

#[test]
fn rejected_edits_are_typed_and_do_not_mutate_the_set() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    // Unknown anchor: refused, constraint set unchanged.
    match session.add_mate(Mate::OnPlane {
        point: aref("cap", "no-such-anchor"),
        plane: aref("base", "top"),
    }) {
        MateOutcome::Rejected { error, .. } => {
            assert!(matches!(error, MateEditError::Workbench(_)));
        }
        other => panic!("bad anchor must be rejected: {other:?}"),
    }
    assert_eq!(session.mates().len(), 9);
    // Out-of-range index: typed rejection.
    match session.remove_mate(999) {
        MateOutcome::Rejected {
            error: MateEditError::IndexOutOfBounds(999, 9),
            ..
        } => {}
        other => panic!("oob index must be typed rejection: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 4. The viewport: rendered frames prove the instances followed
// ---------------------------------------------------------------------------

#[test]
fn render_ppm_proves_instances_follow_the_solve() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    let out = std::env::temp_dir();
    let before = out.join("editor-assem-before.ppm");
    let after = out.join("editor-assem-after.ppm");
    let b = session
        .render_ppm(before.to_str().unwrap())
        .expect("render before");
    let bytes = std::fs::read(&before).expect("read ppm");
    assert_eq!(&bytes[0..15], b"P6\n640 480\n255\n", "P6 header");
    assert!(matches!(session.solve(), SolveOutcome::Solved(_)));
    let a = session
        .render_ppm(after.to_str().unwrap())
        .expect("render after");
    assert_ne!(
        b.sha256, a.sha256,
        "the mis-placed and solved fixtures must render differently"
    );
}

// ---------------------------------------------------------------------------
// 5. Mechanism motion: deterministic crank-slider from the kinematics lane
// ---------------------------------------------------------------------------

#[test]
fn mechanism_motion_is_deterministic_slider_physics() {
    let session = AssemblySession::open_fixture().expect("fixture");
    let base = std::env::temp_dir().join("editor-assem-motion-a");
    let other = std::env::temp_dir().join("editor-assem-motion-b");
    let a = session
        .animate(36, base.to_str().unwrap())
        .expect("animate a");
    let b = session
        .animate(36, other.to_str().unwrap())
        .expect("animate b");
    // Physical numbers from the committed crank-slider fixture.
    assert_eq!(a.frames, 36);
    assert_eq!(a.free_dof_undriven, 1, "crank-slider is a 1-DOF mechanism");
    assert!(
        a.worst_residual < 1e-6,
        "closed-loop residual {:.3e}",
        a.worst_residual
    );
    assert!(
        a.worst_translation_error < 1e-4,
        "analytic match {:.3e}",
        a.worst_translation_error
    );
    // Determinism: two full recordings are byte-identical in the trace.
    assert_eq!(a.trace_sha256, b.trace_sha256, "traces must be identical");
    let trace_a = base.join("trace.json");
    let frames_a = std::path::Path::new(&a.out_dir).join("frames");
    assert!(trace_a.is_file(), "trace written");
    let ppm_count = std::fs::read_dir(&frames_a)
        .expect("frames dir")
        .filter(|e| {
            e.as_ref()
                .map(|d| d.path().extension().map(|x| x == "ppm").unwrap_or(false))
                .unwrap_or(false)
        })
        .count();
    assert_eq!(ppm_count, 36, "one PPM per frame");
    // Piston stroke: 2x crank radius = 60 mm over the turn.
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&trace_a).expect("trace")).expect("json");
    let frames = json["frames"].as_array().expect("frames array");
    let mut x_pos = Vec::new();
    for f in frames {
        let piston = &f["bodies"]["inst-piston"];
        x_pos.push(piston[12].as_f64().expect("piston x"));
        assert!(
            (piston[13].as_f64().unwrap() - 50.0).abs() < 1e-6,
            "y on groove"
        );
    }
    let stroke = x_pos.iter().fold(f64::NEG_INFINITY, |m, v| m.max(*v))
        - x_pos.iter().fold(f64::INFINITY, |m, v| m.min(*v));
    assert!((stroke - 60.0).abs() < 1e-3, "piston stroke {stroke}");
}

// ---------------------------------------------------------------------------
// 6. Session journal is the observable record
// ---------------------------------------------------------------------------

#[test]
fn journal_records_every_edit_and_solve() {
    let mut session = AssemblySession::open_fixture().expect("fixture");
    let before = session.journal().len();
    assert!(matches!(session.solve(), SolveOutcome::Solved(_)));
    let _ = session.add_mate(Mate::Coincident {
        a: aref("peg", "origin"),
        b: aref("cap", "origin"),
    });
    assert!(
        session.journal().len() >= before + 2,
        "solve + edit recorded: {}",
        session.journal().len()
    );
}
