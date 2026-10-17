//! The sandbox runner: spawn a worker with RLIMIT + seccomp applied in the
//! child, optional cgroup attachment in the parent, a wall-clock timeout
//! watchdog, and a structured [`SandboxOutcome`].

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// CommandExt::pre_exec and ExitStatusExt::signal are linux/unix traits.
#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

use crate::cgroup::CgroupGuard;
use crate::limits::SandboxLimits;

/// Describes how a sandboxed run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxOutcome {
    /// True only when the worker exited 0 on its own (did not time out).
    pub success: bool,
    /// Exit code when the worker exited normally.
    pub exit_code: Option<i32>,
    /// Terminating signal when the worker was killed by a signal (e.g.
    /// SIGXCPU/SIGKILL from RLIMIT_CPU, SIGXFSZ from RLIMIT_FSIZE).
    pub signal: Option<i32>,
    /// True when the runner's wall-clock watchdog killed the worker.
    pub timeout: bool,
    /// Captured stdout (lossy; workers should keep output textual).
    pub stdout: String,
    /// Captured stderr.
    pub stderr: String,
}

impl SandboxOutcome {
    /// True when the run was killed by the CPU cap (SIGXCPU=24 / SIGKILL=9).
    pub fn cpu_exhausted(&self) -> bool {
        matches!(self.signal, Some(24) | Some(9))
    }

    /// True when the run was killed by the disk cap (SIGXFSZ=25).
    pub fn disk_exhausted(&self) -> bool {
        self.signal == Some(25)
    }

    /// Human-readable kill reason for logs.
    pub fn kill_reason(&self) -> &'static str {
        if self.timeout {
            "timeout"
        } else if self.cpu_exhausted() {
            "cpu-limit"
        } else if self.disk_exhausted() {
            "disk-limit"
        } else if self.signal.is_some() {
            "signal"
        } else if self.success {
            "completed"
        } else {
            "non-zero-exit"
        }
    }
}

/// Builder for a [`Sandbox`].
#[derive(Debug, Clone)]
pub struct SandboxBuilder {
    limits: SandboxLimits,
    cgroup_root: Option<std::path::PathBuf>,
}

impl SandboxBuilder {
    /// Start building a sandbox for the given caps.
    pub fn new(limits: SandboxLimits) -> Self {
        SandboxBuilder {
            limits,
            cgroup_root: None,
        }
    }

    /// Opt into cgroup v2 enforcement: create a leaf cgroup under `root`
    /// (which must be a writable, delegated cgroup directory) with
    /// `cpu.max` / `memory.max` derived from the caps, and move the worker
    /// into it. `None` (default) runs RLIMIT-only — the acceptance core that
    /// works everywhere. Rooted deployers (NAS/DGX supervisors) can pass a
    /// real cgroup root to add linear throttling (see `docs/P9-M3/sandbox.md`).
    pub fn cgroup_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.cgroup_root = Some(root.into());
        self
    }

    /// Build the sandbox.
    pub fn build(self) -> Sandbox {
        debug_assert!(
            self.limits.is_valid(),
            "invalid SandboxLimits: {:?}",
            self.limits.violations()
        );
        Sandbox {
            limits: self.limits,
            cgroup_root: self.cgroup_root,
        }
    }
}

/// A configured worker sandbox that can run one command per invocation.
#[derive(Debug, Clone)]
pub struct Sandbox {
    limits: SandboxLimits,
    cgroup_root: Option<std::path::PathBuf>,
}

/// Poll interval for the watchdog loop.
const WATCHDOG_POLL: Duration = Duration::from_millis(20);

impl Sandbox {
    /// The caps this sandbox enforces.
    pub fn limits(&self) -> &SandboxLimits {
        &self.limits
    }

    /// Run `program` with `args` inside the sandbox.
    ///
    /// The command inherits the parent's environment (env vars can be
    /// overridden with `env` when a worker needs parameters). Returns the
    /// worker's outcome; I/O errors from spawning itself are returned as
    /// `Err` (e.g. missing binary).
    pub fn run_command(&self, program: &Path, args: &[&str]) -> io::Result<SandboxOutcome> {
        self.run_command_env(program, args, &[])
    }

    /// Run `program` with `args` inside the sandbox plus explicit env
    /// overrides (each entry a `KEY=VALUE` string).
    pub fn run_command_env(
        &self,
        program: &Path,
        args: &[&str],
        env: &[String],
    ) -> io::Result<SandboxOutcome> {
        let limits = self.limits.clone();
        let egress = limits.network_egress;
        // The child closure only needs the caps that survive its lockstep;
        // clone so the parent can still use `limits` for the cgroup setup.
        let limits_for_child = limits.clone();

        let mut cmd = Command::new(program);
        cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

        for kv in env {
            let (k, v) = kv.split_once('=').ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("env not KEY=VALUE: {kv}"),
                )
            })?;
            cmd.env(k, v);
        }

        // SAFETY: the closure runs in the forked-but-not-yet-execed child,
        // before any user code of `program` executes. We only lower resource
        // limits and install a seccomp filter; neither touches the parent.
        unsafe {
            cmd.pre_exec(move || {
                crate::rlimit::apply_limits(&limits_for_child)?;
                if !egress {
                    crate::seccomp::install_egress_block()?;
                }
                Ok(())
            });
        }

        let mut child = cmd.spawn()?;

        // Optional cgroup throttle: attach the worker to its leaf cgroup now,
        // before it does any heavy work.
        let guard = match &self.cgroup_root {
            Some(root) => match CgroupGuard::new(root, limits.cpu_millicores, limits.memory_mib) {
                Ok(Some(g)) => {
                    if g.attach(child.id().try_into().unwrap_or(0)).is_err() {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "sandbox: cgroup attach failed; refused to run uncaged worker",
                        ));
                    }
                    Some(g)
                }
                Ok(None) => None,
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e);
                }
            },
            None => None,
        };

        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        let (out_tx, out_rx) = std::sync::mpsc::channel::<io::Result<Vec<u8>>>();
        let (err_tx, err_rx) = std::sync::mpsc::channel::<io::Result<Vec<u8>>>();
        {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut buf = Vec::new();
                let r = stdout.read_to_end(&mut buf).map(|_| buf);
                let _ = out_tx.send(r);
            });
        }
        {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut buf = Vec::new();
                let r = stderr.read_to_end(&mut buf).map(|_| buf);
                let _ = err_tx.send(r);
            });
        }

        let deadline = self
            .limits
            .timeout_seconds
            .map(|s| Instant::now() + Duration::from_secs(s));

        let mut timed_out = false;
        let status = loop {
            if let Some(st) = child.try_wait()? {
                break st;
            }
            if let Some(dl) = deadline {
                if Instant::now() >= dl {
                    let _ = child.kill();
                    timed_out = true;
                    break child.wait()?;
                }
            }
            std::thread::sleep(WATCHDOG_POLL);
        };

        // Drain deterministically: the reader threads EOF when the child
        // exits, then send their buffers over the channels.
        let out_bytes = out_rx
            .recv()
            .map(|r| r.unwrap_or_default())
            .unwrap_or_default();
        let err_bytes = err_rx
            .recv()
            .map(|r| r.unwrap_or_default())
            .unwrap_or_default();
        let out = String::from_utf8_lossy(&out_bytes).into_owned();
        let err = String::from_utf8_lossy(&err_bytes).into_owned();

        drop(guard);

        Ok(SandboxOutcome {
            success: !timed_out && status.success(),
            exit_code: status.code(),
            signal: status.signal(),
            timeout: timed_out,
            stdout: out,
            stderr: err,
        })
    }
}
