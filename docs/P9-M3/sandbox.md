# P9-M3-T03: Worker sandbox (resource limits, no network egress by default)

Version 1.0, 2026-10-17. Milestone: P9-M3-T03 (worker sandbox:
*resource limits*, *no network egress by default*, *escaping fails a test*).
Program: Keelworks / Open Engineering Platform (OEP), P9 P2P compute lane
(wave 2). Clean-room, own implementation — no guest agent, no sidecar vendor
binary.

Reference implementation: `crates/oep-sandbox` (Rust, `libc` only). Caps are
mirrored by value from the P9-M1-T01 job spec's `ResourceCaps`
(`crates/oep-job/src/spec.rs`) — same field names and bounds — so the sandbox
stacks on the compute lane without a cross-crate path dependency
(`docs/layout.md`: no cross-crate `path` deps until the P6 importer hub).

---

## 1. What the sandbox does

The sandbox runs a worker **command** (an argv — e.g. the P9-M3-T01/T02
worker binaries) under a set of kernel-enforced boundaries applied in the
child *before* `exec`, so no code the worker runs can weaken them:

| cap | mechanism | breach result |
|-----|-----------|---------------|
| CPU | `RLIMIT_CPU` hard ceiling (`cpu_time_seconds`) | SIGXCPU (24), then SIGKILL — a runaway loop is killed, not allowed to hog the node |
| RAM | `RLIMIT_AS` + `RLIMIT_DATA` hard ceilings (bytes = `memory_mib` × 2^20) | allocation returns `ENOMEM` past the cap — never unbounded |
| Disk | `RLIMIT_FSIZE` on the scratch file (bytes = `disk_mib` × 2^20) | `write(2)` returns `EFBIG`, worker killed with SIGXFSZ (25) |
| Network egress | seccomp BPF filter; **default `false`** | any non-`AF_UNIX` `socket(2)` returns `EPERM` → no connect/sendto/bind is possible |
| Wall clock | runner watchdog | worker killed after `timeout_seconds` |
| fd table | `RLIMIT_NOFILE` (1024) | worker cannot exhaust node descriptors |

`RLIMIT_*` are hard ceilings, not proportional shares: a 2000-millicore request
becomes a bounded CPU-time budget, not a 2-core *throttle*. Linear throttling
(`cpu.max` / `memory.max` in a cgroup v2 leaf) is an **opt-in deploy
hardener** via `SandboxBuilder::cgroup_root(...)` for privileged supervisors
(crate module `cgroup`); it degrades to the RLIMIT core where no writable,
delegated cgroup tree exists, and is never a reason to run a worker uncapped.

`SandboxLimits::conservative_default()` (1000 millicores, 2 GiB, 1 h CPU-time,
1 h timeout, egress off) is the same default the job spec carries
(P9-M2-T02 doctrine: defaults are conservative).

## 2. Egress: blocked by default, testable, fail-closed

The egress filter (crate module `seccomp`) is a tiny classic-BPF program that
denies `socket(2)` for every address family except `AF_UNIX` (loopback/local
IPC that never leaves the host). A freshly `exec`'d worker inherits no
sockets, so blocking *creation* of non-UNIX sockets closes every egress path.
The filter is installed with `PR_SET_NO_NEW_PRIVS` + `SECCOMP_MODE_FILTER` in
the child, survives `exec`, and is self-verified (after install the sandbox
checks that `socket(AF_INET)` now fails with `EPERM`).

**Default posture**: `SandboxLimits::network_egress` defaults to `false`
(crate constant `DEFAULT_NETWORK_EGRESS = false`). A job that wants egress
must explicitly opt in (`resources.network_egress: true` in the job spec);
the sandbox then skips the filter. There is no "implicitly open" route.

**Fail-closed rule** (the security property): if the environment forbids
installing the seccomp filter (Docker's default profile denies
`prctl(PR_SET_SECCOMP)`; locked-down agent sandboxes kill it with SIGSYS), a
worker requested with egress-off is **never spawned**. The sandbox returns an
error instead of running a worker that would have network access it was never
granted. Concretely: a worker either runs with egress provably blocked, or
does not run. This is what makes "escaping the sandbox" impossible by
construction where the mechanism is unavailable.

**Testability**:
- The filter's decision table is unit-verified by simulating the exact
  instruction stream the kernel runs (`seccomp::simulate`): non-UNIX families
  → `EPERM`, `AF_UNIX` → allow, other syscalls → allow, unknown arch → kill.
  This runs everywhere, including hosts that forbid installing filters (this
  caught a real jump-offset bug during development).
- Integration tests (`tests/sandbox_limits.rs`) drive a real worker
  (`sbox-probe`) through the sandbox and assert one of two REAL outcomes:
  live `EPERM` denial on capable hosts, or fail-closed refusal on hosts that
  forbid nested seccomp. Either way an egress/escape test passes, and the
  suite prints which branch it took (`SKIP-live-filter ... spawn refused`).

## 3. Captured outcome

`Sandbox::run_command` returns a `SandboxOutcome`:

