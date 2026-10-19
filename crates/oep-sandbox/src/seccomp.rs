//! Seccomp BPF filter blocking network egress.
//!
//! The filter denies `socket(2)` for every address family except `AF_UNIX`
//! (local IPC that never leaves the host). Because a fresh worker process
//! inherits no sockets at `exec`, blocking *creation* of non-`AF_UNIX`
//! sockets closes every ordinary egress path: there is no way to obtain an
//! `AF_INET`/`AF_INET6`/`AF_PACKET` descriptor, so `connect`/`sendto`/`bind`
//! cannot reach the network. Breach attempts return `EPERM` (testable: the
//! worker sees a permission error, not a hang or a success).
//!
//! The filter ALSO denies the io_uring subsystem (`io_uring_setup`,
//! `io_uring_enter`, `io_uring_register`). This closes a class of egress
//! bypass that no amount of `socket(2)` coverage stops: `IORING_OP_SOCKET`
//! creates a real `AF_INET` descriptor through the io_uring machinery, which
//! seccomp never sees because it is not the `socket(2)` syscall. A worker
//! that cannot call `sock_create` either directly or through io_uring has no
//! path to a network descriptor at all (P11-T07 pen-test finding P2T-01-SB).
//!
//! Installed in the child before `exec` with `PR_SET_NO_NEW_PRIVS` (so the
//! filter survives `exec` and cannot be dropped by a setuid binary), then
//! `PR_SET_SECCOMP, SECCOMP_MODE_FILTER`. Unprivileged, container-safe.

use std::io;

/// The default egress policy: `false` = no network egress (P9-M3-T03
/// acceptance: "egress blocked by default and testable"):
pub const DEFAULT_NETWORK_EGRESS: bool = false;

/// One classic BPF instruction (`struct sock_filter` from `linux/filter.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BpfInsn {
    /// The BPF opcode as a little-endian u16 (matches `sock_filter.code`).
    pub code: u16,
    /// Jump-if-true offset (matches `sock_filter.jt`).
    pub jt: u8,
    /// Jump-if-false offset (matches `sock_filter.jf`).
    pub jf: u8,
    /// Operand (matches `sock_filter.k`).
    pub k: u32,
}

// Classic BPF opcode fragments (linux/bpf_common.h values).
const BPF_LD: u16 = 0x00;
const BPF_W: u16 = 0x00;
const BPF_ABS: u16 = 0x20;
const BPF_JMP: u16 = 0x05;
const BPF_JEQ: u16 = 0x10;
const BPF_K: u16 = 0x00;
const BPF_RET: u16 = 0x06;

// seccomp(2) return actions (linux/seccomp.h).
const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;

// offsetof(struct seccomp_data, ...): nr@0, arch@4, args[0]@16.
const SECCOMP_OFF_NR: u32 = 0;
const SECCOMP_OFF_ARCH: u32 = 4;
const SECCOMP_OFF_ARGS0: u32 = 16;

/// `AF_UNIX` (local socketpair/DGRAM IPC) is the only family allowed to be
/// created by a worker.
const AF_UNIX: u32 = 1;

/// Audit-arch constant for the host (`EM_* | __AUDIT_ARCH_64BIT | LE`).
#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0xC000_003E; // EM_X86_64 (62)
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xC000_00B7; // EM_AARCH64 (183)

fn stmt(code: u16, k: u32) -> BpfInsn {
    BpfInsn {
        code,
        jt: 0,
        jf: 0,
        k,
    }
}

fn jump(code: u16, jt: u8, jf: u8, k: u32) -> BpfInsn {
    BpfInsn { code, jt, jf, k }
}

