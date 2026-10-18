//! oep-opsdash -- Ops dashboard for the P9 compute lane.
//!
//! Reads live scheduler state (real `oep-lane` Scheduler, never mocked) and
//! presents it three ways from one binary:
//!
//! ```text
//!   oep-opsdash serve --listen 127.0.0.1:8080 [--live]
//!       serve the JSON + HTML dashboard over std HTTP
//!   oep-opsdash snapshot [--out snapshot.json] [--lease-ms N]
//!       run the deterministic proof wave, dump a real snapshot JSON
//!   oep-opsdash render [--out dashboard.html] [--lease-ms N]
//!       run the deterministic proof wave, render the HTML dashboard
//! ```
//!
//! `--live` spawns a background driver so a served dashboard visibly moves;
//! without it the server shows whatever state exists (live, but static until
//! jobs are submitted). The default node fleet is U1 + the NAS.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex};

use oep_opsdash::clock::{Clock, ManualClock, SystemClock};
use oep_opsdash::dashboard::Dashboard;
use oep_opsdash::demo;
use oep_opsdash::http;

fn usage() -> ! {
    eprintln!(
        "usage:\n  oep-opsdash serve --listen ADDR[:PORT] [--live]\n  \
         oep-opsdash snapshot [--out PATH] [--lease-ms N]\n  \
         oep-opsdash render [--out PATH] [--lease-ms N]"
    );
    process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    match args[0].as_str() {
        "serve" => cmd_serve(&args[1..]),
        "snapshot" => cmd_snapshot(&args[1..]),
        "render" => cmd_render(&args[1..]),
        _ => usage(),
    }
}

fn parse_flag(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn lease_ms(args: &[String]) -> u64 {
    parse_flag(args, "--lease-ms")
        .and_then(|s| s.parse().ok())
        .unwrap_or(50)
}

/// Build the deterministic proof dashboard: a real ManualClock shared with the
/// real Scheduler-backed dashboard, then run the real wave against it.
fn proof_dashboard(lease_ms: u64) -> (Arc<Mutex<Dashboard>>, Arc<ManualClock>) {
    let manual = Arc::new(ManualClock::new(0));
    let clock: Arc<dyn Clock> = manual.clone();
    let mut dash = Dashboard::with_lease(clock, demo::fleet(), lease_ms);
    demo::run_wave(&mut dash, &manual);
    (Arc::new(Mutex::new(dash)), manual)
}

fn cmd_snapshot(args: &[String]) {
    let (shared, _clock) = proof_dashboard(lease_ms(args));
    let snap = shared.lock().expect("lock").snapshot();
    let out = parse_flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("snapshot.json"));
    let json = serde_json::to_string_pretty(&snap).expect("serialize");
    std::fs::write(&out, &json).expect("write snapshot");
    println!(
        "wrote {} ({} jobs, {} nodes, {} runs)",
        out.display(),
        snap.jobs.len(),
        snap.nodes.len(),
        snap.cost_totals.runs
    );
}

fn cmd_render(args: &[String]) {
    let (shared, _clock) = proof_dashboard(lease_ms(args));
    let snap = shared.lock().expect("lock").snapshot();
    let out = parse_flag(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("dashboard.html"));
    let html = oep_opsdash::html::render_html(&snap);
    std::fs::write(&out, &html).expect("write html");
    println!("wrote {}", out.display());
}

fn cmd_serve(args: &[String]) {
    let listen = parse_flag(args, "--listen").unwrap_or_else(|| "127.0.0.1:8080".to_string());
    let live = has_flag(args, "--live");
    let lease = lease_ms(args);

    // Live server uses wall clock so the timeline is real.
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let dash = Dashboard::with_lease(clock, demo::fleet(), lease);
    let shared: http::SharedDashboard = Arc::new(Mutex::new(dash));
    if live {
        demo::spawn_live_driver(Arc::clone(&shared), std::time::Duration::from_millis(250));
    }
    let addr = parse_addr(&listen);
    println!("oep-opsdash serving on {addr}  (live={live}, lease={lease}ms)");
    http::serve(shared, addr).unwrap_or_else(|e| {
        eprintln!("serve error: {e}");
        process::exit(1);
    });
}

/// Accept `HOST:PORT` or a bare `PORT` (localhost).
fn parse_addr(s: &str) -> SocketAddr {
    if let Ok(a) = s.parse() {
        return a;
    }
    let port: u16 = s.parse().unwrap_or_else(|_| usage());
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}
