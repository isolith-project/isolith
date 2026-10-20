//! `oep-cgr2step` — tessellated mesh -> STEP B-Rep passthrough CLI (P2-M5-T02).
//!
//! ```
//! oep-cgr2step INPUT.cgr OUTPUT.step
//! ```
//!
//! Extracts the tessellated triangle mesh from a CGR file
//! (`oep_cgr::mesh::extract`) and writes it as a faceted STEP B-Rep that is
//! explicitly **marked as tessellated and NOT claimed as exact** (see
//! `oep_cgr::step` and `docs/cgr-to-step.md`).
//!
//! ## Exit codes (documented contract)
//!
//! | Code | Meaning |
//! |------|---------|
//! | 0    | success — STEP written |
//! | 1    | I/O error (input unreadable or output unwritable) |
//! | 2    | usage error |
//! | 3    | input is not a recognized CGR (header spine mismatch) |
//! | 4    | classified skip — no usable tessellated mesh in the payload |
//! | 5    | internal mesh-geometry invariant failure (defensive) |
//!
//! On success a short summary and the tessellation honesty warning are printed
//! to stderr; stdout stays clean for scripting.

use std::io::Write;
use std::process::ExitCode;

use oep_cgr::mesh::{extract, MeshError};
use oep_cgr::step::{write_step, StepError, StepOpts};

const EXIT_OK: u8 = 0;
const EXIT_IO: u8 = 1;
const EXIT_USAGE: u8 = 2;
const EXIT_NOT_CGR: u8 = 3;
const EXIT_SKIP: u8 = 4;
const EXIT_INTERNAL: u8 = 5;

const USAGE: &str = "\
oep-cgr2step - tessellated mesh -> STEP B-Rep passthrough (marked tessellated, NOT exact)

USAGE:
    oep-cgr2step INPUT.cgr OUTPUT.step

ARGS:
    INPUT.cgr    CGR file whose tessellated mesh is extracted and exported
    OUTPUT.step  STEP (ISO 10303-21) file to write

EXIT CODES:
    0  success
    1  I/O error
    2  usage error
    3  input is not a recognized CGR
    4  classified skip (no usable tessellated mesh)
    5  internal mesh-geometry invariant failure

NOTE: the output is a TESSELATED faceted B-Rep derived from the CGR
tessellation. It is NOT an exact analytic B-Rep; treat downstream geometry,
tolerance and CAM operations accordingly.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print!("{USAGE}");
        return ExitCode::from(EXIT_OK);
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("oep-cgr2step {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::from(EXIT_OK);
    }
    if args.len() != 2 {
        eprint!("{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    }
    run(&args[0], &args[1])
}

fn run(input: &str, output: &str) -> ExitCode {
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("oep-cgr2step: cannot read '{input}': {e}");
            return ExitCode::from(EXIT_IO);
        }
    };

    let mesh = match extract(&bytes) {
        Ok(m) => m,
        Err(MeshError::NotCgrPreamble) => {
            eprintln!(
                "oep-cgr2step: '{input}' is not a recognized CGR (header spine mismatch); no STEP written"
            );
            return ExitCode::from(EXIT_NOT_CGR);
        }
        Err(skip) => {
            eprintln!("oep-cgr2step: classified skip on '{input}': {skip}; no STEP written");
            return ExitCode::from(EXIT_SKIP);
        }
    };

    if mesh.triangles().is_empty() {
        eprintln!(
            "oep-cgr2step: classified skip on '{input}': mesh has no triangles; no STEP written"
        );
        return ExitCode::from(EXIT_SKIP);
    }

    let stem = std::path::Path::new(input)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| input.to_string());
    let out_leaf = std::path::Path::new(output)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| output.to_string());

    let opts = StepOpts {
        product_name: &stem,
        source_name: &out_leaf,
        timestamp: "",
    };

    let file = match std::fs::File::create(output) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("oep-cgr2step: cannot create '{output}': {e}");
            return ExitCode::from(EXIT_IO);
        }
    };
    let mut out = std::io::BufWriter::new(file);

    match write_step(&mesh, &opts, &mut out) {
        Ok(stats) => {
            let _ = out.flush();
            eprintln!(
                "oep-cgr2step: wrote '{output}': {} vertices, {} triangles -> {} facets \
                 ({} closed / {} open shell{}, {} degenerate skipped)",
                mesh.vertices().len(),
                mesh.triangles().len(),
                stats.faces,
                stats.closed_shells,
                stats.open_shells,
                if stats.shells == 1 { "" } else { "s" },
                stats.degenerate_skipped,
            );
            eprintln!(
                "oep-cgr2step: WARNING: output is a TESSELLATED faceted B-Rep passthrough \
                 from the CGR tessellation and is NOT an exact analytic B-Rep. \
                 Treat downstream geometry/tolerance results accordingly."
            );
            ExitCode::from(EXIT_OK)
        }
        Err(StepError::EmptyMesh) => {
            let _ = std::fs::remove_file(output);
            eprintln!(
                "oep-cgr2step: classified skip on '{input}': no usable (non-degenerate) \
                 triangles; no STEP written"
            );
            ExitCode::from(EXIT_SKIP)
        }
        Err(StepError::Io(e)) => {
            let _ = std::fs::remove_file(output);
            eprintln!("oep-cgr2step: I/O error writing '{output}': {e}");
            ExitCode::from(EXIT_IO)
        }
        Err(other) => {
            let _ = std::fs::remove_file(output);
            eprintln!("oep-cgr2step: internal error exporting '{input}': {other}");
            ExitCode::from(EXIT_INTERNAL)
        }
    }
}
