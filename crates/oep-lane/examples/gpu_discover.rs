//! P9-M4-T01 -- live GPU discovery on real hardware.
//!
//! Shells out to `nvidia-smi --query-gpu=... --format=csv,noheader`, feeds
//! the output through [`oep_lane::gpu::parse_smi_csv`] and prints what was
//! discovered. Run it on a dgx/gpu node to re-measure discovery honestly:
//!
//! ```text
//! cargo run -p oep-lane --example gpu_discover
//! ```
//!
//! On a host with no nvidia-smi it reports zero devices (a CPU-only node)
//! rather than failing, matching the "discovery never wedges the lane" rule.
//! The recorded reference-host measurement lives in `P9-M4/spike/run-log.json`.

use oep_lane::gpu::{parse_smi_csv, GpuProbe, NodeResources, StaticProbe};

fn main() {
    // 1. Live probe if nvidia-smi exists.
    let live = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=index,name,uuid,driver_version,compute_cap,memory.total",
            "--format=csv,noheader",
        ])
        .output();

    match live {
        Ok(out) if out.status.success() => {
            let csv = String::from_utf8_lossy(&out.stdout);
            match parse_smi_csv(&csv) {
                Ok(devices) => {
                    println!(
                        "live nvidia-smi probe: {} device(s) discovered",
                        devices.len()
                    );
                    for d in &devices {
                        println!("  {d}");
                    }
                    if devices.is_empty() {
                        println!("  (host reports no GPU -- CPU-only node)");
                    }
                }
                Err(e) => println!("nvidia-smi output did not parse: {e}\nraw:\n{csv}"),
            }
        }
        _ => println!("nvidia-smi not available on this host: reporting CPU-only node"),
    }

    // 2. Deterministic reference (regression belt): the fixture must still
    //    parse to the GB10 device recorded in run-log.json.
    let gb10 =
        "0, NVIDIA GB10, GPU-331f26d3-21e3-86c0-3574-fa1c79e58297, 580.178.04, 12.1, [N/A]\n";
    let parsed = parse_smi_csv(gb10).expect("reference fixture parses");
    println!("\nreference fixture: {} device(s)", parsed.len());
    for d in &parsed {
        println!("  {d}");
    }

    // 3. NodeResources summary from the static probe, i.e. what a registration
    //    looks like when it is handed to the scheduler pool.
    for n in StaticProbe::new(vec![NodeResources::cpu_only("cpu-only-01", 16 * 1024)]).probe() {
        println!("\nnode {}: cpu-only, {} gpu(s)", n.node_id, n.gpus.len());
    }
}
