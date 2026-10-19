//! P2-M5-T01 — CLI `oep-cgr2gltf`: convert a real CGR file to glTF/GLB.
//!
//! Lives inside the `oep-cgr` crate so it can call the crate's clean-room
//! extractor without a cross-crate path dependency (layout.md convention: no
//! cross-crate path deps until the P6 importer hub).
//!
//! Pipeline: read input -> `oep_cgr::mesh::extract` (validates the CGR header
//! spine AND latches a structurally-verified triangle mesh, or a classified
//! skip) -> write GLB (default) / glTF (JSON + BIN). Never panics; every
//! outcome maps to a documented exit code.
//!
//! ## Exit code contract (documented in `docs/re-notes/oep-cgr2gltf-cli.md`
//! and repeated in `--help`):
//!
//! ```text
//!   0  success (output written)
//!   1  usage error
//!   2  input I/O error
//!   3  not a CGR container (header spine mismatch)
//!   4  classified skip (valid CGR, no usable tessellation found)
//!   5  output I/O error
//!   6  internal error (extractor returned an invalid mesh)
//! ```
//!
//! Scope note: the crate-level glTF/GLB writer is P2-M2-T03 (a running
//! sibling card). This CLI embeds a minimal self-contained GLB writer
//! (`write_glb`) so the CLI is independently verifiable now; the seam to swap
//! in `oep_cgr::gltf` when that card merges is one function call.

use std::io::Write;
use std::process::ExitCode;

use oep_cgr::mesh::{extract, Mesh};

/// Exit codes of the CLI contract (documented in --help + re-notes).
mod exit_codes {
    pub const OK: u8 = 0;
    pub const USAGE: u8 = 1;
    pub const INPUT_IO: u8 = 2;
    pub const NOT_CGR: u8 = 3;
    pub const CLASSIFIED_SKIP: u8 = 4;
    pub const OUTPUT_IO: u8 = 5;
    pub const INTERNAL: u8 = 6;
}

const HELP: &str = "\
oep-cgr2gltf — convert a CATIA Graphics Representation (.cgr) file to glTF/GLB.

USAGE:
    oep-cgr2gltf [OPTIONS] <INPUT.cgr>

OPTIONS:
    -o, --output <PATH>   Output file. Extension .glb (default) or .gltf.
    -f, --format <FMT>    Force output format: 'glb' or 'gltf'
                          (default: inferred from output extension, else glb).
    -v, --verbose         Print extraction details to stderr.
    --version             Print version and exit.
    -h, --help            Print this help and exit.

EXIT CODES:
    0  success (output written)
    1  usage error (bad or unknown arguments)
    2  input I/O error (file missing or unreadable)
    3  not a CGR container (header spine mismatch)
    4  classified skip (valid CGR but no usable tessellation found)
    5  output I/O error (cannot write output file)
    6  internal error (extractor produced an invalid mesh)
";

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        // All error paths already returned an exit code via Err(buffer);
        // this arm is dead by construction, but keep the fallback explicit.
        Err((code, msg)) => {
            eprintln!("oep-cgr2gltf: {msg}");
            ExitCode::from(code)
        }
    }
}

type CliResult = Result<u8, (u8, String)>;