/// The compiled egress-blocking program, as a list of BPF instructions.
///
/// Decision table (instruction stream the kernel runs; verified by
/// [`simulate`], and the layout below is pinned by index so every jump target
/// is exact):
///
/// ```text
///  0  LD arch
///  1  JEQ AUDIT_ARCH  jt->3 (skip KILL), jf->2
///  2  KILL_PROCESS                      (foreign arch: unreachable)
///  3  LD syscall nr
///  4  JEQ __NR_socket jt->10 (family check), jf->5
///  5  JEQ io_uring_setup  jt->9 (DENY), jf->6
///  6  JEQ io_uring_enter  jt->9 (DENY), jf->7
///  7  JEQ io_uring_register jt->9 (DENY), jf->8
///  8  ALLOW                            (any other syscall)
///  9  ERRNO(EPERM)                     (io_uring syscalls denied)
/// 10  LD args[0]                       (socket family)
/// 11  JEQ AF_UNIX jt->13 (allow), jf->12
/// 12  ERRNO(EPERM)                     (non-AF_UNIX socket denied)
/// 13  ALLOW                            (AF_UNIX socket allowed)
/// ```
///
/// `io_uring_*` are denied outright because a worker has no legitimate need
/// for them and every one of their socket-creating operations would bypass a
/// filter that only watches `socket(2)`.
pub fn egress_filter_instructions() -> Vec<BpfInsn> {
    let sysnr_socket = socket_syscall_nr();
    let io_setup = io_uring_setup_syscall_nr();
    let io_enter = io_uring_enter_syscall_nr();
    let io_register = io_uring_register_syscall_nr();
    vec![
        // 0: load arch
        stmt(BPF_LD | BPF_W | BPF_ABS, SECCOMP_OFF_ARCH),
        // 1: if arch == AUDIT_ARCH -> insn 3 (skip the KILL), else insn 2
        jump(BPF_JMP | BPF_JEQ | BPF_K, 1, 0, AUDIT_ARCH),
        // 2: unknown arch: kill
        stmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),
        // 3: load syscall nr
        stmt(BPF_LD | BPF_W | BPF_ABS, SECCOMP_OFF_NR),
        // 4: if nr == socket -> insn 10 (family check); else next (5)
        jump(BPF_JMP | BPF_JEQ | BPF_K, 5, 0, sysnr_socket),
        // 5..7: if nr is any io_uring syscall -> insn 9 (EPERM); else next
        jump(BPF_JMP | BPF_JEQ | BPF_K, 3, 0, io_setup),
        jump(BPF_JMP | BPF_JEQ | BPF_K, 2, 0, io_enter),
        jump(BPF_JMP | BPF_JEQ | BPF_K, 1, 0, io_register),
        // 8: any other syscall is allowed
        stmt(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
        // 9: io_uring syscalls denied (closes IORING_OP_SOCKET egress bypass)
        stmt(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | libc::EPERM as u32),
        // 10: socket(): load args[0] (family)
        stmt(BPF_LD | BPF_W | BPF_ABS, SECCOMP_OFF_ARGS0),
        // 11: if family == AF_UNIX -> insn 13 (allow), else insn 12
        jump(BPF_JMP | BPF_JEQ | BPF_K, 1, 0, AF_UNIX),
        // 12: non-AF_UNIX socket -> EPERM
        stmt(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | libc::EPERM as u32),
        // 13: AF_UNIX socket -> allow
        stmt(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    ]
}

/// The `__NR_socket` value for the host, via libc's target-arch table.
fn socket_syscall_nr() -> u32 {
    libc::SYS_socket as u32
}

/// The `__NR_io_uring_setup` value for the host.
fn io_uring_setup_syscall_nr() -> u32 {
    libc::SYS_io_uring_setup as u32
}

/// The `__NR_io_uring_enter` value for the host.
fn io_uring_enter_syscall_nr() -> u32 {
    libc::SYS_io_uring_enter as u32
}

/// The `__NR_io_uring_register` value for the host.
fn io_uring_register_syscall_nr() -> u32 {
    libc::SYS_io_uring_register as u32
}

// ---------------------------------------------------------------------------
// Filter semantics, verified by simulation (the exact instruction stream the
// kernel would run). seccomp install is blocked by Docker's default profile
// and by locked-down dev sandboxes, so the *decision table* of the filter is
// proven here against synthetic `seccomp_data`, and the live kernel behaviour
// is covered by the fail-closed runner contract (never run an uncaged worker).
// ---------------------------------------------------------------------------

/// Minimal `struct seccomp_data` view the simulator needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeccompData {
    /// Intrinsic syscall number (`nr`).
    pub syscall: u32,
    /// `arch` field (`AUDIT_ARCH_*`).
    pub arch: u32,
    /// First syscall argument (for `socket(2)` this is the address family).
    pub args0: u64,
}

