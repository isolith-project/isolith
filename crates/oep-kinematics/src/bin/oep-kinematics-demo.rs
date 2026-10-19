//! P7-M3-T02 demo binary: solve and record the crank-slider mechanism.
//!
//! Usage: `oep-kinematics-demo [outdir] [frames]`
//!   - `outdir`   output directory (default `demos/p7-m3-t02`)
//!   - `frames`   number of animation frames (default 90, one crank turn)
//!
//! Produces:
//!   `outdir/trace.json`       deterministic per-frame kinematic trace
//!   `outdir/manifest.json`    gates + provenance summary
//!   `outdir/frames/*.ppm`     rendered frames (own software renderer, PPM P6)
//!
//! The PPM frames are packaged into a playable GIF/MP4 by the recording
//! script `scripts/p7-m3-t02-record.sh` (ffmpeg on the recording host; not a
//! crate dependency).

use oep_kinematics::demo::{self, DemoResult};

fn parse<T: std::str::FromStr>(name: &str, s: &Option<String>, default: T) -> T {
    match s {
        Some(v) => v.parse().unwrap_or_else(|_| {
            eprintln!("ignoring invalid {name}={v:?}; using default");
            default
        }),
        None => default,
    }
}

fn relativize(path: &str) -> String {
    // Keep recorded paths stable and hermetic: already-relative paths pass
    // through unchanged; absolute paths are expressed relative to the process
    // CWD when they sit under it, else reduced to the leaf.
    let p = std::path::Path::new(path);
    if !p.is_absolute() {
        return path.to_string();
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    match p.strip_prefix(&cwd) {
        Ok(rel) => rel.display().to_string(),
        Err(_) => p
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string()),
    }
}

fn write_manifest(
    result: &DemoResult,
    outdir: &str,
    trace_path: &str,
    frames_dir: &str,
    count: usize,
) -> std::io::Result<()> {
    let gates = result.gates();
    let trace = relativize(trace_path);
    let frames = relativize(frames_dir);
    let mut b = String::new();
    b.push_str("{\n");
    b.push_str("  \"program\": \"P7-M3-T02\",\n");
    b.push_str("  \"task\": \"mates-kinematics-demo\",\n");
    b.push_str(
        "  \"cleanroom\": \"own implementation; no vendor libraries (see tests/cleanroom.rs)\",\n",
    );
    b.push_str(&format!("  \"frames\": {},\n", result.frames.len()));
    b.push_str(&format!(
        "  \"worst_residual_max\": {:.3e},\n",
        gates.worst_residual
    ));
    b.push_str(&format!(
        "  \"worst_translation_error_mm\": {:.3e},\n",
        gates.worst_translation_error
    ));
    b.push_str(&format!(
        "  \"worst_orientation_error_rad\": {:.3e},\n",
        gates.worst_orientation_error
    ));
    b.push_str(&format!(
        "  \"free_dof_undriven\": {},\n",
        gates.free_dof_undriven
    ));
    b.push_str(&format!("  \"trace\": \"{trace}\",\n"));
    b.push_str(&format!("  \"frame_dir\": \"{frames}\",\n"));
    b.push_str(&format!("  \"ppm_frame_count\": {count}\n",));
    b.push_str("}\n");
    std::fs::write(std::path::Path::new(outdir).join("manifest.json"), b)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let outdir = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "demos/p7-m3-t02".to_string());
    let frames: usize = parse("frames", &args.get(2).cloned(), 90_usize);

    println!("P7-M3-T02 mates/kinematics demo: crank-slider fixture");
    println!("  outdir={outdir} frames={frames}");

    let result = demo::run_demo(frames);

    let trace_path = format!("{outdir}/trace.json");
    let frames_dir = format!("{outdir}/frames");
    std::fs::create_dir_all(&outdir).expect("create outdir");
    demo::write_trace(&result, &result.frames, &trace_path).expect("write trace");
    let rendered =
        demo::render_frames(&result.mechanism, &result.frames, &frames_dir).expect("render");
    write_manifest(&result, &outdir, &trace_path, &frames_dir, rendered.len())
        .expect("write manifest");

    println!("  {}", demo::summarize(&result));
    println!(
        "  wrote {trace_path} + {} PPM frames -> {frames_dir}",
        rendered.len()
    );
    let text = std::fs::read_to_string(&trace_path).expect("read trace back");
    assert!(
        text.starts_with('{') && text.ends_with("}\n"),
        "trace is a JSON object"
    );
    println!("  trace re-read ok ({} bytes)", text.len());
}
