//! Machine-callable OEP interchange workbench CLI (P7-M2-T01, help surface
//! P7-M5-T02).
//!
//! Drives the same [`oep_shell::workbench::Workbench`] the shell panel will
//! host, one command per argument so the surface is scriptable and
//! headless-testable (the P7-M5-T01 CLI-parity target). A PDM client can be
//! bound with `--pdm <json-file>` later; by default each process gets a fresh
//! in-memory local stub, so `save-pdm` followed by `open-pdm` in the same
//! invocation round-trips (persistence is a P8 deliverable).
//!
//! The help surface is GENERATED from the command metadata
//! ([`oep_shell::commands`], single source of truth): `help`, `help:NAME`,
//! `help:--json` and `--markdown` all render from the registry, and
//! `--write-help-index <path>` regenerates the committed markdown index. No
//! hand-written usage block lives here, so the help cannot drift from the
//! surface.
//!
//! Usage:
//!
//! ```text
//! oep-workbench <command> [<command> ...]
//!
//!   load:cgr:PATH        load a CGR file (classic .cgr or modern V5_CFV2)
//!   load:3dxml:PATH      load a 3DXML file (ZIP or standalone XML)
//!   load:step:PATH       load an ISO 10303-21 STEP file
//!   open-pdm:ITEM:REV    open a PDM vault revision into the workbench
//!   save-pdm:ITEM:REV[:SUMMARY]
//!                        publish the current document to the PDM vault
//!   save:cgr|3dxml|step  request an interchange save lane (honest pending)
//!   journal              print the operation journal
//!   help                 print the generated command index
//!   help:NAME            print the full help for one command
//!   help:--json          print the machine-readable help index (JSON)
//!   --markdown           print the generated markdown help index
//!   --write-help-index:PATH
//!                        regenerate the committed markdown help index
//! ```
//!
//! Example:
//!
//! ```text
//! oep-workbench load:step:box-reference.step save-pdm:BOX-01:A:initial open-pdm:BOX-01:A journal
//! ```
//!
//! Exit code: 0 when every command succeeded, 1 when any command Failed or
//! the invocation was empty.

use oep_shell::assem::{self, AssemblySession, MateOutcome, SolveOutcome};
use oep_shell::commands;
use oep_shell::editor::{self, EditorSession};
use oep_shell::workbench::{Interchange, Workbench, WorkbenchAction};
use oep_sketch::sketch::{Constraint, DimExpr};