/// The simulator's result: the `SECCOMP_RET_*` action (or `None` when the
/// program ran past its end without a RET, which the kernel rejects anyway).
pub fn simulate(insns: &[BpfInsn], data: SeccompData) -> Option<u32> {
    let mut acc: Option<u32> = None;
    let mut pc: usize = 0;
    while let Some(insn) = insns.get(pc) {
        match insn.code {
            c if c == (BPF_LD | BPF_W | BPF_ABS) => {
                acc = Some(match insn.k {
                    SECCOMP_OFF_NR => data.syscall,
                    SECCOMP_OFF_ARCH => data.arch,
                    SECCOMP_OFF_ARGS0 => data.args0 as u32,
                    _ => 0,
                });
                pc += 1;
            }
            c if c == (BPF_JMP | BPF_JEQ | BPF_K) => {
                let a = acc?; // our filter always compares a just-loaded value
                let taken = a == insn.k;
                pc += 1 + if taken {
                    insn.jt as usize
                } else {
                    insn.jf as usize
                };
            }
            c if c == (BPF_RET | BPF_K) => return Some(insn.k),
            _ => return None, // interpreter covers exactly the emitted instruction set
        }
    }
    None
}

/// Assert a simulated run ends in the expected RET action.
pub fn simulated_action(insns: &[BpfInsn], syscall: u32, args0: u64) -> u32 {
    simulate(
        insns,
        SeccompData {
            syscall,
            arch: AUDIT_ARCH,
            args0,
        },
    )
    .expect("filter must always terminate in a RET")
}

/// Install the egress-blocking filter on the calling thread (child pre-exec)
/// and verify it took effect: creation of an `AF_INET` socket must now fail
/// with `EPERM`, and the io_uring syscalls must be denied (closing the
/// `IORING_OP_SOCKET` egress bypass). If the environment forbids installing
/// filters (Docker's default profile, locked-down sandboxes) the call fails
/// with an error — the caller's fail-closed contract then refuses to run the
/// worker uncaged.
///
/// # Safety
///
/// Call in the forked-but-not-yet-execed child only (a `pre_exec` hook); it
/// permanently attaches a seccomp filter to this process (survives `exec`)
/// and the self-verification printf-style probes that follow run inside the
/// filter. Never call from a long-lived process of your own.
///
/// Linux only. On other platforms this is a no-op that reports success so the
/// runner degrades to plain spawn.
#[cfg(target_os = "linux")]
pub unsafe fn install_egress_block() -> io::Result<()> {
    let insns = egress_filter_instructions();
    let prog = libc::sock_fprog {
        len: insns.len() as u16,
        filter: insns.as_ptr() as *mut libc::sock_filter,
    };

    if libc::prctl(
        libc::PR_SET_NO_NEW_PRIVS,
        1 as libc::c_ulong,
        0 as libc::c_ulong,
        0 as libc::c_ulong,
        0 as libc::c_ulong,
    ) != 0
    {
        return Err(io::Error::last_os_error());
    }
    if libc::prctl(
        libc::PR_SET_SECCOMP,
        libc::SECCOMP_MODE_FILTER as libc::c_ulong,
        &prog as *const _ as libc::c_ulong,
        0 as libc::c_ulong,
        0 as libc::c_ulong,
    ) != 0
    {
        return Err(io::Error::last_os_error());
    }
    // Self-verify: an AF_INET socket must now be denied. Fail closed if the
    // filter did not take effect (some vectors keep sockets creatable).
    let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
    if fd >= 0 {
        libc::close(fd);
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "egress filter installed but socket(AF_INET) still permitted",
        ));
    }
    if io::Error::last_os_error().raw_os_error() != Some(libc::EPERM) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "egress filter verification failed: socket(AF_INET) not denied with EPERM",
        ));
    }
    // Self-verify: io_uring_setup must be denied too (the IORING_OP_SOCKET
    // egress bypass). Fail closed if it is reachable and NOT denied. A kernel
    // without CONFIG_IO_URING returns ENOSYS — that is also fine (no io_uring
    // machinery exists, so the bypass is impossible). Only a reachable,
    // un-denied io_uring is a failure.
    // SAFETY: raw syscall in the throwaway child; the filter denies it with
    // EPERM before the kernel ever runs io_uring setup code. The params arg is
    // a 128-byte buffer io_uring fills on success (never reached under the
    // filter, which denies the syscall).
    let mut params = [0u8; 128];
    let ring = libc::syscall(
        libc::SYS_io_uring_setup,
        8 as libc::c_uint,
        params.as_mut_ptr() as *mut libc::c_void,
    );
    if ring > 0 {
        // The fd was returned: the filter did not block io_uring. Close it.
        let _ = libc::close(ring as libc::c_int);
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "egress filter installed but io_uring_setup still permitted (IORING_OP_SOCKET bypass)",
        ));
    }
    match io::Error::last_os_error().raw_os_error() {
        // Any denial errno (EPERM from our own filter, or EACCES/EPERM from an
        // outer profile the same precedence) proves io_uring is unreachable,
        // which is exactly the property we need: the bypass is closed.
        Some(libc::EPERM) | Some(libc::EACCES) => {}
        Some(libc::ENOSYS) => {}         // kernel has no io_uring: bypass impossible
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "egress filter verification failed: io_uring_setup not denied with EPERM",
            ))
        }
    }
    Ok(())
}

