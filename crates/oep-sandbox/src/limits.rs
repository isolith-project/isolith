//! Sandbox limits: the hard caps the sandbox enforces on a worker.
//!
//! Field names and bounds mirror the P9 job-spec `ResourceCaps`
//! (P9-M1-T01, `crates/oep-job/src/spec.rs`) by value, plus one sandbox-only
//! field: `cpu_time_seconds` — the RLIMIT_CPU hard ceiling, because RLIMIT
//! cannot express "millicores" (it counts CPU *time*, not share).
//! `SandboxLimits::conservative_default()` is the same default the job spec
//! carries, so a job that declares nothing is still run bounded.

/// Maximum CPU request in millicores accepted for [`SandboxLimits::cpu_millicores`].
pub const MAX_CPU_MILLICORES: u64 = 64_000;
/// Maximum memory request in MiB (1 TiB), [`SandboxLimits::memory_mib`].
pub const MAX_MEMORY_MIB: u64 = 1_048_576;
/// Maximum scratch-disk request in MiB (8 TiB), [`SandboxLimits::disk_mib`].
pub const MAX_DISK_MIB: u64 = 8_388_608;
/// Maximum hard CPU-time ceiling in seconds (7 days),
/// [`SandboxLimits::cpu_time_seconds`] — mirrors the job-spec timeout bound.
pub const MAX_CPU_TIME_SECONDS: u64 = 604_800;
/// Maximum wall-clock timeout in seconds (7 days), [`SandboxLimits::timeout_seconds`].
pub const MAX_TIMEOUT_SECONDS: u64 = 604_800;

/// The caps a single sandboxed worker runs under.
///
/// `cpu_millicores` and `memory_mib` are required (the same contract as the
/// P9 job spec); everything else is optional with conservative semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxLimits {
    /// CPU *request* in millicores (thousandths of a core). Informational for
    /// the RLIMIT core (see crate docs); consumed as the proportional request
    /// when a cgroup throttle is configured (P9-M2-T02 / P9-M4 feed).
    pub cpu_millicores: u64,
    /// RAM cap in MiB: enforced as RLIMIT_AS + RLIMIT_DATA bytes.
    pub memory_mib: u64,
    /// Optional scratch-disk cap in MiB: enforced as RLIMIT_FSIZE bytes.
    pub disk_mib: Option<u64>,
    /// Hard CPU-time ceiling in seconds (RLIMIT_CPU); the runaway-worker
    /// backstop. `None` = no RLIMIT_CPU (still bounded by [`Self::timeout_seconds`]).
    pub cpu_time_seconds: Option<u64>,
    /// `false` (default): no network egress — the seccomp filter denies any
    /// non-`AF_UNIX` `socket(2)`. `true`: egress permitted (filter skipped).
    pub network_egress: bool,
    /// Wall-clock timeout for the run; the runner kills the worker when
    /// exceeded. `None` = no wall-clock cap.
    pub timeout_seconds: Option<u64>,
}

impl SandboxLimits {
    /// CPU-only, egress-blocked, one-hour-bounded cap of 1000 millicores /
    /// 2 GiB — the conservative default for jobs that do not state otherwise,
    /// matching `oep-job`'s `ResourceCaps::conservative_default`.
    pub fn conservative_default() -> Self {
        SandboxLimits {
            cpu_millicores: 1000,
            memory_mib: 2048,
            disk_mib: None,
            cpu_time_seconds: Some(3600),
            network_egress: DEFAULT_NETWORK_EGRESS,
            timeout_seconds: Some(3600),
        }
    }

    /// A disk-capped variant of the conservative default (used by tests and
    /// scratch-heavy workers).
    pub fn with_disk_mib(mut self, disk_mib: u64) -> Self {
        self.disk_mib = Some(disk_mib);
        self
    }

    /// Bounds-check the limits; returns every violated rule as a `&'static str`.
    /// Does not panic (P8 doctrine: validate structurally, reject nothing at
    /// construction).
    pub fn violations(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if !(1..=MAX_CPU_MILLICORES).contains(&self.cpu_millicores) {
            v.push("cpu_millicores out of bounds");
        }
        if !(1..=MAX_MEMORY_MIB).contains(&self.memory_mib) {
            v.push("memory_mib out of bounds");
        }
        if let Some(d) = self.disk_mib {
            if !(1..=MAX_DISK_MIB).contains(&d) {
                v.push("disk_mib out of bounds");
            }
        }
        if let Some(c) = self.cpu_time_seconds {
            if !(1..=MAX_CPU_TIME_SECONDS).contains(&c) {
                v.push("cpu_time_seconds out of bounds");
            }
        }
        if let Some(t) = self.timeout_seconds {
            if !(1..=MAX_TIMEOUT_SECONDS).contains(&t) {
                v.push("timeout_seconds out of bounds");
            }
        }
        v
    }

    /// True when the limits are structurally valid.
    pub fn is_valid(&self) -> bool {
        self.violations().is_empty()
    }
}

use crate::DEFAULT_NETWORK_EGRESS;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conservative_default_is_egress_blocked_cpu_only_1h() {
        let d = SandboxLimits::conservative_default();
        assert!(d.is_valid());
        assert_eq!(d.cpu_millicores, 1000);
        assert_eq!(d.memory_mib, 2048);
        assert_eq!(d.disk_mib, None);
        assert!(!d.network_egress, "egress must be blocked by default");
        assert_eq!(d.timeout_seconds, Some(3600));
    }

    #[test]
    fn violations_report_every_bad_bound() {
        let bad = SandboxLimits {
            cpu_millicores: 0,
            memory_mib: 0,
            disk_mib: Some(0),
            cpu_time_seconds: Some(0),
            network_egress: false,
            timeout_seconds: Some(0),
        };
        assert!(!bad.is_valid());
        assert_eq!(bad.violations().len(), 5);
    }

    #[test]
    fn valid_limits_report_no_violations() {
        let l = SandboxLimits {
            cpu_millicores: 2000,
            memory_mib: 4096,
            disk_mib: Some(512),
            cpu_time_seconds: Some(120),
            network_egress: true,
            timeout_seconds: Some(300),
        };
        assert!(l.is_valid());
        assert!(l.violations().is_empty());
    }
}