/// Dispatch one editor-surface command (`editor:`, `sk:`, `vars:`, `feat:`,
/// `view:`, `export:`, `props`, `undo`, `redo`) against the interactive
/// editor session. Returns `true` when the command ran cleanly; `false` for
/// malformed input or missing session.
fn dispatch_editor(session: &mut Option<EditorSession>, token: &str, arg: Option<&str>) -> bool {
    let err_none = |msg: &str| -> bool {
        println!("failed: {msg}");
        false
    };
    let (sub, rest) = match arg {
        Some(a) => match a.split_once(':') {
            Some((s, r)) => (s.to_string(), Some(r.to_string())),
            None => (a.to_string(), None),
        },
        None => (String::new(), None),
    };
    match token {
        "editor" => {
            if sub != "open" {
                return err_none("editor:open is the only editor command");
            }
            *session = Some(EditorSession::new());
            println!(
                "editor open: part={} body={} sketch=sketch-1",
                editor::PART_ID,
                editor::BODY_ID
            );
            true
        }
        "sk" => match sub.as_str() {
            "point" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                let args: Vec<&str> = rest.as_deref().unwrap_or("").split(':').collect();
                let (x, y) = match (
                    args.first().and_then(|v| v.parse().ok()),
                    args.get(1).and_then(|v| v.parse().ok()),
                ) {
                    (Some(x), Some(y)) => (x, y),
                    _ => return err_none("sk:point expects X:Y"),
                };
                match s.point(x, y) {
                    Ok(r) => {
                        println!("sketch point {} at ({x}, {y})", r.id);
                        true
                    }
                    Err(e) => err_none(&e.to_string()),
                }
            }
            "line" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                let args: Vec<&str> = rest.as_deref().unwrap_or("").split(':').collect();
                if args.len() != 2 {
                    return err_none("sk:line expects START:END");
                }
                match s.line(args[0], args[1]) {
                    Ok(r) => {
                        println!("sketch line {} ({}) to ({})", r.id, r.start, r.end);
                        true
                    }
                    Err(e) => err_none(&e.to_string()),
                }
            }
            "constrain" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                let args: Vec<&str> = rest.as_deref().unwrap_or("").split(':').collect();
                match parse_constraint(&args) {
                    Some(c) => match s.constrain(c) {
                        Ok(()) => {
                            println!("constraint added");
                            true
                        }
                        Err(e) => err_none(&e.to_string()),
                    },
                    None => err_none("sk:constrain expects KIND:ARGS... (see help:sk:constrain)"),
                }
            }
            "solve" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                match s.solve() {
                    Ok(rep) => {
                        println!(
                            "solved dof={} residual={:.3e} dims={} points={} lines={}",
                            rep.dof,
                            rep.residual,
                            rep.dimensions.len(),
                            rep.point_count,
                            rep.line_count
                        );
                        for (name, v) in &rep.dimensions {
                            println!("  {name} = {v:.6}");
                        }
                        true
                    }
                    Err(e) => {
                        println!("solved failed: {e}");
                        false
                    }
                }
            }
            "render" => {
                let Some(s) = session.as_ref() else {
                    return err_none("run editor:open first");
                };
                let Some(arg) = rest else {
                    return err_none("sk:render expects PATH[:WIDTH:HEIGHT][:present]");
                };
                // Trailing tokens after the path (split on ':'): the path
                // itself must not contain a colon.
                let mut parts: Vec<&str> = arg.split(':').collect();
                let present = parts.last().map(|t| *t == "present").unwrap_or(false);
                if present {
                    parts.pop();
                }
                let (path, width, height) = match parts.len() {
                    1 => (parts[0].to_string(), None, None),
                    3 => match (parts[1].parse::<u32>().ok(), parts[2].parse::<u32>().ok()) {
                        (Some(w), Some(h)) if w > 0 && h > 0 => {
                            (parts[0].to_string(), Some(w), Some(h))
                        }
                        _ => return err_none("sk:render WIDTH:HEIGHT must be positive integers"),
                    },
                    _ => return err_none("sk:render expects PATH[:WIDTH:HEIGHT][:present]"),
                };
                match s.render_sized(&path, width, height, present) {
                    Ok(r) => {
                        if r.present {
                            println!(
                                "rendered {} sha256={} {}x{} present grid={} axes={} labels={}",
                                r.path,
                                r.sha256,
                                r.width,
                                r.height,
                                r.grid_lines,
                                r.axis_arms,
                                r.label_count
                            );
                        } else {
                            println!(
                                "rendered {} sha256={} {}x{}",
                                r.path, r.sha256, r.width, r.height
                            );
                        }
                        true
                    }
                    Err(e) => err_none(&e.to_string()),
                }
            }
            "props" => print_sketch_props(session),
            other => err_none(&format!(
                "unknown sk sub-command '{other}'; see help:sk:constrain"
            )),
        },
        "vars" => match sub.as_str() {
            "list" => {
                let Some(s) = session.as_ref() else {
                    return err_none("run editor:open first");
                };
                for (name, value) in s.vars() {
                    println!("{name} = {value}");
                }
                true
            }
            "set" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                let args: Vec<&str> = rest.as_deref().unwrap_or("").split(':').collect();
                let (name, value) = match (args.first(), args.get(1).and_then(|v| v.parse().ok())) {
                    (Some(n), Some(v)) => (*n, v),
                    _ => return err_none("vars:set expects NAME:VALUE"),
                };
                match s.set_var(name, value) {
                    Ok(r) => {
                        println!(
                            "variable {name} = {value} (volume {}{}{})",
                            r.volume_before
                                .map(|v| format!("{v:.6} -> "))
                                .unwrap_or_default(),
                            r.volume_after
                                .map(|v| format!("{v:.6}"))
                                .unwrap_or_else(|| "n/a".to_string()),
                            if r.volume_after.is_some() {
                                " live recompute"
                            } else {
                                ""
                            }
                        );
                        true
                    }
                    Err(e) => err_none(&e.to_string()),
                }
            }
            other => err_none(&format!("unknown vars sub-command '{other}'")),
        },
        "feat" => match sub.as_str() {
            "extrude" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                let Some(val) = rest else {
                    return err_none("feat:extrude expects DEPTH");
                };
                match editor::parse_dim(&val).and_then(|d| s.extrude(d)) {
                    Ok(r) => {
                        println!(
                            "extruded {}: volume {:.6} faces={} edges={} bbox {:?}",
                            r.feature_id, r.volume, r.faces, r.edges, r.bbox
                        );
                        true
                    }
                    Err(e) => err_none(&e.to_string()),
                }
            }
            "revolve" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                let Some(val) = rest else {
                    return err_none("feat:revolve expects ANGLE");
                };
                match editor::parse_dim(&val).and_then(|d| s.revolve(d)) {
                    Ok(r) => {
                        println!(
                            "revolved {}: volume {:.6} faces={} edges={} bbox {:?}",
                            r.feature_id, r.volume, r.faces, r.edges, r.bbox
                        );
                        true
                    }
                    Err(e) => err_none(&e.to_string()),
                }
            }
            "recompute" => {
                let Some(s) = session.as_mut() else {
                    return err_none("run editor:open first");
                };
                match s.recompute() {
                    Ok(r) => {
                        println!(
                            "recomputed volume={} bbox={:?}",
                            r.volume
                                .map(|v| format!("{v:.6}"))
                                .unwrap_or_else(|| "n/a".into()),
                            r.bbox
                        );
                        true
                    }
                    Err(e) => err_none(&e.to_string()),
                }
            }
            other => err_none(&format!("unknown feat sub-command '{other}'")),
        },
        "view" => {
            let Some(s) = session.as_mut() else {
                return err_none("run editor:open first");
            };
            let args: Vec<&str> = rest.as_deref().unwrap_or("").split(':').collect();
            match sub.as_str() {
                "frame" => {
                    s.frame();
                    println!("camera framed");
                    true
                }
                "orbit" => match (
                    args.first().and_then(|v| v.parse().ok()),
                    args.get(1).and_then(|v| v.parse().ok()),
                ) {
                    (Some(y), Some(p)) => {
                        s.orbit(y, p);
                        println!("camera orbit");
                        true
                    }
                    _ => err_none("view:orbit expects YAW:PITCH"),
                },
                "zoom" => match args.first().and_then(|v| v.parse().ok()) {
                    Some(f) => {
                        s.zoom(f);
                        println!("camera zoom");
                        true
                    }
                    None => err_none("view:zoom expects FACTOR"),
                },
                "pan" => match (
                    args.first().and_then(|v| v.parse().ok()),
                    args.get(1).and_then(|v| v.parse().ok()),
                ) {
                    (Some(dx), Some(dy)) => {
                        s.pan(dx, dy);
                        println!("camera pan");
                        true
                    }
                    _ => err_none("view:pan expects DX:DY"),
                },
                "pick" => match (
                    args.first().and_then(|v| v.parse().ok()),
                    args.get(1).and_then(|v| v.parse().ok()),
                ) {
                    (Some(x), Some(y)) => match s.select_at(x, y) {
                        Some(sel) => {
                            println!(
                                "selected {} (part={} body={} dist={:.3})",
                                sel.id, sel.part_id, sel.body_id, sel.distance
                            );
                            true
                        }
                        None => {
                            println!("no pick at ({x}, {y})");
                            true
                        }
                    },
                    _ => err_none("view:pick expects X:Y"),
                },
                other => err_none(&format!("unknown view sub-command '{other}'")),
            }
        }
        "export" => {
            if sub != "step" {
                return err_none("export:step is the only export command");
            }
            let Some(s) = session.as_ref() else {
                return err_none("run editor:open first");
            };
            let Some(path) = rest else {
                return err_none("export:step expects PATH");
            };
            match s.export_step(&path) {
                Ok(r) => {
                    println!(
                        "exported STEP {path}: faces={} shells={} closed={}",
                        r.faces, r.shells, r.closed_shells
                    );
                    true
                }
                Err(e) => err_none(&e.to_string()),
            }
        }
        "props" => print_props(session),
        "undo" => {
            let Some(s) = session.as_mut() else {
                return err_none("run editor:open first");
            };
            match s.undo() {
                Ok(r) => {
                    println!("undo: op={} retained={}", r.op, r.count);
                    true
                }
                Err(e) => err_none(&e.to_string()),
            }
        }
        "redo" => {
            let Some(s) = session.as_mut() else {
                return err_none("run editor:open first");
            };
            match s.redo() {
                Ok(r) => {
                    println!("redo: op={} retained={}", r.op, r.count);
                    true
                }
                Err(e) => err_none(&e.to_string()),
            }
        }
        _ => err_none("unsupported editor command"),
    }
}