fn run() -> CliResult {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (opts, positionals) = parse_args(&args)?;

    if opts.help {
        print!("{HELP}");
        return Ok(exit_codes::OK);
    }
    if opts.version {
        println!("oep-cgr2gltf {}", env!("CARGO_PKG_VERSION"));
        return Ok(exit_codes::OK);
    }
    if positionals.len() != 1 {
        return Err((
            exit_codes::USAGE,
            "expected exactly one INPUT.cgr (see --help)".into(),
        ));
    }

    let input = &positionals[0];
    let bytes = std::fs::read(input)
        .map_err(|e| (exit_codes::INPUT_IO, format!("cannot read '{input}': {e}")))?;

    let mesh: Mesh = match extract(&bytes) {
        Ok(m) => {
            if let Err(internal) = m.validate() {
                return Err((
                    exit_codes::INTERNAL,
                    format!("extractor returned an invalid mesh: {internal}"),
                ));
            }
            if opts.verbose {
                eprintln!(
                    "extracted mesh: {} vertices, {} triangles",
                    m.vertices().len(),
                    m.triangles().len()
                );
            }
            m
        }
        Err(e) => match &e {
            oep_cgr::mesh::MeshError::NotCgrPreamble => {
                return Err((exit_codes::NOT_CGR, format!("not a CGR container: {e}")))
            }
            _ => return Err((exit_codes::CLASSIFIED_SKIP, format!("{e}"))),
        },
    };
    if mesh.triangles().is_empty() {
        return Err((
            exit_codes::CLASSIFIED_SKIP,
            "classified skip: mesh has no triangles".into(),
        ));
    }

    let output = match &opts.output {
        Some(o) => o.clone(),
        None => {
            let stem = input.trim_end_matches(".cgr").trim_end_matches(".CGR");
            format!("{stem}.glb")
        }
    };
    let fmt = match &opts.format {
        Some(f) => f.as_str(),
        None if output.ends_with(".gltf") => "gltf",
        _ => "glb",
    };

    let written: Vec<u8> = match fmt {
        "glb" => write_glb(&mesh),
        "gltf" => write_gltf_json(&mesh),
        other => {
            return Err((
                exit_codes::USAGE,
                format!("unknown output format '{other}' (use glb or gltf)"),
            ))
        }
    };

    let mut file = std::fs::File::create(&output).map_err(|e| {
        (
            exit_codes::OUTPUT_IO,
            format!("cannot create '{output}': {e}"),
        )
    })?;
    file.write_all(&written).map_err(|e| {
        (
            exit_codes::OUTPUT_IO,
            format!("cannot write '{output}': {e}"),
        )
    })?;

    if opts.verbose {
        eprintln!("wrote {} bytes to {output}", written.len());
    }
    Ok(exit_codes::OK)
}

#[derive(Default)]
struct Opts {
    output: Option<String>,
    format: Option<String>,
    verbose: bool,
    help: bool,
    version: bool,
}

fn parse_args(args: &[String]) -> Result<(Opts, Vec<String>), (u8, String)> {
    let mut opts = Opts::default();
    let mut positionals = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-o" | "--output" => {
                let v = iter
                    .next()
                    .ok_or((exit_codes::USAGE, format!("{arg} requires a value")))?;
                opts.output = Some(v.clone());
            }
            "-f" | "--format" => {
                let v = iter
                    .next()
                    .ok_or((exit_codes::USAGE, format!("{arg} requires a value")))?;
                opts.format = Some(v.clone());
            }
            "-v" | "--verbose" => opts.verbose = true,
            "-h" | "--help" => opts.help = true,
            "--version" => opts.version = true,
            s if s.starts_with('-') => {
                return Err((exit_codes::USAGE, format!("unknown option '{s}'")))
            }
            p => positionals.push(p.to_string()),
        }
    }
    Ok((opts, positionals))
}

/// Build the BIN chunk (positions + indices) shared by GLB and .gltf.
fn bin_chunk(mesh: &Mesh) -> Vec<u8> {
    let mut bin = Vec::new();
    // Vertex positions: VEC3 float32 (12 bytes each).
    for v in mesh.vertices() {
        bin.extend_from_slice(&v[0].to_le_bytes());
        bin.extend_from_slice(&v[1].to_le_bytes());
        bin.extend_from_slice(&v[2].to_le_bytes());
    }
    // Triangle indices: SCALAR u32 (3 per triangle).
    for t in mesh.triangles() {
        for &i in t {
            bin.extend_from_slice(&i.to_le_bytes());
        }
    }
    bin
}