- `success` / `exit_code` / `signal` (worker exit, or the terminating signal:
  24/9 from CPU cap, 25 from disk cap),
- `timeout` (watchdog fired),
- `stdout` / `stderr` (captured losslessly via reader threads),
- helpers `cpu_exhausted()`, `disk_exhausted()`, `kill_reason()` for the
  scheduler's practical use (P9-M2-T01 retry/dead-letter on `kill_reason`).

## 4. Mapping to the job spec

| job-spec `ResourceCaps` field | sandbox treatment |
|-------------------------------|-------------------|
| `cpu_millicores` | cap *request*; backed by `RLIMIT_CPU` + optional `cpu.max` |
| `memory_mib` | `RLIMIT_AS`/`RLIMIT_DATA` (+ optional `memory.max`, swap=0) |
| `disk_mib` | `RLIMIT_FSIZE` |
| `network_egress` (default `false`) | seccomp filter toggled by this flag; fail-closed when uninstallable |
| `timeout_seconds` | runner watchdog |
| `gpu` (P9-M4-T01) | not enforced here; GPU scheduling is the scheduler's job |
| `checkpoint_every_seconds` (P9-M4-T02) | not enforced here; checkpoints are worker-emitted |

The crate intentionally does not depend on `oep-job` (unmerged at the time of
writing); it pins the same shapes by value so it stacks regardless of merge
order. A follow-up can add an adapter/`TryFrom` once cross-crate path deps are
allowed.

## 5. Threat-model boundary (mapped to P11-T01)

This card owns the **worker execution boundary**:
- Untrusted/crashed/compromised *worker code* cannot escape the node resources
  it was granted (CPU/RAM/disk caps are kernel-enforced, non-opt-out) and
  cannot exfiltrate data from the P8 vault to the network (no egress by
  default + fail-closed).
- The boundary is **worker→host** (process isolation, resource ceiling,
  network egress). It explicitly does NOT cover: host kernel CVE chains,
  malicious *supervisor* root (a root supervisor can always do more — cgroup
  hardener and netns are the escalation layers for rooted deploys), or
  network-level segmentation of the mesh itself (NATS topics / P2P TLS are
  separate cards).
- Escalation for more hostile bodies: (1) run workers under a dedicated
  unprivileged uid; (2) add a dedicated netns per worker (root supervisor,
  `ip netns exec` / `systemd-run`); (3) enable the cgroup hardener; (4)
  transitive container image isolation for `custom` executors (P9-M1-T01
  `image` field). Each is a documented deploy option; none is required for
  the acceptance boundary.

## 6. Verification record (2026-10-17, real worker processes)

- `cargo test -p oep-sandbox`: **11 lib + 13 integration + 1 doctest, all
  green**; `cargo fmt --check` and `cargo clippy -p oep-sandbox --all-targets
  -- -D warnings` clean.
- CPU cap: a real busy-spin `sbox-probe` under `cpu_time_seconds: 1` is
  killed by SIGXCPU/SIGKILL within a measured window (`cpu_cap_enforced_*`).
- RAM cap: `try_reserve` 4 GiB under a 64 MiB cap returns `ENOMEM`
  (worker reports `ALLOC_FAILED`); a 4 MiB control allocation succeeds.
- Disk cap: writing past a 1 MiB `RLIMIT_FSIZE` → EFBIG / SIGXFSZ; escape
  via disk flood fails.
- Egress decision table: simulated over the exact BPF instruction stream —
  `AF_INET`/`AF_INET6`/`AF_PACKET`/`AF_NETLINK` → `EPERM`, `AF_UNIX` → allow,
  unrelated syscalls → allow.
- Egress live: on hosts that permit nested seccomp (target worker hosts), the
  connect/raw-socket probes assert live `EPERM`; on hosts that forbid it
  (CI containers using Docker's default profile), the sandbox's fail-closed
  refusal is asserted instead (the 4 egress/escape tests passed via the
  fail-closed branch on the dev host, printed as `SKIP-live-filter`). No
  environment ever produced an uncaged worker.
- `tests/test_p9_m3_sandbox.py` (this card's repo-level acceptance driver)
  runs the crate suite and asserts each acceptance test name ran: green.

## 7. Handoff notes

- P9-M3-T01/T02 (workers): call `oep_sandbox::SandboxBuilder::new(limits)` and
  `run_command(argv)`; wire `SandboxOutcome::kill_reason` into the job broker.
- P9-M2-T02 (fair share): the `cpu.max`/`memory.max` hardener is the natural
  throttle seam; enable it via `cgroup_root` on supervisors with write access
  to a delegated cgroup v2 tree.
- P9-M4-T01 (GPU): `SandboxLimits` has no GPU fields — add them there when
  the GPU scheduler lands; `gpu` in the job spec stays with the scheduler.
- P11-T01 (threat model): §5 above is the authoritative worker-boundary
  statement to fold into the P11 document.
- Q: why is egress not also enforced at a firewall netns by default? Because
  a netns requires privileges a plain worker host may not grant the spawner;
  seccomp works unprivileged on every intended host and is fail-closed. Where
  root is available, add the netns layer per §5(2).