/// Print the sketch summary (points, lines, constraints, solved dof).
fn print_sketch_props(session: &Option<EditorSession>) -> bool {
    let Some(s) = session.as_ref() else {
        println!("failed: run editor:open first");
        return false;
    };
    match s.sketch() {
        Some(sk) => {
            println!(
                "sketch points={} lines={} constraints={} dof={}",
                sk.points.len(),
                sk.lines.len(),
                sk.constraints.len(),
                s.last_solve().map(|r| r.dof).unwrap_or(-1)
            );
            true
        }
        None => {
            println!("failed: no sketch in the session");
            false
        }
    }
}

/// Print the model measures.
fn print_props(session: &Option<EditorSession>) -> bool {
    let Some(s) = session.as_ref() else {
        println!("failed: run editor:open first");
        return false;
    };
    let p = s.props();
    println!(
        "props volume={} bbox={:?} features={} dof={} points={} lines={} constraints={}",
        p.volume
            .map(|v| format!("{v:.6}"))
            .unwrap_or_else(|| "n/a".into()),
        p.bbox,
        p.features,
        p.dof.unwrap_or(-1),
        p.sketch_points,
        p.sketch_lines,
        p.constraints
    );
    for (name, value) in &p.vars {
        println!("  var {name} = {value}");
    }
    true
}