/// Non-Linux placeholder: no seccomp.
///
/// # Safety
///
/// Same child-only contract as the Linux variant; here it simply returns `Ok`.
#[cfg(not(target_os = "linux"))]
pub unsafe fn install_egress_block() -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn egress_blocked_by_default_flag() {
        const {
            assert!(!DEFAULT_NETWORK_EGRESS);
        }
    }

    #[test]
    fn filter_structure_is_sane() {
        let f = egress_filter_instructions();
        assert!(f.len() >= 13, "filter too small: {}", f.len());
        // First op loads the arch; EPERM-denial RETs and an ALLOW RET exist.
        assert_eq!(f[0].code, BPF_LD | BPF_W | BPF_ABS);
        let rets: Vec<u32> = f
            .iter()
            .filter(|i| i.code == (BPF_RET | BPF_K))
            .map(|i| i.k)
            .collect();
        let eperm = SECCOMP_RET_ERRNO | libc::EPERM as u32;
        assert!(rets.contains(&eperm));
        assert!(rets.contains(&SECCOMP_RET_ALLOW));
        // More than one denial path exists (socket + io_uring).
        assert!(rets.iter().filter(|k| **k == eperm).count() >= 2);
    }

    #[test]
    fn filter_denies_io_uring_syscalls() {
        let f = egress_filter_instructions();
        let eperm = SECCOMP_RET_ERRNO | libc::EPERM as u32;
        for nr in [
            libc::SYS_io_uring_setup as u32,
            libc::SYS_io_uring_enter as u32,
            libc::SYS_io_uring_register as u32,
        ] {
            let action = simulated_action(&f, nr, 0);
            assert_eq!(
                action, eperm,
                "io_uring syscall {nr} must be denied (IORING_OP_SOCKET egress bypass)"
            );
        }
    }

    // The filter's decision table, executed by the mini-interpreter over the
    // exact instruction stream the kernel would run. This is the security
    // property of the egress boundary, verified regardless of whether the
    // host permits installing the filter (Docker's default profile does not).
    #[test]
    fn filter_denies_every_non_unix_socket_family() {
        let f = egress_filter_instructions();
        for family in [
            libc::AF_INET as u32,
            libc::AF_INET6 as u32,
            libc::AF_PACKET as u32,
            libc::AF_NETLINK as u32,
            libc::AF_UNIX as u32, // covered explicitly below for contrast
        ] {
            let action = simulated_action(&f, socket_syscall_nr(), family as u64);
            if family == libc::AF_UNIX as u32 {
                assert_eq!(action, SECCOMP_RET_ALLOW, "AF_UNIX must stay allowed");
            } else {
                assert_eq!(
                    action,
                    SECCOMP_RET_ERRNO | libc::EPERM as u32,
                    "family {family} must be denied at socket(2)",
                );
            }
        }
    }

    #[test]
    fn filter_allows_other_syscalls() {
        let f = egress_filter_instructions();
        for syscall in [
            libc::SYS_read as u32,
            libc::SYS_write as u32,
            libc::SYS_mmap as u32,
        ] {
            let action = simulated_action(&f, syscall, 0);
            assert_eq!(action, SECCOMP_RET_ALLOW, "syscall {syscall} unaffected");
        }
    }

    #[test]
    fn filter_kills_on_unknown_arch() {
        let f = egress_filter_instructions();
        let action = simulate(
            &f,
            SeccompData {
                syscall: socket_syscall_nr(),
                arch: 0xDEAD_0000,
                args0: 0,
            },
        )
        .expect("must terminate");
        assert_eq!(action, SECCOMP_RET_KILL_PROCESS);
    }
}

