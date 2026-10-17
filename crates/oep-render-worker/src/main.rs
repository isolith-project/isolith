//! CLI binary: `oep-render-worker`.
//!
//! Headless oep-geo render worker. Renders a real `oep-geo` model document
//! (JSON) to a PNG image offscreen, with parameterized resolution and
//! fidelity, and registers with the P9 scheduler under
//! `render/oep-geo-render`.
//!
//! Usage:
//! ```text
//!   oep-render-worker --model <file.oepgeo.json> --output <out.png>
//!                     [--resolution WxH] [--fidelity low|medium|high]
//!   oep-render-worker --describe
//! ```
//!
//! The `--resolution` / `--fidelity` flags mirror the P9-M1 job spec's
//! `executor.args` for the `render` kind (crates/oep-job), so a job that
//! says `args: ["--resolution","1920x1080","--fidelity","high"]` maps
//! directly onto this binary. Exit 0 = produced the artifact.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

use oep_geo::Document;

use oep_render_worker::options::{Fidelity, RenderOptions, Resolution};
use oep_render_worker::registry::{registration_json, RENDER_WORKER};
use oep_render_worker::worker::render_document;
use oep_viewer::render::encode_png;

struct Args {
    describe: bool,
    model: Option<PathBuf>,
    output: Option<PathBuf>,
    resolution: Resolution,
    fidelity: Fidelity,
}

fn usage_and_exit() -> ! {
    eprintln!(
        "usage: oep-render-worker --model <file.oepgeo.json> --output <out.png> \
         [--resolution WxH] [--fidelity low|medium|high]\n       \
         oep-render-worker --describe"
    );
    process::exit(2);
}

fn parse_args() -> Args {
    let argv: Vec<String> = env::args().skip(1).collect();
    let mut a = Args {
        describe: false,
        model: None,
        output: None,
        resolution: Resolution::default(),
        fidelity: Fidelity::default(),
    };
    let mut i = 0;
    while i < argv.len() {
        let flag = argv[i].as_str();
        let mut next = || -> Option<&String> {
            i += 1;
            argv.get(i)
        };
        let mut next_val = |what: &str| -> &String {
            next().unwrap_or_else(|| {
                eprintln!("{what} needs a value");
                usage_and_exit();
            })
        };
        match flag {
            "--describe" => a.describe = true,
            "--model" => a.model = Some(PathBuf::from(next_val("--model"))),
            "--output" => a.output = Some(PathBuf::from(next_val("--output"))),
            "--resolution" => {
                let v = next_val("--resolution");
                a.resolution = match v.parse::<Resolution>() {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("bad --resolution: {e}");
                        usage_and_exit();
                    }
                };
            }
            "--fidelity" => {
                let v = next_val("--fidelity");
                a.fidelity = match v.parse::<Fidelity>() {
                    Ok(f) => f,
                    Err(e) => {
                        eprintln!("bad --fidelity: {e}");
                        usage_and_exit();
                    }
                };
            }
            unknown => {
                eprintln!("unknown flag: {unknown}");
                usage_and_exit();
            }
        }
        i += 1;
    }
    a
}

fn main() {
    let args = parse_args();

    if args.describe {
        println!("{}", registration_json(&RENDER_WORKER));
        return;
    }

    let model = args.model.unwrap_or_else(|| {
        eprintln!("--model is required (unless --describe)");
        usage_and_exit();
    });
    let output = args.output.unwrap_or_else(|| {
        eprintln!("--output is required (unless --describe)");
        usage_and_exit();
    });

    let text = fs::read_to_string(&model).unwrap_or_else(|e| {
        eprintln!("cannot read model {}: {e}", model.display());
        process::exit(3);
    });
    let doc = Document::from_json_pretty(&text).unwrap_or_else(|e| {
        eprintln!("cannot parse oep-geo document {}: {e}", model.display());
        process::exit(3);
    });

    let opts = RenderOptions::new(args.resolution, args.fidelity);
    let result = render_document(&doc, opts);
    let prims = oep_viewer::scene::build_scene(&doc).primitives.len();

    let png = encode_png(&result.image);
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|e| {
            eprintln!("cannot create output dir {}: {e}", parent.display());
            process::exit(3);
        });
    }
    fs::write(&output, &png).unwrap_or_else(|e| {
        eprintln!("cannot write {}: {e}", output.display());
        process::exit(3);
    });

    println!(
        "oep-geo rendered {}x{} fidelity={} -> {} ({} bytes, {} primitives)",
        result.image.width,
        result.image.height,
        match result.fidelity {
            Fidelity::Low => "low",
            Fidelity::Medium => "medium",
            Fidelity::High => "high",
        },
        output.display(),
        png.len(),
        prims
    );
}