/// Parse a `sk:constrain:KIND:ARGS...` string into an oep-sketch constraint.
fn parse_constraint(args: &[&str]) -> Option<oep_sketch::sketch::Constraint> {
    let kind = *args.first()?;
    let num = |i: usize| -> Option<f64> { args.get(i).and_then(|v| v.parse().ok()) };
    let dim = |i: usize| -> Option<DimExpr> {
        let t = args.get(i)?;
        if let Ok(v) = t.parse::<f64>() {
            Some(DimExpr::Literal(v))
        } else {
            Some(DimExpr::Variable((*t).to_string()))
        }
    };
    let ptr = |i: usize| -> Option<String> { args.get(i).map(|s| (*s).to_string()) };
    match kind {
        "fixed" => Some(Constraint::Fixed {
            point: ptr(1)?,
            x: num(2)?,
            y: num(3)?,
        }),
        "horizontal" => Some(Constraint::Horizontal { line: ptr(1)? }),
        "vertical" => Some(Constraint::Vertical { line: ptr(1)? }),
        "coincident" => Some(Constraint::Coincident {
            point_a: ptr(1)?,
            point_b: ptr(2)?,
        }),
        "length" => Some(Constraint::Length {
            line: ptr(1)?,
            expr: dim(2)?,
        }),
        "distance" => Some(Constraint::Distance {
            point_a: ptr(1)?,
            point_b: ptr(2)?,
            expr: dim(3)?,
        }),
        _ => None,
    }
}

fn print_outcome(outcome: &oep_shell::workbench::WorkbenchOutcome) -> bool {
    match outcome {
        oep_shell::workbench::WorkbenchOutcome::Loaded {
            format,
            stats,
            note,
            ..
        } => {
            println!(
                "loaded {format}: parts={} bodies={} features={} assemblies={}{}",
                stats.parts,
                stats.bodies,
                stats.features,
                stats.assemblies,
                note.as_deref()
                    .map(|n| format!(" [{n}]"))
                    .unwrap_or_default()
            );
            true
        }
        oep_shell::workbench::WorkbenchOutcome::OpenedFromPdm { stats, .. } => {
            println!(
                "opened from PDM: parts={} bodies={} features={}",
                stats.parts, stats.bodies, stats.features
            );
            true
        }
        oep_shell::workbench::WorkbenchOutcome::SavedToPdm { revision } => {
            println!(
                "saved to PDM: {}@{} state={} digest={}",
                revision.item_id,
                revision.revision,
                revision.state,
                revision.blob_digest.as_deref().unwrap_or("none")
            );
            true
        }
        oep_shell::workbench::WorkbenchOutcome::InterchangeSavePending { format, reason } => {
            println!("{format} save pending: {reason}");
            true
        }
        oep_shell::workbench::WorkbenchOutcome::Skipped { format, reason } => {
            println!("{format} load skipped: {reason}");
            true
        }
        oep_shell::workbench::WorkbenchOutcome::Failed { message } => {
            println!("failed: {message}");
            false
        }
    }
}

