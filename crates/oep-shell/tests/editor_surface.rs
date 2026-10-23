//! INTERACTIVE-SHELL acceptance (TDD: the acceptance path comes first): a
//! user sketches a rectangle, constrains it (fixed + horizontal + vertical +
//! length via the variable set), solves it, extrudes it, changes a dimension
//! in the variable set, sees the solid update live, and exports STEP.
//!
//! Locked here on real session output (measured, not mocked):
//!
//! 1. The sketch solves fully constrained (dof 0) with width/height coming
//!    from the variable set (FreeCAD-v1-variable-set semantics).
//! 2. Extrude produces the analytic solid (80 x 40 x 10 -> volume 32000) in
//!    the canonical oep-geo document; the body bbox is the real mesh bbox.
//! 3. Editing the width variable to 120 re-solves + re-derives the solid live:
//!    volume 48000, bbox x-max 120. Two renders before/after differ; two
//!    renders of the same state match byte-for-byte (sha256).
//! 4. STEP export writes a real AP214 file whose bytes re-import through the
//!    importer hub (canonical model round trip, never fabricated).
//! 5. Undo/redo navigate the real feature graph: undo re-derives the prior
//!    solid, redo replays the edit.
//! 6. Selection picks the solid by screen ray; orbit/zoom/pan move the camera
//!    deterministically; an unconstrained extrude is a typed error, never a
//!    panic.
//! 7. Revolve from a meridian profile sweeps the analytic faceted ring, and
//!    the angle follows the variable set live.

use oep_import::{ImportOutcome, SourceKind};
use oep_shell::editor::{
    EditorSession, PART_ID, REVOLVE_SEGMENTS, VIEWPORT_HEIGHT, VIEWPORT_WIDTH,
};
use oep_sketch::sketch::{Constraint, DimExpr};
use oep_sketch::varset::VariableSet;

/// Build the acceptance rectangle exactly as the oep-sketch card solves it:
/// bottom-left anchored, width/height driven by variable-set entries.
fn build_rectangle(session: &mut EditorSession) {
    session.point(0.0, 0.0).unwrap(); // p0
    session.point(3.0, 1.0).unwrap(); // p1
    session.point(4.0, 4.0).unwrap(); // p2
    session.point(1.0, 5.0).unwrap(); // p3
    session.line("p0", "p1").unwrap(); // l0
    session.line("p1", "p2").unwrap(); // l1
    session.line("p2", "p3").unwrap(); // l2
    session.line("p3", "p0").unwrap(); // l3
    session
        .constrain(Constraint::Fixed {
            point: "p0".into(),
            x: 0.0,
            y: 0.0,
        })
        .unwrap();
    session
        .constrain(Constraint::Horizontal { line: "l0".into() })
        .unwrap();
    session
        .constrain(Constraint::Horizontal { line: "l2".into() })
        .unwrap();
    session
        .constrain(Constraint::Vertical { line: "l1".into() })
        .unwrap();
    session
        .constrain(Constraint::Vertical { line: "l3".into() })
        .unwrap();
    session
        .constrain(Constraint::Length {
            line: "l0".into(),
            expr: DimExpr::Variable("width".into()),
        })
        .unwrap();
    session
        .constrain(Constraint::Length {
            line: "l1".into(),
            expr: DimExpr::Variable("height".into()),
        })
        .unwrap();
    // The dimensions are driven by the variable set, never literals.
    let mut vars = VariableSet::new();
    vars.set("width", 80.0);
    vars.set("height", 40.0);
    for (name, value) in vars.variables {
        session.set_var(&name, value).unwrap();
    }
}

