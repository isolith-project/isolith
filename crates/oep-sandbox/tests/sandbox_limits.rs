//! P9-M3-T03 acceptance tests: the worker sandbox boundary.
//!
//! Every test drives a real worker (`sbox-probe`, this package's bin) through
//! the sandbox and asserts the kernel-enforced boundary held. The acceptance
//! criteria each test pins:
//!
//! - **CPU/RAM caps enforced**: `cpu_cap_enforced_kills_runaway_worker`,
//!   `ram_cap_enforced_allocation_fails_enomem`, `disk_cap_enforced_writes_fail`.
//! - **Egress blocked by default and testable**: `egress_blocked_by_default_*`.
//! - **Escaping the sandbox fails a test**: the `escape_*` tests each attempt
//!   a concrete escape (raw socket, connect, unbounded CPU, unbounded RAM,
//!   disk flood) and assert the escape FAILS.
//!
//! Resource-cap tests set `network_egress: true` so they prove the cap
//! mechanism in isolation, independent of whether the host permits installing
//! the egress filter. Egress/escape tests assert one of two REAL outcomes:
//!
//! - host allows seccomp -> the worker runs caged and its escape attempt is
//!   denied with `EPERM` (the live boundary), or
//! - host forbids nested seccomp (Docker's default profile, locked-down dev
//!   sandboxes) -> the sandbox REFUSES to spawn (fail-closed), so there is no
//!   uncaged worker to escape with.
//!
//! The egress filter *decision table* itself is unit-verified in
//! `src/seccomp.rs` by simulating the exact instruction stream the kernel
//! runs, so the security property is covered even where install is blocked.
//!
//! Run with `cargo test -p oep-sandbox`.

use oep_sandbox::{SandboxBuilder, SandboxLimits};
use std::path::PathBuf;
use std::sync::OnceLock;

fn probe() -> PathBuf {
    // The integration test binary lives at <target>/deps/sandbox_limits-<hash>;
    // the sbox-probe bin at <target>/sbox-probe (cargo builds package bins).
    let exe = std::env::current_exe().expect("current_exe for probe lookup");
    let target = exe.parent().and_then(|d| d.parent());
    if let Some(dir) = target {
        let bin = dir.join("sbox-probe");
        if bin.exists() {
            return bin;
        }
    }
    if let Some(p) = option_env!("CARGO_BIN_EXE_sbox_probe") {
        return PathBuf::from(p);
    }
    panic!(
        "cannot locate the sbox-probe worker next to {} (searched <target>/sbox-probe)",
        exe.display()
    );
}

fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("oep-sandbox-{}-{}", std::process::id(), name))
}

fn envs(list: &[(&str, &str)]) -> Vec<String> {
    list.iter().map(|(k, v)| format!("{k}={v}")).collect()
}

/// True when THIS host can install and enforce the seccomp egress filter.
/// Probed once per suite run via a throwaway sbox-probe child (on hosts whose
/// outer profile forbids nested seccomp the probe dies with SIGSYS — exactly
/// like the real worker path — and we report "not available").
fn seccomp_available() -> bool {
    static AVAIL: OnceLock<bool> = OnceLock::new();
    *AVAIL.get_or_init(|| {
        let out = std::process::Command::new(probe())
            .env("OEP_SBOX_PROBE", "seccomp_probe")
            .output()
            .expect("probe spawn");
        out.status.success() && String::from_utf8_lossy(&out.stdout).contains("SECCOMP_ENFORCED")
    })
}

/// Run a worker under the given limits with the requested egress policy;
/// returns the outcome, or the spawn error when the sandbox refused to run
/// the worker (fail-closed because egress could not be enforced).
fn run_sb(
    limits: SandboxLimits,
    egress: bool,
    env: &[(&str, &str)],
) -> Result<oep_sandbox::SandboxOutcome, std::io::Error> {
    let limits = SandboxLimits {
        network_egress: egress,
        ..limits
    };
    let sb = SandboxBuilder::new(limits).build();
    sb.run_command_env(probe().as_path(), &[], &envs(env))
}

// ---------------------------------------------------------------------------
// CPU / RAM / disk caps (tested in isolation: egress=true so the cap is proven
// without depending on the host's seccomp posture)
// ---------------------------------------------------------------------------

