//! Isolith contributor one-command gate (`cargo xtask ci`).
//!
//! Standard-library only, no dependencies on purpose. The command list mirrors
//! what the Gitea/GitHub CI jobs run on every push (lint = fmt + clippy, unit
//! = `cargo test --workspace --lib`, integration = `cargo test --workspace
//! --tests`) plus `pytest tests/`, which is the repo's documented gate
//! (CONTRIBUTING.md). So a contributor's local `cargo xtask ci` runs exactly
//! the checks CI runs, before CI does.
//!
//! Subcommands:
//!   ci            run the full non-OCCT gate (fmt, clippy, unit+integration
//!                 tests, pytest). Add --kernel to also build the pinned OCCT
//!                 8.0.1 dependency and run the through-the-seam kernel tests.
//!   occt          build the pinned OCCT dependency only (scripts/kernel/
//!                 build-occt.sh) and print the install prefix.
//!   check-sysdeps verify the toolchain and system tools a build needs, cheap
//!                 and fast; used by CI as the devcontainer rot gate and by
//!                 contributors to confirm their environment before a long
//!                 build.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{self, Command};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let (cmd, rest) = match args.split_first() {
        Some((c, r)) => (c.as_str(), r),
        None => ("ci", &[][..]),
    };

    let outcome = match cmd {
        "ci" => cmd_ci(rest),
        "occt" => {
            if cmd_occt() {
                0
            } else {
                1
            }
        }
        "check-sysdeps" => cmd_check_sysdeps(),
        "help" | "--help" | "-h" => {
            print_usage();
            0
        }
        other => {
            eprintln!("xtask: unknown subcommand '{other}'");
            print_usage();
            2
        }
    };
    process::exit(outcome);
}

fn print_usage() {
    eprintln!(
        "Isolith xtask: contributor one-command gate\n\n\
         usage:\n  \
         cargo xtask ci            run the full non-OCCT gate\n  \
         cargo xtask ci --kernel   also build OCCT 8.0.x and run kernel seam tests\n  \
         cargo xtask occt          build the pinned OCCT dependency only\n  \
         cargo xtask check-sysdeps verify toolchain and system tools\n\n\
         run from anywhere inside the repository; the workspace root is located automatically."
    );
}

fn repo_root() -> PathBuf {
    let mut dir = env::current_dir().expect("xtask: cannot read current directory");
    loop {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.is_file()
            && std::fs::read_to_string(&cargo_toml)
                .map(|s| s.contains("[workspace]"))
                .unwrap_or(false)
        {
            return dir;
        }
        if !dir.pop() {
            eprintln!("xtask: could not locate workspace root (no Cargo.toml with [workspace] found above cwd)");
            process::exit(2);
        }
    }
}

/// Run a command inheriting stdio, print a header line, return exit status.
fn run(prog: &str, args: &[&str]) -> bool {
    println!("==> {prog} {}", args.join(" "));
    let status = Command::new(prog).args(args).status();
    match status {
        Ok(s) => s.success(),
        Err(e) => {
            eprintln!("xtask: failed to spawn '{prog}': {e}");
            false
        }
    }
}

fn run_in(dir: &Path, prog: &str, args: &[&str]) -> bool {
    println!("==> [{}] {prog} {}", dir.display(), args.join(" "));
    let status = Command::new(prog).current_dir(dir).args(args).status();
    match status {
        Ok(s) => s.success(),
        Err(e) => {
            eprintln!("xtask: failed to spawn '{prog}' in {}: {e}", dir.display());
            false
        }
    }
}

fn ok(what: &str) {
    println!("[ok]   {what}");
}

fn fail(what: &str) {
    eprintln!("[FAIL] {what}");
}

/// Pick the python interpreter for the pytest leg: prefer an active venv, then
/// the repo .venv (uv convention), then system python3.
fn python_probe(root: &Path) -> Option<PathBuf> {
    if let Ok(v) = env::var("VIRTUAL_ENV") {
        let p = PathBuf::from(v).join("bin").join("python");
        if p.is_file() {
            return Some(p);
        }
    }
    let venv = root.join(".venv").join("bin").join("python");
    if venv.is_file() {
        return Some(venv);
    }
    // `python3` found on PATH (check silently; the version line must not leak).
    let probe = Command::new("python3").arg("--version").output();
    if probe.map(|o| o.status.success()).unwrap_or(false) {
        return Some(PathBuf::from("python3"));
    }
    None
}

