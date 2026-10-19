//! OEP assembly workbench command-line surface.
//!
//! Solve an oep-geo model + constraint set through the workbench and write
//! the solved document back out. This is a thin headless binding so the
//! "assemble and constrain through the workbench" acceptance is demonstrable
//! without a GUI shell (the P7 shell workbench surface lands with P7-M2).
//!
//! Usage:
//!   oep-assem solve --model <model.oepgeo.json> --constraints <cset.json>
//!       [--moving i1,i2] [--assembly <id>] [--out <solved.oepgeo.json>]

use std::fs;
use std::process::ExitCode;

use oep_assem::workbench;

fn usage() -> ! {
    eprintln!(
        "oep-assem solve --model <m> --constraints <c> [--assembly <id>] [--moving a,b] [--out <o>]"
    );
    std::process::exit(2);
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("solve") {
        usage();
    }
    let model = match arg(&args, "--model") {
        Some(m) => m,
        None => usage(),
    };
    let constraints_path = match arg(&args, "--constraints") {
        Some(c) => c,
        None => usage(),
    };
    let out = arg(&args, "--out");
    let moving: Vec<String> = arg(&args, "--moving")
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let model_text = match fs::read_to_string(&model) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error reading model: {e}");
            return ExitCode::FAILURE;
        }
    };
    let cset_text = match fs::read_to_string(&constraints_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error reading constraints: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut doc = match workbench::load_document(&model_text) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("error loading model: {e}");
            return ExitCode::FAILURE;
        }
    };
    let cset = match oep_assem::ConstraintSet::from_json_pretty(&cset_text) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error loading constraints: {e}");
            return ExitCode::FAILURE;
        }
    };

    let req = workbench::SolveRequest::new(cset.assembly.clone(), cset.mates.clone(), moving);
    match workbench::solve(&mut doc, &req) {
        Ok(report) => {
            println!(
                "solve status: {:?}  dof={}  max_dist_err={:.3e}  max_angle_err={:.3e}",
                report.status, report.dof, report.max_distance_error, report.max_angle_error
            );
            for (id, t) in &report.moved {
                println!("moved {id}: t=({:.6}, {:.6}, {:.6})", t[12], t[13], t[14]);
            }
            for r in &report.residuals {
                println!(
                    "  residual[{}] {:?} = {:.3e}",
                    r.mate_index, r.kind, r.value
                );
            }
            if let Some(path) = out {
                let json = workbench::save_document(&doc)
                    .map_err(|e| {
                        eprintln!("error serializing: {e}");
                        std::process::exit(1)
                    })
                    .unwrap();
                fs::write(&path, json)
                    .map_err(|e| {
                        eprintln!("error writing: {e}");
                        std::process::exit(1)
                    })
                    .unwrap();
                println!("wrote {path}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("solve failed: {e}");
            ExitCode::FAILURE
        }
    }
}