#[test]
fn cpu_cap_enforced_kills_runaway_worker() {
    let limits = SandboxLimits {
        cpu_millicores: 1000,
        memory_mib: 256,
        disk_mib: None,
        cpu_time_seconds: Some(1), // 1 CPU second, then SIGXCPU/SIGKILL
        network_egress: true,
        timeout_seconds: Some(60), // watchdog, in case the cap failed
    };
    let started = std::time::Instant::now();
    let out = run_sb(
        limits,
        true,
        &[("OEP_SBOX_PROBE", "spin"), ("OEP_SBOX_SPIN_SECONDS", "600")],
    )
    .expect("sandbox with egress=true must spawn");
    let wall = started.elapsed();

    assert!(
        !out.success,
        "runaway CPU worker must NOT be allowed to finish"
    );
    assert!(
        out.cpu_exhausted(),
        "expected SIGXCPU(24)/SIGKILL(9), got signal={:?} stderr={}",
        out.signal,
        out.stderr
    );
    assert!(
        wall < std::time::Duration::from_secs(30),
        "kill must be prompt, took {wall:?}"
    );
}

#[test]
fn ram_cap_enforced_allocation_fails_enomem() {
    let limits = SandboxLimits {
        cpu_millicores: 1000,
        memory_mib: 64, // hard RLIMIT_AS/RLIMIT_DATA ceiling
        disk_mib: None,
        cpu_time_seconds: Some(120),
        network_egress: true,
        timeout_seconds: Some(60),
    };

    // Control: a small allocation comfortably inside the cap succeeds.
    let ok = run_sb(
        limits.clone(),
        true,
        &[("OEP_SBOX_PROBE", "alloc"), ("OEP_SBOX_ALLOC_MIB", "4")],
    )
    .expect("sandbox with egress=true must spawn");
    assert!(ok.success, "small alloc must succeed: {}", ok.stderr);
    assert!(
        ok.stdout.contains("ALLOC_OK"),
        "control alloc marker: {}",
        ok.stdout
    );

    // Attempt: a 4 GiB alloc against a 64 MiB cap must FAIL (ENOMEM).
    let big = run_sb(
        limits,
        true,
        &[("OEP_SBOX_PROBE", "alloc"), ("OEP_SBOX_ALLOC_MIB", "4096")],
    )
    .expect("sandbox with egress=true must spawn");
    assert!(
        !big.success,
        "oversized alloc must NOT succeed: {}",
        big.stdout
    );
    assert!(
        big.stdout.contains("ALLOC_FAILED"),
        "expected ALLOC_FAILED marker, got stdout={} stderr={}",
        big.stdout,
        big.stderr
    );
}

#[test]
fn disk_cap_enforced_writes_fail() {
    let target = scratch("disk-probe.tmp");
    let limits = SandboxLimits {
        cpu_millicores: 1000,
        memory_mib: 256,
        disk_mib: Some(1), // 1 MiB scratch cap
        cpu_time_seconds: Some(60),
        network_egress: true,
        timeout_seconds: Some(60),
    };
    let out = run_sb(
        limits,
        true,
        &[
            ("OEP_SBOX_PROBE", "writefile"),
            ("OEP_SBOX_WRITE_PATH", target.to_str().unwrap()),
        ],
    )
    .expect("sandbox with egress=true must spawn");
    let _ = std::fs::remove_file(&target);

    // Either the kernel killed it with SIGXFSZ (25) or the probe's write hit
    // EFBIG and it printed WRITE_BLOCKED. In both cases the unbounded-scratch
    // attempt FAILED.
    let blocked = out.disk_exhausted() || (out.success && out.stdout.contains("WRITE_BLOCKED"));
    assert!(blocked, "disk cap not enforced: {:?}", out);
}

// ---------------------------------------------------------------------------
// Network egress: blocked by default, testable, opt-in toggle
// ---------------------------------------------------------------------------

/// Default (no `network_egress` set) = an escapist worker cannot reach the
/// network: either its connect is denied with EPERM (live filter) or the
/// sandbox refuses to produce a worker at all (fail-closed).
#[test]
fn egress_blocked_by_default_connect_fails() {
    let limits = SandboxLimits::conservative_default();
    if seccomp_available() {
        let out = run_sb(
            limits.clone(),
            false,
            &[
                ("OEP_SBOX_PROBE", "connect"),
                ("OEP_SBOX_CONNECT_ADDR", "8.8.8.8:53"),
            ],
        )
        .expect("egress-capable sandbox must spawn the worker");
        assert!(
            out.stdout.contains("CONNECT_BLOCKED"),
            "expected egress blocked (EPERM), got stdout={} stderr={}",
            out.stdout,
            out.stderr
        );
        assert_eq!(out.exit_code, Some(0), "blocked connect probe exits 0");
    } else {
        let err = run_sb(
            limits,
            false,
            &[
                ("OEP_SBOX_PROBE", "connect"),
                ("OEP_SBOX_CONNECT_ADDR", "8.8.8.8:53"),
            ],
        )
        .expect_err("sandbox must refuse to spawn when egress cannot be enforced");
        eprintln!("SKIP-live-filter (host forbids nested seccomp): spawn refused: {err}");
    }
}

