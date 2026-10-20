//! oep-sandbox -- P9: P2P compute lane (workstream P9), worker sandbox.
//!
//! P9-M3-T03 delivers the **worker sandbox**: the enforcement layer that runs
//! a P9 worker process under hard resource caps and no network egress by
//! default. It is the "sandbox" half of the P9 worker boundary; the
//! worker functions the sandbox runs are delivered by P9-M3-T01/T02 and the
//! caps themselves are declared by the [`oep-job`] job/task spec
//! (P9-M1-T01). This crate mirrors the `oep-job` `ResourceCaps` shape by
//! value ([`SandboxLimits`]) so it stacks on the P9 wave without a
//! cross-crate dependency (per `docs/layout.md`, no cross-crate `path` deps
//! until the P6 importer hub).
//!
//! # Enforcement model (Linux, kernel-enforced, unprivileged)
//!
//! A worker is spawned with these boundaries applied *before* `exec` (in the
//! child), so no code the worker runs can weaken them:
//!
//! | cap | mechanism | breach result | module |
//! |-----|-----------|---------------|--------|
//! | CPU | `RLIMIT_CPU` hard ceiling | SIGXCPU/SIGKILL after `cpu_time_seconds` of CPU | [`rlimit`] |
//! | RAM | `RLIMIT_AS` + `RLIMIT_DATA` hard ceiling | allocation fails (`ENOMEM`), never runaway | [`rlimit`] |
//! | Disk | `RLIMIT_FSIZE` on the scratch file | `write` fails `EFBIG` / SIGXFSZ | [`rlimit`] |
//! | Network egress | seccomp BPF filter (off by default) | any non-`AF_UNIX` `socket(2)` returns `EPERM` | [`seccomp`] |
//! | Wall clock | runner side watchdog | worker killed after [`SandboxLimits::timeout_seconds`] | [`run`] |
//!
//! Every mechanism is a kernel check on the worker's *own* syscalls and
//! resource usage: the worker cannot opt out, and "escaping the sandbox"
//! (opening a socket, eating unbounded CPU/RAM/disk) is a failed operation,
//! not a symptom. That is the testable boundary the P11 threat model will
//! build on (see `docs/compute/workers/sandbox.md`).
//!
//! The caps are hard ceilings, not proportional shares: a 2000-millicore
//! request becomes a bounded CPU-time budget, not a 2-core throttle. Linear
//! throttling (`cpu.max`/`memory.max` in a cgroup v2 leaf) is available as an
//! opt-in deploy hardener via [`SandboxBuilder::cgroup_root`] where a
//! writable, delegated cgroup tree exists (root infra); it is not required
//! for the acceptance boundary.
//!
//! # Fail-closed network policy
//!
//! The egress boundary is **fail-closed**. Installing the seccomp filter
//! requires only `PR_SET_NO_NEW_PRIVS` and plain Linux — it works on the
//! intended worker hosts (bare-metal NAS/U1/DGX nodes, and the cgroup v2 user
//! trees on this program's infra). A few constrained environments forbid
//! nested seccomp installation (Docker's default profile, locked-down
//! sandboxes). There the sandbox **refuses to spawn** a worker requested with
//! `network_egress: false` rather than run it with network access it was
//! never granted: a worker with blocked-by-default egress either runs with
//! egress provably blocked, or does not run at all. The filter's decision
//! table is verified independently of installability by simulating the exact
//! instruction stream the kernel executes ([`seccomp::simulate`]).
//!
//! # Example
//!
//! ```no_run
//! use oep_sandbox::{SandboxBuilder, SandboxLimits};
//! use std::path::Path;
//!
//! let sb = SandboxBuilder::new(SandboxLimits::conservative_default()).build();
//! let out = sb.run_command(Path::new("/bin/echo"), &["-n", "hello"])?;
//! assert!(out.success);
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! [oep-job]: https://192.168.0.101:3200/keelworks/keelworks
#![deny(missing_docs)]

pub mod cgroup;
pub mod limits;
pub mod rlimit;
pub mod run;
pub mod seccomp;

/// Re-exports of the crate-level sandbox types (`SandboxLimits` caps, the
/// [`Sandbox`]/[`SandboxBuilder`] runner, and `SandboxOutcome`).
pub use limits::{
    SandboxLimits, MAX_CPU_MILLICORES, MAX_CPU_TIME_SECONDS, MAX_DISK_MIB, MAX_MEMORY_MIB,
    MAX_TIMEOUT_SECONDS,
};
pub use run::{Sandbox, SandboxBuilder, SandboxOutcome};
/// The default egress policy: `false` = blocked by default (P9-M3-T03).
pub use seccomp::DEFAULT_NETWORK_EGRESS;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-COMPUTE";
