//! RLIMIT-based resource caps, applied in the child before `exec`.
//!
//! These are hard, kernel-enforced ceilings on the worker's *own* resource
//! usage. They are applied with `setrlimit(2)` in a `pre_exec` hook, survive
//! `exec`, and cannot be raised by the worker (a non-root process can only
//! lower hard limits).

use std::io;

/// Returns the memory cap in bytes for the RLIMIT_AS/RLIMIT_DATA ceilings.
fn memory_bytes(memory_mib: u64) -> u64 {
    memory_mib.saturating_mul(1024 * 1024)
}

/// Apply the sandbox's RLIMIT ceilings to the calling process.
///
/// Only lowers limits (safe for the child pre-exec). On non-Linux targets this
/// is a no-op; the Linux sandbox geometry is intentional.
#[cfg(target_os = "linux")]
unsafe fn set_hard(resource: libc::c_uint, value: libc::rlim_t) -> io::Result<()> {
    let lim = libc::rlimit {
        rlim_cur: value,
        rlim_max: value,
    };
    if libc::setrlimit(resource, &lim) != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Apply the sandbox's RLIMIT ceilings to the calling process.
///
/// Only lowers limits (safe for the child pre-exec). On non-Linux targets this
/// is a no-op; the Linux sandbox geometry is intentional.
///
/// # Safety
///
/// Call in the forked-but-not-yet-execed child only (a `pre_exec` hook): it
/// installs hard limits on this process that persist across `exec` and cannot
/// be raised again, permanently bounding the process that runs next.
#[cfg(target_os = "linux")]
pub unsafe fn apply_limits(limits: &crate::SandboxLimits) -> io::Result<()> {
    use libc::{RLIMIT_AS, RLIMIT_CPU, RLIMIT_DATA, RLIMIT_FSIZE, RLIMIT_NOFILE};

    // CPU: hard ceiling on CPU seconds (a worker cannot hog the machine).
    if let Some(secs) = limits.cpu_time_seconds {
        set_hard(RLIMIT_CPU, secs as libc::rlim_t)?;
    }
    // RAM: hard ceiling on address space AND data segment (kernel >= 4.7 also
    // counts anonymous mmap against RLIMIT_DATA). Allocation past the cap
    // fails with ENOMEM instead of running away.
    let mem = memory_bytes(limits.memory_mib);
    set_hard(RLIMIT_AS, mem as libc::rlim_t)?;
    set_hard(RLIMIT_DATA, mem as libc::rlim_t)?;
    // Disk: hard ceiling on any single file's size (scratch writes past the
    // cap fail EFBIG / SIGXFSZ).
    if let Some(disk_mib) = limits.disk_mib {
        set_hard(RLIMIT_FSIZE, (disk_mib * 1024 * 1024) as libc::rlim_t)?;
    }
    // Bounded fd table so a worker cannot exhaust the node's descriptors.
    set_hard(RLIMIT_NOFILE, 1024 as libc::rlim_t)?;

    Ok(())
}

/// Non-Linux placeholder: no RLIMIT application (the sandbox geometry is
/// Linux; the runner degrades to a plain spawn elsewhere).
///
/// # Safety
///
/// Same child-only contract as the Linux variant; here it simply returns `Ok`.
#[cfg(not(target_os = "linux"))]
pub unsafe fn apply_limits(_limits: &crate::SandboxLimits) -> io::Result<()> {
    Ok(())
}

// NOTE: no in-process RLIMIT unit tests here. setrlimit applies to the whole
// test process, so a low cap (e.g. RLIMIT_CPU=1) would kill the harness, not
// just one test. Behavioural RLIMIT coverage lives in
// `tests/sandbox_limits.rs`, where each cap is applied to a separate worker
// child (`examples/sbox_probe.rs`).