/// Egress default-off must also deny a raw AF_PACKET socket (layer 2).
#[test]
fn egress_blocked_by_default_raw_packet_fails() {
    let limits = SandboxLimits::conservative_default();
    if seccomp_available() {
        let out = run_sb(limits, false, &[("OEP_SBOX_PROBE", "socket_af_packet")])
            .expect("egress-capable sandbox must spawn the worker");
        assert!(
            out.stdout.contains("SOCKET_BLOCKED"),
            "raw packet socket must be denied: {}",
            out.stdout
        );
    } else {
        let err = run_sb(limits, false, &[("OEP_SBOX_PROBE", "socket_af_packet")])
            .expect_err("sandbox must refuse to spawn when egress cannot be enforced");
        eprintln!("SKIP-live-filter (host forbids nested seccomp): spawn refused: {err}");
    }
}

/// Explicit opt-in (`network_egress: true`) skips the filter: socket creation
/// is allowed again regardless of host seccomp posture.
#[test]
fn egress_opted_in_is_not_permission_denied() {
    let out = run_sb(
        SandboxLimits::conservative_default(),
        true,
        &[("OEP_SBOX_PROBE", "socket_af_inet")],
    )
    .expect("sandbox with egress=true must spawn");
    assert!(
        !out.stdout.contains("SOCKET_BLOCKED"),
        "egress opt-in must not be seccomp-blocked; got {}",
        out.stdout
    );
    assert!(
        out.stdout.contains("SOCKET_OK"),
        "socket creation should succeed with egress on; got {}",
        out.stdout
    );
}

/// P11-T07 P2T-01-SB regression: the io_uring subsystem must be denied so the
/// `IORING_OP_SOCKET` egress bypass cannot create an AF_INET descriptor
/// without ever calling `socket(2)`. A filter that only denies socket(2) is
/// reachable around; this test pins the hardened decision.
///
/// Outcomes on egress=false:
///   - URING_BLOCKED (exit 0)       -> hardened filter or outer profile denied
///     io_uring_setup: bypass closed.
///   - URING_UNAVAILABLE (exit 1)   -> kernel has no io_uring: nothing to bypass.
///   - URING_OK (exit 2)            -> io_uring reachable => the egress bypass
///     is open; the sandbox FAILED to block egress.
#[test]
fn escape_via_io_uring_socket_fails() {
    let limits = SandboxLimits::conservative_default();
    if seccomp_available() {
        let out = run_sb(limits, false, &[("OEP_SBOX_PROBE", "io_uring_sock")])
            .expect("egress-capable sandbox must spawn the worker");
        assert!(
            out.stdout.contains("URING_BLOCKED") || out.stdout.contains("URING_UNAVAILABLE"),
            "io_uring must be denied (bypass closed); got stdout={} stderr={}",
            out.stdout,
            out.stderr
        );
    } else {
        let err = run_sb(limits, false, &[("OEP_SBOX_PROBE", "io_uring_sock")])
            .expect_err("escape attempt must be refused (fail-closed)");
        eprintln!("SKIP-live-filter (host forbids nested seccomp): spawn refused: {err}");
    }
}

// ---------------------------------------------------------------------------
// Escaping the sandbox FAILS a test
// ---------------------------------------------------------------------------

/// Escape attempt: open a raw AF_INET socket -> denied (or refused to run).
#[test]
fn escape_via_raw_inet_socket_fails() {
    let limits = SandboxLimits::conservative_default();
    if seccomp_available() {
        let out = run_sb(limits, false, &[("OEP_SBOX_PROBE", "socket_af_inet")])
            .expect("egress-capable sandbox must spawn the worker");
        assert!(
            out.stdout.contains("SOCKET_BLOCKED"),
            "escape must fail: {}",
            out.stdout
        );
    } else {
        let err = run_sb(limits, false, &[("OEP_SBOX_PROBE", "socket_af_inet")])
            .expect_err("escape attempt must be refused (fail-closed)");
        eprintln!("SKIP-live-filter (host forbids nested seccomp): spawn refused: {err}");
    }
}