#[test]
fn acceptance_sketch_constrain_extrude_edit_export_step() {
    let mut session = EditorSession::new();
    build_rectangle(&mut session);

    // Solve: fully constrained rectangle, dims from the variable set.
    let solve = session.solve().expect("rectangle solves");
    assert_eq!(
        solve.dof, 0,
        "dof {} residual {}",
        solve.dof, solve.residual
    );
    assert!(solve.residual < 1e-9);
    assert!((solve.dimensions["width"] - 80.0).abs() < 1e-6);
    assert!((solve.dimensions["height"] - 40.0).abs() < 1e-6);
    let (x1, _) = solve.points["p1"];
    assert!((x1 - 80.0).abs() < 1e-6, "p1.x = width = 80, got {x1}");

    // Extrude: the analytic solid lands in the canonical document.
    let report = session.extrude(DimExpr::Literal(10.0)).expect("extrudes");
    assert!(
        (report.volume - 32000.0).abs() < 1e-6,
        "volume {}",
        report.volume
    );
    assert!(report.faces >= 12, "at least 12 faces on the prism");
    assert_close_bbox(report.bbox, [0.0, 0.0, 0.0, 80.0, 40.0, 10.0]);
    assert!(
        session.document().validate().is_empty(),
        "document validates"
    );
    // Canonical model is real: the body carries the derived solid's bbox.
    let body = &session.document().bodies[0];
    assert_close_bbox(
        [
            body.bbox.unwrap().min[0],
            body.bbox.unwrap().min[1],
            body.bbox.unwrap().min[2],
            body.bbox.unwrap().max[0],
            body.bbox.unwrap().max[1],
            body.bbox.unwrap().max[2],
        ],
        [0.0, 0.0, 0.0, 80.0, 40.0, 10.0],
    );
    assert!(session
        .document()
        .features
        .iter()
        .any(|f| f.kind == "Extrude"));

    // Render determinism: same state -> same sha; edited state -> different.
    let dir = std::env::temp_dir().join(format!("oep-editor-accept-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    session.frame();
    let first = dir.join("before.png");
    let r1 = session.render(first.to_str().unwrap()).unwrap();
    let r1b = session.render(first.to_str().unwrap()).unwrap();
    assert_eq!(
        r1.sha256, r1b.sha256,
        "same-state renders are byte-identical"
    );
    assert_eq!(r1.width, VIEWPORT_WIDTH);
    assert_eq!(r1.height, VIEWPORT_HEIGHT);

    // Live recompute: change a dimension in the variable set, solid follows.
    let edit = session
        .set_var("width", 120.0)
        .expect("var edit drives live recompute");
    assert!((edit.volume_before.unwrap() - 32000.0).abs() < 1e-6);
    assert!((edit.volume_after.unwrap() - 48000.0).abs() < 1e-6);
    let body = &session.document().bodies[0];
    assert!(
        (body.bbox.unwrap().max[0] - 120.0).abs() < 1e-6,
        "body follows the edit to 120"
    );

    let second = dir.join("after.png");
    let r2 = session.render(second.to_str().unwrap()).unwrap();
    assert_ne!(
        r1.sha256, r2.sha256,
        "the solid update must be visible in the rendered viewport"
    );

    // Undo/redo over the real feature graph. Undo pops the pre-edit snapshot
    // (width 80) and re-derives the solid; redo replays the edit (width 120).
    session.undo().expect("undo");
    assert!(
        (session.props().volume.unwrap() - 32000.0).abs() < 1e-6,
        "undo re-derives the 80-wide solid"
    );
    assert!(
        (session.document().bodies[0].bbox.unwrap().max[0] - 80.0).abs() < 1e-6,
        "undo restores the 80-wide body"
    );
    session.redo().expect("redo");
    assert!(
        (session.props().volume.unwrap() - 48000.0).abs() < 1e-6,
        "redo replays the edit back to 120 wide"
    );

    // Selection by screen ray on the framed solid.
    session.frame();
    let sel = session.select_at(VIEWPORT_WIDTH as f64 / 2.0, VIEWPORT_HEIGHT as f64 / 2.0);
    assert!(sel.is_some(), "centre ray hits the solid");
    assert_eq!(
        session.selected(),
        Some(&format!("part:{PART_ID}:body:body-1")[..])
    );

    // STEP export: real AP214 bytes that re-import as a canonical document.
    let step_path = dir.join("edpart.step");
    let step = session
        .export_step(step_path.to_str().unwrap())
        .expect("STEP export");
    assert!(step.closed_shells >= 1, "watertight solid shells");
    let bytes = std::fs::read(&step_path).unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("SHELL_BASED_SURFACE_MODEL"));
    match oep_import::import_bytes(SourceKind::Step, &bytes).expect("no panic") {
        ImportOutcome::Imported { document, .. } => {
            assert_eq!(document.format(), "oep-geo");
            assert!(document.validate().is_empty());
            assert!(document.features.iter().any(|f| {
                f.parameters
                    .get("triangle_count")
                    .and_then(|v| v.as_f64())
                    .is_some()
            }));
        }
        other => panic!("exported STEP must re-import, got {other:?}"),
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn render_hd_is_a_real_render_not_an_upscale() {
    let mut session = EditorSession::new();
    build_rectangle(&mut session);
    session.solve().unwrap();
    session.extrude(DimExpr::Literal(10.0)).unwrap();
    session.frame();
    let dir = std::env::temp_dir().join(format!("oep-editor-hd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let hd = dir.join("hd-before.png");
    let r = session
        .render_sized(hd.to_str().unwrap(), Some(1920), Some(1080), false)
        .unwrap();
    assert_eq!(r.width, 1920, "real 1920-wide raster");
    assert_eq!(r.height, 1080, "real 1080-tall raster");
    assert!(!r.present);
    assert_eq!(r.grid_lines, 0);
    assert_eq!(r.axis_arms, 0);
    assert_eq!(r.label_count, 0);
    // NOTE: the real determinism/golden guard for the default path is the
    // recorded 640x480 sha256 in demos/editor-sketch-extrude/manifest.json,
    // asserted byte-for-byte by the pytest drive; here we only prove the HD
    // path reports real dimensions.
}

#[test]
fn present_render_sources_labels_from_the_variable_set() {
    let mut session = EditorSession::new();
    build_rectangle(&mut session);
    session.solve().unwrap();
    session.extrude(DimExpr::Literal(10.0)).unwrap();
    session.frame();
    let dir = std::env::temp_dir().join(format!("oep-editor-present-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // Present render draws grid + triad + the two driven dimension labels.
    let before = dir.join("present-before.png");
    let r = session
        .render_sized(before.to_str().unwrap(), Some(1280), Some(720), true)
        .unwrap();
    assert!(r.present);
    assert_eq!(r.width, 1280);
    assert_eq!(r.height, 720);
    assert!(r.grid_lines > 0, "world grid segments drawn");
    assert_eq!(r.axis_arms, 3, "XYZ axes triad drawn");
    assert_eq!(
        r.label_count, 2,
        "width + height labels from the variable set"
    );

    // Selection adds a label line (sourced from the real selection state).
    // Picking is done through the session viewport, so aim at its centre and
    // render the present frame at the same size.
    session.select_at(320.0, 240.0);
    let selected = dir.join("present-selected.png");
    let r2 = session
        .render_sized(selected.to_str().unwrap(), None, None, true)
        .unwrap();
    assert_eq!(r2.label_count, 3, "selection id becomes a third label");

    // Editing the variable moves the solid AND the labels: the present frame
    // changes (same overlay composition, different raster).
    session.set_var("width", 120.0).unwrap();
    let after = dir.join("present-after.png");
    let r3 = session
        .render_sized(after.to_str().unwrap(), Some(1280), Some(720), true)
        .unwrap();
    assert_eq!(r3.label_count, 3);
    assert_eq!(r3.axis_arms, 3);
    assert!(r3.grid_lines > 0);
    assert_ne!(
        r2.sha256, r3.sha256,
        "the variable edit must move the present frame (solid + labels)"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn camera_orbit_zoom_pan_are_deterministic() {
    let mut session = EditorSession::new();
    session.frame();
    session.orbit(0.5, 0.25);
    session.zoom(0.8);
    session.pan(5.0, -3.0);
    let c1 = *session.camera();
    // Deterministic: replaying the same gestures gives the same camera.
    let mut replay = EditorSession::new();
    replay.frame();
    replay.orbit(0.5, 0.25);
    replay.zoom(0.8);
    replay.pan(5.0, -3.0);
    assert_eq!(*replay.camera(), c1);
}

#[test]
fn unconstrained_extrude_is_a_typed_error_not_a_panic() {
    let mut session = EditorSession::new();
    session.point(0.0, 0.0).unwrap();
    session.point(1.0, 0.0).unwrap();
    session.line("p0", "p1").unwrap();
    // Not a closed loop, and never solved: typed error, never a panic.
    let err = session.extrude(DimExpr::Literal(5.0)).unwrap_err();
    assert!(
        err.to_string().contains("closed loop") || err.to_string().contains("solve"),
        "typed error text: {err}"
    );
}

#[test]
fn revolve_sweeps_the_analytic_faceted_ring_and_follows_the_variable() {
    let mut session = EditorSession::new();
    // Meridian ring in the radial-axial half plane: x = radial >= 0.
    session.point(0.4, 0.0).unwrap();
    session.point(1.0, 0.0).unwrap();
    session.point(1.0, 1.0).unwrap();
    session.point(0.4, 1.0).unwrap();
    session.line("p0", "p1").unwrap();
    session.line("p1", "p2").unwrap();
    session.line("p2", "p3").unwrap();
    session.line("p3", "p0").unwrap();
    session
        .constrain(Constraint::Fixed {
            point: "p0".into(),
            x: 0.4,
            y: 0.0,
        })
        .unwrap();
    session
        .constrain(Constraint::Horizontal { line: "l0".into() })
        .unwrap();
    session
        .constrain(Constraint::Vertical { line: "l1".into() })
        .unwrap();
    session
        .constrain(Constraint::Horizontal { line: "l2".into() })
        .unwrap();
    session
        .constrain(Constraint::Vertical { line: "l3".into() })
        .unwrap();
    session
        .constrain(Constraint::Length {
            line: "l0".into(),
            expr: DimExpr::Literal(0.6),
        })
        .unwrap();
    session
        .constrain(Constraint::Length {
            line: "l1".into(),
            expr: DimExpr::Variable("height".into()),
        })
        .unwrap();
    session.set_var("height", 1.0).unwrap();
    session.set_var("angle", std::f64::consts::PI).unwrap();
    let solve = session.solve().expect("ring solves");
    assert_eq!(solve.dof, 0, "ring fully constrained");

    let report = session
        .revolve(DimExpr::Variable("angle".into()))
        .expect("revolves");
    let _ = report;
    // Half turn about Z: faceted sector volume.
    let m = REVOLVE_SEGMENTS;
    let outer = 0.5 * m as f64 * (std::f64::consts::PI / m as f64).sin();
    let inner = 0.5 * 0.16 * m as f64 * (std::f64::consts::PI / m as f64).sin();
    let expected = (outer - inner) * 1.0;
    assert!(
        (session.props().volume.unwrap() - expected).abs() < 1e-4,
        "half-turn faceted ring volume"
    );
    // The angle follows the variable set live: halving the angle re-sweeps a
    // quarter sector (correct faceted quarter-sector expected at the same
    // segment count).
    session
        .set_var("angle", std::f64::consts::FRAC_PI_2)
        .unwrap();
    let q_area_outer = 0.5 * m as f64 * (std::f64::consts::FRAC_PI_2 / m as f64).sin();
    let q_area_inner = 0.5 * 0.16 * m as f64 * (std::f64::consts::FRAC_PI_2 / m as f64).sin();
    let expected_q = (q_area_outer - q_area_inner) * 1.0;
    let vol_q = session.props().volume.unwrap();
    assert!(
        (vol_q - expected_q).abs() < 1e-4,
        "angle-var edit re-sweeps the solid ({vol_q} vs {expected_q})"
    );
    assert!(
        vol_q < expected * 0.99,
        "quarter sector is smaller than the half turn ({vol_q} vs {expected})"
    );
    assert!(session.document_json().unwrap().contains("\"Revolve\""));
}

#[test]
fn every_shown_state_is_the_real_document() {
    let mut session = EditorSession::new();
    build_rectangle(&mut session);
    session.solve().unwrap();
    session.extrude(DimExpr::Literal(7.0)).unwrap();
    let json = session.document_json().unwrap();
    // The canonical layer is the single source of truth: the document
    // serializes deterministically and re-parses cleanly with the same
    // feature sequence, variables, body volume and body bbox. (Solver
    // coordinates may move by 1 ULP through a JSON round trip, so equality is
    // asserted semantically, not bit-for-bit.)
    let doc = oep_geo::Document::from_json_pretty(&json).unwrap();
    assert!(doc.validate().is_empty());
    let kinds: Vec<&str> = doc.features.iter().map(|f| f.kind.as_str()).collect();
    assert_eq!(kinds, ["Sketch", "VariableSet", "Extrude"]);
    let vars = &doc.features[1].parameters;
    assert_eq!(vars["variables"]["width"], 80.0);
    assert_eq!(vars["variables"]["height"], 40.0);
    assert_eq!(doc.bodies[0].bbox.unwrap().max[2], 7.0);
    assert!((session.props().volume.unwrap() - 80.0 * 40.0 * 7.0).abs() < 1e-6);
}

/// Assert two bbox arrays agree component-wise within the solver/tessellation
/// tolerance (1e-5). The OSS solver may place a Fixed anchor at ~1e-13.
fn assert_close_bbox(a: [f64; 6], b: [f64; 6]) {
    for i in 0..6 {
        assert!((a[i] - b[i]).abs() < 1e-5, "bbox[{i}] {a:?} vs {b:?}");
    }
}