/// Dispatch one `assem:` sub-command against the interactive assembly surface.
/// Returns `true` when the command ran cleanly (a diverged solve is a valid
/// edit outcome, not a command failure); `false` only for malformed input or
/// missing session.
fn dispatch_assem(asm: &mut Option<AssemblySession>, arg: &str) -> bool {
    let (sub, rest) = match arg.split_once(':') {
        Some((s, r)) => (s, Some(r)),
        None => (arg, None),
    };
    match sub {
        "open" => match AssemblySession::open_fixture() {
            Ok(session) => {
                println!(
                    "assembled {}: instances={} mates={}",
                    session.assembly_id(),
                    session.instances().len(),
                    session.mates().len()
                );
                *asm = Some(session);
                true
            }
            Err(e) => {
                println!("failed: {e}");
                false
            }
        },
        "instances" => {
            let Some(session) = asm.as_ref() else {
                println!("failed: run assem:open first to load the assembly fixture");
                return false;
            };
            for info in session.instances() {
                let t = &info.transform;
                println!(
                    "instance {} part={} at ({:.4}, {:.4}, {:.4})",
                    info.id, info.source_id, t[12], t[13], t[14]
                );
            }
            true
        }
        "mates" => {
            let Some(session) = asm.as_ref() else {
                println!("failed: run assem:open first to load the assembly fixture");
                return false;
            };
            for (i, mate) in session.mates().iter().enumerate() {
                println!("mate {}", assem::describe_mate(i, mate));
            }
            true
        }
        "solve" => {
            let Some(session) = asm.as_mut() else {
                println!("failed: run assem:open first to load the assembly fixture");
                return false;
            };
            match session.solve() {
                SolveOutcome::Solved(report) => {
                    print_solve(&report);
                    true
                }
                SolveOutcome::Diverged(error) => {
                    println!("diverged: {}: {error}", assem::workbench_error_kind(&error));
                    true
                }
            }
        }
        "render" => {
            let Some(rest) = rest else {
                println!("failed: assem:render expects a PATH");
                return false;
            };
            let Some(session) = asm.as_ref() else {
                println!("failed: run assem:open first to load the assembly fixture");
                return false;
            };
            match session.render_ppm(rest) {
                Ok(r) => {
                    println!(
                        "rendered {} sha256={} {}x{}",
                        r.path, r.sha256, r.width, r.height
                    );
                    true
                }
                Err(e) => {
                    println!("failed: {e}");
                    false
                }
            }
        }
        "animate" => {
            let Some(rest) = rest else {
                println!("failed: assem:animate expects FRAMES:OUT_DIR");
                return false;
            };
            let mut parts = rest.splitn(2, ':');
            let frames: usize = match parts.next().and_then(|v| v.parse().ok()) {
                Some(f) => f,
                None => {
                    println!("failed: assem:animate FRAMES must be a positive integer");
                    return false;
                }
            };
            let out_dir = parts.next().unwrap_or_default();
            let Some(session) = asm.as_ref() else {
                println!("failed: run assem:open first to load the assembly fixture");
                return false;
            };
            match session.animate(frames, out_dir) {
                Ok(a) => {
                    println!(
                        "motion frames={} worst_residual={:.3e} worst_translation_mm={:.3e} \
                         worst_orientation_rad={:.3e} free_dof_undriven={} trace_sha256={} out={}",
                        a.frames,
                        a.worst_residual,
                        a.worst_translation_error,
                        a.worst_orientation_error,
                        a.free_dof_undriven,
                        a.trace_sha256,
                        a.out_dir
                    );
                    true
                }
                Err(e) => {
                    println!("failed: {e}");
                    false
                }
            }
        }
        "mate-add" | "mate-edit" | "mate-del" => {
            let Some(session) = asm.as_mut() else {
                println!("failed: run assem:open first to load the assembly fixture");
                return false;
            };
            let edit = sub != "mate-add";
            let delete = sub == "mate-del";
            let Some(rest) = rest else {
                println!("failed: {sub} expects arguments");
                return false;
            };
            if delete {
                let index: usize = match rest.parse() {
                    Ok(i) => i,
                    Err(_) => {
                        println!("failed: {sub} expects an integer INDEX");
                        return false;
                    }
                };
                return print_mate_outcome("removed", session.remove_mate(index));
            }
            let (index_part, kind_part_owned) = if edit {
                match rest.split_once(':') {
                    Some((i, k)) => (Some(i), k.to_string()),
                    None => {
                        println!("failed: {sub} expects INDEX:KIND:ARGS...");
                        return false;
                    }
                }
            } else {
                (None, rest.to_string())
            };
            let kind_args: Vec<&str> = kind_part_owned.split(':').collect();
            let kind = kind_args.first().copied().unwrap_or_default();
            let args = &kind_args[if kind_args.len() > 1 { 1 } else { 0 }..];
            if edit {
                let index: usize = match index_part.unwrap_or_default().parse() {
                    Ok(i) => i,
                    Err(_) => {
                        println!("failed: {sub} expects an integer INDEX");
                        return false;
                    }
                };
                let mate = match assem::parse_mate(kind, args) {
                    Ok(m) => m,
                    Err(e) => {
                        println!("failed: {e}");
                        return false;
                    }
                };
                print_mate_outcome("edited", session.edit_mate(index, mate))
            } else {
                let mate = match assem::parse_mate(kind, args) {
                    Ok(m) => m,
                    Err(e) => {
                        println!("failed: {e}");
                        return false;
                    }
                };
                print_mate_outcome("added", session.add_mate(mate))
            }
        }
        other => {
            println!(
                "failed: unknown assem sub-command '{other}'; see help:assem:open for the surface"
            );
            false
        }
    }
}