fn cmd_ci(rest: &[String]) -> i32 {
    let kernel = rest.iter().any(|a| a == "--kernel");
    let root = repo_root();
    let start = std::time::Instant::now();
    let mut failed: Vec<&str> = Vec::new();

    println!(
        "== Isolith gate: cargo xtask ci{} ==",
        if kernel { " --kernel" } else { "" }
    );
    if !run("cargo", &["fmt", "--all", "--", "--check"]) {
        failed.push("rustfmt");
    }
    // Lint and format-check the dev-tooling crate itself so xtask cannot rot.
    if !run(
        "cargo",
        &[
            "fmt",
            "--manifest-path",
            "./xtask/Cargo.toml",
            "--",
            "--check",
        ],
    ) {
        failed.push("xtask rustfmt");
    }
    if !run(
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ) {
        failed.push("clippy");
    }
    if !run(
        "cargo",
        &[
            "clippy",
            "--manifest-path",
            "./xtask/Cargo.toml",
            "--",
            "-D",
            "warnings",
        ],
    ) {
        failed.push("xtask clippy");
    }
    if !run("cargo", &["test", "--workspace", "--lib"]) {
        failed.push("unit tests");
    }
    if !run("cargo", &["test", "--workspace", "--tests"]) {
        failed.push("integration tests");
    }

    // Python leg.
    match python_probe(&root) {
        Some(py) => {
            let py = py.to_str().expect("xtask: non-utf8 python path");
            if !run_in(&root, py, &["-m", "pytest", "tests/", "-q"]) {
                failed.push("pytest");
            }
        }
        None => {
            fail("no python interpreter found (pytest leg skipped as FAIL): install python3 or create .venv with uv sync");
            failed.push("pytest (no interpreter)");
        }
    }

    if kernel {
        println!("== kernel seam: building pinned OCCT 8.0.1 ==");
        if !cmd_occt() {
            failed.push("kernel seam: OCCT build");
        } else if !run(
            "cargo",
            &[
                "test",
                "-p",
                "oep-geo",
                "--features",
                "kernel",
                "--test",
                "kernel_seam",
                "--",
                "--test-threads=1",
            ],
        ) {
            failed.push("kernel seam tests");
        } else if !run(
            "cargo",
            &[
                "test",
                "-p",
                "oep-geo",
                "--features",
                "kernel",
                "--test",
                "kernel_replay",
                "--",
                "--test-threads=1",
            ],
        ) {
            failed.push("kernel replay tests");
        }
    }

    if failed.is_empty() {
        let secs = start.elapsed().as_secs();
        println!("\n== gate PASS ({secs}s) ==");
        0
    } else {
        eprintln!("\n== gate FAIL: {}", failed.join(", "));
        1
    }
}

fn cmd_occt() -> bool {
    let root = repo_root();
    let script = root.join("scripts").join("kernel").join("build-occt.sh");
    if !script.is_file() {
        fail("scripts/kernel/build-occt.sh is missing; run from a full checkout");
        return false;
    }
    if !run("bash", &[script.to_str().expect("non-utf8 path")]) {
        return false;
    }
    ok("pinned OCCT installed (see prefix printed above)");
    true
}

fn cmd_check_sysdeps() -> i32 {
    let root = repo_root();
    let mut bad = 0usize;

    println!("== Isolith sysdeps check ==");

    // Rust toolchain vs rust-toolchain.toml pin.
    let pin = std::fs::read_to_string(root.join("rust-toolchain.toml")).unwrap_or_default();
    let channel = pin
        .lines()
        .find(|l| l.trim_start().starts_with("channel"))
        .and_then(|l| {
            let v = l.split('=').nth(1)?.trim().trim_matches('"').to_string();
            let v = v.trim_matches('\'').to_string();
            Some(v)
        });

    let rustc = Command::new("rustc").arg("--version").output();
    match rustc {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if let Some(p) = &channel {
                if v.contains(p.as_str()) {
                    ok(&format!("rustc {v} matches pin"));
                } else {
                    fail(&format!(
                        "rustc {v} does not match rust-toolchain.toml pin {p}"
                    ));
                    bad += 1;
                }
            } else {
                fail("rust-toolchain.toml has no parseable channel line");
                bad += 1;
            }
        }
        _ => {
            fail(
                "rustc not on PATH (install rustup and let rust-toolchain.toml pin the toolchain)",
            );
            bad += 1;
        }
    }

    // Cargo + gate-invoked subcommands.
    for (prog, check) in [
        ("cargo", "--version"),
        ("cargo", "clippy --version"),
        ("cargo", "fmt --version"),
    ] {
        let okv = Command::new(prog)
            .args(if check.contains(' ') {
                check.split(' ').collect()
            } else {
                vec![check]
            })
            .output();
        match okv {
            Ok(o) if o.status.success() => ok(&format!("{prog} {check}")),
            _ => {
                fail(&format!(
                    "{prog} {check} failed (rustup components rustfmt and clippy are required)"
                ));
                bad += 1;
            }
        }
    }

    // System tools the OCCT fetch/build leg needs.
    for prog in ["cmake", "g++", "make", "curl", "bash", "git"] {
        let o = Command::new(prog).arg("--version").output();
        match o {
            Ok(o) if o.status.success() => ok(prog),
            _ => {
                fail(&format!(
                    "{prog} missing or broken; devcontainer installs it"
                ));
                bad += 1;
            }
        }
    }

    // Python + pytest.
    match python_probe(&root) {
        Some(py) => {
            let pystr = py.to_str().unwrap_or("python3");
            match Command::new(pystr)
                .arg("-c")
                .arg("import pytest, sys; print('pytest', pytest.__version__)")
                .output()
            {
                Ok(o) if o.status.success() => ok(&format!(
                    "python+pytest ready: {} {}",
                    pystr,
                    String::from_utf8_lossy(&o.stdout).trim()
                )),
                _ => {
                    fail("pytest not importable by the probed python; run `uv sync` in the repo root (devcontainer postCreate does this)");
                    bad += 1;
                }
            }
        }
        None => {
            fail("no python interpreter found");
            bad += 1;
        }
    }

    // uv, when present (the repo's Python tooling convention).
    match Command::new("uv").arg("--version").output() {
        Ok(o) if o.status.success() => {
            ok(&format!("uv {}", String::from_utf8_lossy(&o.stdout).trim()))
        }
        _ => ok("uv not on PATH (optional on a host with a manual python; devcontainer ships it)"),
    }

    if bad == 0 {
        println!("\n== check-sysdeps PASS ==");
        0
    } else {
        eprintln!("\n== check-sysdeps FAIL ({bad} missing/broken) ==");
        1
    }
}