/// Minimal, self-contained binary glTF (GLB) writer. Emits a valid GLB 2.0
/// container: header + JSON chunk (rounded to 4) + BIN chunk (rounded to 4).
/// Positions are VEC3 f32, indices SCALAR u32 (glTF component type 5125).
fn write_glb(mesh: &Mesh) -> Vec<u8> {
    let bin = bin_chunk(mesh);
    let min = bounding_box(mesh);
    let accessor_stride = 12u32; // VEC3 f32
    let idx_count = (mesh.triangles().len() * 3) as u32;
    // JSON with 4-byte alignment for the BIN pointer.
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":1}},"indices":2,"mode":4}}]}}],"buffers":[{{"byteLength":{b}}}]"#,
        b = bin.len()
    );
    let json = format!(
        "{json},\"bufferViews\":[{{\"buffer\":0,\"byteOffset\":0,\"byteLength\":{b},\"target\":34962}}],\
\"accessors\":[{{\"bufferView\":0,\"byteOffset\":0,\"componentType\":5126,\"count\":{vc},\"type\":\"VEC3\",\"min\":[{x0},{y0},{z0}],\"max\":[{x1},{y1},{z1}]}},\
{{\"bufferView\":0,\"byteOffset\":{vo},\"componentType\":5125,\"count\":{ic},\"type\":\"SCALAR\"}}]}}",
        b = bin.len(),
        vc = mesh.vertices().len(),
        ic = idx_count,
        vo = mesh.vertices().len() * accessor_stride as usize,
        x0 = min.0[0], y0 = min.0[1], z0 = min.0[2],
        x1 = min.1[0], y1 = min.1[1], z1 = min.1[2],
    );

    let mut glb = Vec::new();
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes()); // version
                                                // Length placeholder, patched below once padding is known.
    glb.extend_from_slice(&0u32.to_le_bytes());
    // JSON chunk (padded to 4).
    glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x4E4F534Au32.to_le_bytes()); // 'JSON'
    glb.extend_from_slice(json.as_bytes());
    while glb.len() % 4 != 0 {
        glb.push(0x20);
    }
    // BIN chunk (padded to 4).
    glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004E4942u32.to_le_bytes()); // 'BIN\0'
    glb.extend_from_slice(&bin);
    while glb.len() % 4 != 0 {
        glb.push(0x00);
    }
    // Patch the total length in the 12-byte header (offset 8..12).
    let total = glb.len() as u32;
    glb[8..12].copy_from_slice(&total.to_le_bytes());
    glb
}

/// Minimal .gltf (JSON + embedded data URI BIN). Statistically of limited use
/// for large meshes, but valid and handy for the 11-sample corpus.
fn write_gltf_json(mesh: &Mesh) -> Vec<u8> {
    let bin = bin_chunk(mesh);
    let min = bounding_box(mesh);
    let b64 = base64_encode(&bin);
    let vo = mesh.vertices().len() * 12;
    let json = format!(
        r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],"meshes":[{{"primitives":[{{"attributes":{{"POSITION":1}},"indices":2,"mode":4}}]}}],"buffers":[{{"uri":"data:application/octet-stream;base64,{b64}","byteLength":{b}}}],"bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":{b},"target":34962}}],"accessors":[{{"bufferView":0,"byteOffset":0,"componentType":5126,"count":{vc},"type":"VEC3","min":[{x0},{y0},{z0}],"max":[{x1},{y1},{z1}]}},{{"bufferView":0,"byteOffset":{vo},"componentType":5125,"count":{ic},"type":"SCALAR"}}]}}"#,
        b = bin.len(),
        vc = mesh.vertices().len(),
        ic = mesh.triangles().len() * 3,
        vo = vo,
        x0 = min.0[0],
        y0 = min.0[1],
        z0 = min.0[2],
        x1 = min.1[0],
        y1 = min.1[1],
        z1 = min.1[2],
    );
    json.into_bytes()
}

fn bounding_box(mesh: &Mesh) -> ([f32; 3], [f32; 3]) {
    let mut mn = [f32::INFINITY; 3];
    let mut mx = [f32::NEG_INFINITY; 3];
    for v in mesh.vertices() {
        for k in 0..3 {
            mn[k] = mn[k].min(v[k]);
            mx[k] = mx[k].max(v[k]);
        }
    }
    if !mn[0].is_finite() {
        mn = [0.0; 3];
        mx = [0.0; 3];
    }
    (mn, mx)
}

/// Extremely small std-only base64 encoder (the crate has no dependencies).
/// Standard RFC 4648 alphabet.
const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let mut n = (chunk[0] as u32) << 16;
        if chunk.len() > 1 {
            n |= (chunk[1] as u32) << 8;
        }
        if chunk.len() > 2 {
            n |= chunk[2] as u32;
        }
        out.push(B64[(n >> 18) as usize & 63] as char);
        out.push(B64[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            B64[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            B64[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}