/// Escape attempt: reach a routable IP -> denied (or refused to run).
#[test]
fn escape_via_connect_fails() {
    let limits = SandboxLimits::conservative_default();
    if seccomp_available() {
        let out = run_sb(
            limits,
            false,
            &[
                ("OEP_SBOX_PROBE", "connect"),
                ("OEP_SBOX_CONNECT_ADDR", "8.8.8.8:53"),
            ],
        )
        .expect("egress-capable sandbox must spawn the worker");
        assert!(
            out.stdout.contains("CONNECT_BLOCKED"),
            "escape must fail: {}",
            out.stdout
        );
    } else {
        let err = run_sb(
            limits,
            false,
            &[
                ("OEP_SBOX_PROBE", "connect"),
                ("OEP_SBOX_CONNECT_ADDR", "8.8.8.8:53"),
            ],
        )
        .expect_err("escape attempt must be refused (fail-closed)");
        eprintln!("SKIP-live-filter (host forbids nested seccomp): spawn refused: {err}");
    }
}

/// Escape attempt: consume unbounded CPU -> killed by the cap (always live).
#[test]
fn escape_via_unbounded_cpu_fails() {
    let out = run_sb(
        SandboxLimits {
            cpu_time_seconds: Some(1),
            ..SandboxLimits::conservative_default()
        },
        true, // cap mechanism independent of egress posture
        &[("OEP_SBOX_PROBE", "spin"), ("OEP_SBOX_SPIN_SECONDS", "600")],
    )
    .expect("sandbox with egress=true must spawn");
    assert!(!out.success, "escape via CPU must fail");
    assert!(
        out.cpu_exhausted(),
        "killed by CPU cap, got {:?}",
        out.signal
    );
}

/// Escape attempt: consume unbounded RAM -> allocation fails ENOMEM (always live).
#[test]
fn escape_via_unbounded_ram_fails() {
    let out = run_sb(
        SandboxLimits {
            memory_mib: 64,
            ..SandboxLimits::conservative_default()
        },
        true,
        &[("OEP_SBOX_PROBE", "alloc"), ("OEP_SBOX_ALLOC_MIB", "8192")],
    )
    .expect("sandbox with egress=true must spawn");
    assert!(!out.success, "escape via RAM must fail");
    assert!(
        out.stdout.contains("ALLOC_FAILED"),
        "ENOMEM expected: {}",
        out.stdout
    );
}

/// Escape attempt: flood scratch disk past the cap -> write fails (always live).
#[test]
fn escape_via_disk_flood_fails() {
    let target = scratch("escape-disk.tmp");
    let out = run_sb(
        SandboxLimits {
            disk_mib: Some(1),
            ..SandboxLimits::conservative_default()
        },
        true,
        &[
            ("OEP_SBOX_PROBE", "writefile"),
            ("OEP_SBOX_WRITE_PATH", target.to_str().unwrap()),
        ],
    )
    .expect("sandbox with egress=true must spawn");
    let _ = std::fs::remove_file(&target);
    let blocked = out.disk_exhausted() || (out.success && out.stdout.contains("WRITE_BLOCKED"));
    assert!(blocked, "escape via disk must fail: {:?}", out);
}

// ---------------------------------------------------------------------------
// Conservative defaults + egress posture
// ---------------------------------------------------------------------------

/// The same cap the job spec would negotiate must still let an honest worker
/// run to completion (the sandbox is a boundary, not a spoiler).
#[test]
fn conservative_default_sandbox_runs_a_benign_worker() {
    let out = run_sb(
        SandboxLimits::conservative_default(),
        true,
        &[("OEP_SBOX_PROBE", "alloc"), ("OEP_SBOX_ALLOC_MIB", "1")],
    )
    .expect("sandbox with egress=true must spawn");
    assert!(out.success, "benign worker must succeed: {}", out.stderr);
    assert!(out.stdout.contains("ALLOC_OK"));
}

/// The default policy flag is off ("egress blocked by default") and the
/// capability probe reports honestly on this host.
#[test]
fn egress_policy_defaults_off_and_probe_is_honest() {
    let default_egress = oep_sandbox::DEFAULT_NETWORK_EGRESS;
    assert!(!default_egress, "egress must be off by default");
    let avail = seccomp_available();
    eprintln!("host seccomp-egress capability: {avail}");
}
