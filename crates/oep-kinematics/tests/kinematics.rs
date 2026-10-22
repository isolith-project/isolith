//! P7-M3-T02 acceptance-driven integration tests for the mates/kinematics demo.
//!
//! The fixture is an inline crank-slider with a closed-form analytic solution,
//! so the solver's output is verifiable to machine precision on REAL material:
//! a real `oep-geo` model (the corpus golden bracket) is instanced at the base
//! of the mechanism, and the solved motion is compared to the analytic ground
//! truth frame by frame. These numbers are the recorded-demo acceptance gate.

use std::path::Path;

use oep_geo::math::Vec3;
use oep_geo::Transform;
use oep_kinematics::demo;
use oep_kinematics::pose;
use oep_kinematics::solver::Solver;

const GOLDEN_OEPGEO: &str = include_str!("golden/example-bracket.oepgeo.json");

#[test]
fn golden_copy_is_byte_identical_to_real_corpus_material() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let real_path = manifest.join("../oep-geo/tests/golden/example-bracket.oepgeo.json");
    let real = std::fs::read(&real_path)
        .unwrap_or_else(|e| panic!("real oep-geo golden {real_path:?}: {e}"));
    assert_eq!(
        std::fs::read(manifest.join("tests/golden/example-bracket.oepgeo.json"))
            .expect("embedded copy"),
        real,
        "the oep-kinematics copy of the real-corpus golden must stay byte-identical \
         to crates/oep-geo/tests/golden/example-bracket.oepgeo.json"
    );
}

#[test]
fn demo_document_builds_from_real_model_and_validates() {
    let doc = demo::build_demo_document();
    assert!(doc.format() == "oep-geo" && doc.version() == oep_geo::document::VERSION);
    // Re-parse the embedded golden independently.
    let golden = oep_geo::Document::from_json_pretty(GOLDEN_OEPGEO).unwrap();
    assert_eq!(golden.assemblies.len(), 2);
    let demo_asm = doc
        .assemblies
        .iter()
        .find(|a| a.id == "oep-assembly-demo")
        .expect("demo assembly");
    assert_eq!(demo_asm.instances.len(), 6);
    let errors = doc.validate();
    assert!(errors.is_empty(), "demo document must validate: {errors:?}");
}

#[test]
fn mechanism_mates_shape_is_as_expected() {
    let mech = demo::build_demo_mechanism();
    assert_eq!(mech.unknowns(), 3 * 6);
    assert_eq!(mech.residual_count(), 6 + 6 + 6 + 5);
}

#[test]
fn solver_matches_analytic_ground_truth() {
    let mut mech = demo::build_demo_mechanism();
    let solver = Solver::default();
    for i in [0_usize, 6, 15, 22, 30, 45, 60] {
        let theta = 2.0 * std::f64::consts::PI * i as f64 / 90.0;
        let (_poses, residual, te, oe) = demo::solve_frame(&mut mech, theta, &solver);
        assert!(residual < 1e-8, "residual {residual} at theta={theta}");
        assert!(te < 1e-6, "translation err {te} mm at theta={theta}");
        assert!(oe < 1e-6, "orientation err {oe} rad at theta={theta}");
    }
}

#[test]
fn closed_loop_and_stroke_are_physically_correct() {
    // Stroke of an inline crank-slider: 2 * R = 60 mm around 50 +- 50.
    let a = demo::analytic_solution(0.0);
    let b = demo::analytic_solution(std::f64::consts::PI);
    let x0 = a["inst-piston"].position.x;
    let x180 = b["inst-piston"].position.x;
    assert!(
        (x0 - x180 - 2.0 * demo::CRANK_R).abs() < 1e-9,
        "{x0} vs {x180}"
    );
    // Piston stays on the prismatic groove line.
    for map in [&a, &b] {
        assert!((map["inst-piston"].position.y - demo::PIVOT[1]).abs() < 1e-12);
    }
}

#[test]
fn full_demo_run_passes_recording_gates() {
    let result = demo::run_demo(24);
    assert_eq!(result.frames.len(), 24);
    assert!(
        result.worst_residual < 1e-8,
        "worst residual {}",
        result.worst_residual
    );
    assert!(
        result.worst_translation_error < 1e-6,
        "worst translation {}",
        result.worst_translation_error
    );
    assert!(
        result.worst_orientation_error < 1e-6,
        "worst orientation {}",
        result.worst_orientation_error
    );
    assert_eq!(
        result.free_dof_undriven, 1,
        "a planar four-joint crank-slider mechanism has exactly 1 DOF \
         when the crank is not commanded; got {}",
        result.free_dof_undriven
    );
}

#[test]
fn trace_is_deterministic_and_valid_json() {
    let result = demo::run_demo(6);
    let tmp_a = std::env::temp_dir().join(format!("kw-trace-a-{}.json", std::process::id()));
    let tmp_b = std::env::temp_dir().join(format!("kw-trace-b-{}.json", std::process::id()));
    demo::write_trace(&result, &result.frames, tmp_a.to_str().unwrap()).unwrap();
    demo::write_trace(&result, &result.frames, tmp_b.to_str().unwrap()).unwrap();
    let buf_a = std::fs::read(&tmp_a).unwrap();
    let buf_b = std::fs::read(&tmp_b).unwrap();
    assert_eq!(buf_a, buf_b, "trace bytes must be deterministic");
    let text = String::from_utf8(buf_a).unwrap();
    let json: serde_json::Value = serde_json::from_str(&text).expect("trace parses as JSON");
    assert_eq!(json["fixture"], "crank-slider");
    assert_eq!(json["frames"].as_array().unwrap().len(), 6);
    let frame0 = &json["frames"][0]["bodies"]["inst-piston"];
    assert_eq!(frame0.as_array().unwrap().len(), 16);
}

#[test]
fn renders_ppm_frames_with_our_rasterizer() {
    let result = demo::run_demo(4);
    let dir = std::env::temp_dir().join(format!("kw-frames-{}", std::process::id()));
    let dir = dir.to_str().unwrap();
    let rendered = demo::render_frames(&result.mechanism, &result.frames, dir).expect("renders");
    assert_eq!(rendered.len(), 4);
    let hdr = std::fs::read(&rendered[0]).unwrap();
    assert!(hdr.starts_with(b"P6\n640 480\n255\n"), "PPM P6 header");
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn instance_transform_round_trip_through_poses() {
    // A real instance transform from the golden model (fastener at (10,30,0)).
    let t = Transform([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 30.0, 0.0, 1.0,
    ]);
    let p = pose::Pose::from_transform(&t);
    assert_eq!(p.position, Vec3::new(10.0, 30.0, 0.0));
}

#[test]
fn spherical_and_rigid_mates_exist_in_model() {
    // Constructor/arity smoke test for the two extra DMU-like joints.
    let sph = oep_kinematics::mate::Spherical::new("a", Vec3::zero(), "b", Vec3::zero());
    assert_eq!(sph.residual_len(), 3);
    let rigid = oep_kinematics::mate::Rigid::new("a", "b");
    assert_eq!(rigid.residual_len(), 6);
}