/// Print the outcome of a live-solve mate edit. A diverged solve reports the
/// typed error but is a valid session state (not a command failure).
fn print_mate_outcome(verb: &str, outcome: MateOutcome) -> bool {
    match outcome {
        MateOutcome::AppliedAndSolved { mate_index, report } => {
            println!(
                "{verb} mate {mate_index}: solved dof={} moved={} max_dist={:.3e} max_angle={:.3e}",
                report.dof,
                report.moved.len(),
                report.max_distance_error,
                report.max_angle_error
            );
            for (id, t) in &report.moved {
                println!("moved {id} to ({:.4}, {:.4}, {:.4})", t[12], t[13], t[14]);
            }
            true
        }
        MateOutcome::AppliedButDiverged { mate_index, error } => {
            println!(
                "diverged mate {mate_index}: {}: {error}",
                assem::workbench_error_kind(&error)
            );
            true
        }
        MateOutcome::Rejected { mate_index, error } => {
            println!("failed mate {mate_index}: {error}");
            false
        }
    }
}

/// Print a successful solve report.
fn print_solve(report: &oep_assem::SolveReport) {
    println!(
        "solved dof={} moved={} max_dist={:.3e} max_angle={:.3e}",
        report.dof,
        report.moved.len(),
        report.max_distance_error,
        report.max_angle_error
    );
    for (id, t) in &report.moved {
        println!("moved {id} to ({:.4}, {:.4}, {:.4})", t[12], t[13], t[14]);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        print!("{}", commands::render_index());
        std::process::exit(1);
    }

    let mut workbench = Workbench::with_local_vault();
    let mut asm: Option<AssemblySession> = None;
    let mut editor_session: Option<EditorSession> = None;
    let mut ok = true;

    for arg in &args {
        // Pure help-surface tokens: all generated from the command metadata.
        if arg == "help" || arg == "--help" || arg == "-h" {
            print!("{}", commands::render_index());
            continue;
        }
        if let Some(name) = arg.strip_prefix("help:") {
            if name == "--json" {
                println!("{}", commands::render_index_json());
            } else if let Some(doc) = commands::render_help(name) {
                print!("{doc}");
            } else {
                println!(
                    "failed: unknown command to document '{name}'; run 'help' for the full index"
                );
                ok = false;
            }
            continue;
        }
        if arg == "--markdown" {
            print!("{}", commands::render_index_markdown());
            continue;
        }
        if let Some(path) = arg.strip_prefix("--write-help-index:") {
            match std::fs::write(path, commands::render_index_markdown()) {
                Ok(()) => println!("wrote help index to {path}"),
                Err(e) => {
                    println!("failed: cannot write {path}: {e}");
                    ok = false;
                }
            }
            continue;
        }

        // Split on the FIRST ':' only; summaries may contain ':'.
        let (cmd, rest) = match arg.split_once(':') {
            Some((c, r)) => (c, Some(r)),
            None => (arg.as_str(), None),
        };
        // Validate the token against the metadata before dispatch so an
        // unknown command fails with an actionable message naming the token.
        if let Err(message) = commands::validate_token(cmd) {
            println!("failed: {message}");
            ok = false;
            continue;
        }
        match (cmd, rest) {
            ("load", Some(arg)) => {
                let fmt = arg.split(':').next().unwrap_or_default();
                let path = arg.get(fmt.len().saturating_add(1)..).unwrap_or_default();
                let format = match fmt {
                    "cgr" => Interchange::Cgr,
                    "3dxml" => Interchange::ThreeDXml,
                    "step" => Interchange::Step,
                    other => {
                        println!(
                            "failed: unknown load format '{other}'; expected cgr, 3dxml or step"
                        );
                        ok = false;
                        continue;
                    }
                };
                let bytes = match std::fs::read(path) {
                    Ok(b) => b,
                    Err(e) => {
                        println!("failed: cannot read {path}: {e}");
                        ok = false;
                        continue;
                    }
                };
                let out = workbench.run(WorkbenchAction::Load { format, bytes });
                ok &= print_outcome(&out);
            }
            ("save-pdm", Some(arg)) => {
                let mut parts = arg.splitn(2, ':');
                let item = parts.next().unwrap_or_default().to_string();
                let rest = parts.next().unwrap_or_default();
                let (revision, summary) = rest
                    .split_once(':')
                    .map(|(r, s)| (r.to_string(), s.to_string()))
                    .unwrap_or((rest.to_string(), String::new()));
                let out = workbench.run(WorkbenchAction::SaveToPdm {
                    item_id: item,
                    revision,
                    summary,
                });
                ok &= print_outcome(&out);
            }
            ("save", Some(format)) => {
                let out = workbench.run(WorkbenchAction::Save {
                    format: match format {
                        "cgr" => Interchange::Cgr,
                        "3dxml" => Interchange::ThreeDXml,
                        "step" => Interchange::Step,
                        other => {
                            println!(
                                "failed: unknown save format '{other}'; expected cgr, 3dxml or step"
                            );
                            ok = false;
                            continue;
                        }
                    },
                });
                ok &= print_outcome(&out);
            }
            ("open-pdm", Some(arg)) => {
                let mut parts = arg.splitn(2, ':');
                let item = parts.next().unwrap_or_default().to_string();
                let revision = parts.next().unwrap_or_default().to_string();
                let out = workbench.run(WorkbenchAction::OpenFromPdm {
                    item_id: item,
                    revision,
                });
                ok &= print_outcome(&out);
            }
            ("assem", Some(arg)) => {
                ok &= dispatch_assem(&mut asm, arg);
            }
            (
                "editor" | "sk" | "vars" | "feat" | "view" | "export" | "props" | "undo" | "redo",
                arg,
            ) => {
                ok &= dispatch_editor(&mut editor_session, cmd, arg);
            }
            ("journal", None) => {
                for line in workbench.journal() {
                    println!("{line}");
                }
            }
            ("search", rest) => {
                let query = rest.unwrap_or_default();
                let hits = commands::search(query);
                if hits.is_empty() {
                    println!("search: no command matches '{query}'; run 'help' for the full index");
                } else {
                    for c in &hits {
                        println!("  {:<32} {}", c.usage, c.summary);
                    }
                    println!(
                        "search: {} of {} commands match",
                        hits.len(),
                        commands::commands().len()
                    );
                }
            }
            (other, _) => {
                // Unreachable for unknown tokens (validated above); kept so a
                // known token can never silently fall through as success.
                println!("failed: unsupported command '{other}'; run 'help' for the full index");
                ok = false;
            }
        }
    }

    std::process::exit(if ok { 0 } else { 1 });
}
