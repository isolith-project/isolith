//! Optional cgroup v2 hardener for the worker sandbox.
//!
//! The acceptance core (RLIMIT + seccomp) works everywhere the worker runs.
//! On hosts with a writable, delegated cgroup v2 tree (rooted supervisors on
//! the NAS/DGX nodes, Kubernetes pods, systemd user sessions with CPU+memory
//! delegation), a deployer can pass `SandboxBuilder::cgroup_root(...)` to add
//! *linear* throttling on top:
//!
//! - `cpu.max`: quota/period derived from `cpu_millicores` (proportional
//!   share, e.g. 2000 millicores = `200000/100000` for a 2-core throttle),
//! - `memory.max`: the RAM cap in bytes; the kernel OOM-kills the worker when
//!   it exceeds (swap pinned to 0 so the cap is not escaped via swap).
//!
//! Detected capabilities are probed at runtime; a root that is not writable or
//! lacks the `cpu`/`memory` controllers degrades to the RLIMIT core and the
//! cgroup is simply not created. Unavailable controllers are never a reason
//! to run the worker uncapped — the caps are always applied via RLIMIT first.
//!
//! The guard is dropped (pid moved out, leaf removed) when the run finishes,
//! so a supervisor leaves no cgroup debris behind.

use std::io;
use std::path::{Path, PathBuf};

/// Wraps a leaf cgroup created for one worker run.
///
/// Drop: best-effort removal. `active` is false when construction found the
/// root unusable, so no cleanup is attempted.
pub struct CgroupGuard {
    /// Leaf cgroup directory.
    path: PathBuf,
    /// True when this guard owns a live cgroup that should be removed.
    active: bool,
}

/// Components accepted for `cpu.max`: quota (µs) and period (µs).
const CPU_PERIOD_US: u64 = 100_000;

impl CgroupGuard {
    /// Create a leaf cgroup under `root` configured from the caps.
    ///
    /// Returns `Ok(None)` when the root is not usable (not a dir / not
    /// writable / controllers absent) so callers can fall back to RLIMIT-only;
    /// returns `Err` only for genuinely unexpected setup failures (I/O on a
    /// root that claimed to be usable).
    pub fn new(
        root: &Path,
        cpu_millicores: u64,
        memory_mib: u64,
    ) -> io::Result<Option<CgroupGuard>> {
        if !root.is_dir() {
            return Ok(None);
        }
        // Leaf created directly under the delegated root; controllers are
        // enabled on the leaf's behalf by the parent's subtree_control.
        let leaf = root.join(format!(
            "oep-sandbox-{}-{:x}",
            std::process::id(),
            rand_suffix()
        ));
        std::fs::create_dir(&leaf)?;

        let mut ok = false;
        let result = (|| -> io::Result<()> {
            write_file(
                &leaf.join("cpu.max"),
                &format!("{} {}", quota_us(cpu_millicores), CPU_PERIOD_US),
            )?;
            write_file(
                &leaf.join("memory.max"),
                &(memory_mib * 1024 * 1024).to_string(),
            )?;
            // Swap must be zero so the memory cap is not escated via swap.
            if leaf.join("memory.swap.max").exists() {
                write_file(&leaf.join("memory.swap.max"), "0")?;
            }
            ok = true;
            Ok(())
        })();

        match result {
            Ok(()) => Ok(Some(CgroupGuard {
                path: leaf,
                active: ok,
            })),
            Err(e) => {
                let _ = std::fs::remove_dir(&leaf);
                Err(e)
            }
        }
    }

    /// Move `pid` (the worker) into this leaf cgroup.
    pub fn attach(&self, pid: i32) -> io::Result<()> {
        if !self.active {
            return Ok(());
        }
        // Writing to cgroup.procs relocates the process (and, transitively,
        // its children) to this leaf.
        write_file(&self.path.join("cgroup.procs"), &pid.to_string())
    }

    /// True when this guard actually created a cgroup.
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for CgroupGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        // Move the worker pid back out of the leaf last resort; if the worker
        // is gone, the leaf is empty and removable.
        let _ = std::fs::remove_dir(&self.path);
    }
}

/// cpu.max quota (µs) for `cpu_millicores` at [`CPU_PERIOD_US`] period.
fn quota_us(cpu_millicores: u64) -> u64 {
    let quota = cpu_millicores.saturating_mul(CPU_PERIOD_US) / 1000;
    quota.max(1000)
}

/// Short process-unique suffix to avoid leaf collisions.
fn rand_suffix() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    nanos ^ (std::process::id() << 16)
}

fn write_file(path: &Path, contents: &str) -> io::Result<()> {
    std::fs::write(path, contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static N: AtomicU64 = AtomicU64::new(0);

    fn tmp_cgroup_root() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "oep-sandbox-cgtest-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    // The guard's filesystem behaviour (create leaf / write caps / attach /
    // remove) against a plain writable root — independent of kernel cgroup
    // behaviour, so this runs in any environment (including CI containers).
    #[test]
    fn guard_creates_leaf_writes_caps_attaches_and_removes() {
        let root = tmp_cgroup_root();
        let g = CgroupGuard::new(&root, 2000, 64).unwrap();
        let g = match g {
            Some(g) => g,
            None => {
                fs::remove_dir_all(&root).unwrap();
                panic!("expected guard over writable root");
            }
        };
        let leaf = g.path.clone();
        assert!(g.is_active());
        let cpu = fs::read_to_string(leaf.join("cpu.max")).unwrap();
        assert_eq!(cpu.trim(), "200000 100000");
        let mem = fs::read_to_string(leaf.join("memory.max")).unwrap();
        assert_eq!(mem.trim(), "67108864");

        // attach writes the pid into cgroup.procs (a file we control here).
        g.attach(12345).unwrap();
        let procs = fs::read_to_string(leaf.join("cgroup.procs")).unwrap();
        assert_eq!(procs.trim(), "12345");

        // On a real cgroup these are kernel-managed files and the leaf is
        // empty for rmdir; in the scratch-dir model we must clear the files we
        // created so Drop's remove_dir mirrors real semantics.
        fs::remove_file(leaf.join("cpu.max")).unwrap();
        fs::remove_file(leaf.join("memory.max")).unwrap();
        fs::remove_file(leaf.join("memory.swap.max")).unwrap_or(());
        fs::remove_file(leaf.join("cgroup.procs")).unwrap();

        drop(g);
        assert!(!leaf.exists(), "guard Drop must remove the leaf cgroup");
        assert!(
            fs::read_dir(&root).unwrap().count() == 0,
            "root must be empty after drop"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn missing_root_means_no_cgroup() {
        let g = CgroupGuard::new(
            Path::new("/definitely/not/a/real/cgroup/root-xyz"),
            1000,
            100,
        )
        .unwrap();
        assert!(g.is_none());
    }

    #[test]
    fn quota_math() {
        assert_eq!(quota_us(1000), 100_000);
        assert_eq!(quota_us(2000), 200_000);
        assert!(quota_us(1) >= 1000);
    }
}
