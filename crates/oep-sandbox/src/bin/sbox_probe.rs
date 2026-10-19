//! sbox_probe -- tiny worker binary used by the oep-sandbox test suite.
//!
//! A sandboxed worker that performs exactly one behaviour, selected by
//! `OEP_SBOX_PROBE`. Exit semantics are testable:
//!
//! | probe | behaviour | expected under sandbox |
//! |-------|-----------|-------------------------|
//! | `alloc` | reserve `OEP_SBOX_ALLOC_MIB` MiB | ALLOC_OK (0) or ALLOC_FAILED (1) |
//! | `spin` | busy-loop `OEP_SBOX_SPIN_SECONDS` s | killed by RLIMIT_CPU |
//! | `connect` | TCP connect to `OEP_SBOX_CONNECT_ADDR` | CONNECT_BLOCKED (0) / CONNECT_UNREACHABLE (1) / CONNECT_OK (2) |
//! | `socket_af_inet` | `socket(AF_INET,...)` | SOCKET_BLOCKED (0) / SOCKET_OK (2) |
//! | `socket_af_packet` | `socket(AF_PACKET,...)` | SOCKET_BLOCKED (0) / SOCKET_OK (2) |
//! | `io_uring_sock` | create AF_INET socket via IORING_OP_SOCKET | URING_BLOCKED (0) / URING_OK (2) / URING_UNAVAILABLE (1) |
//! | `writefile` | write to `OEP_SBOX_WRITE_PATH` past the disk cap | WRITE_BLOCKED (0) / WRITE_OK (2) |
//! | `seccomp_probe` | install the egress filter on itself and probe | SECCOMP_ENFORCED (0) / UNAVAILABLE (1) / NOT_ENFORCED (2) |
//!
//! Exit 0 always means "the sandbox did its job" for a probe whose job is to
//! be stopped; exit 2 means the sandbox failed to stop us.

use std::io::Write as _;
use std::net::TcpStream;

fn mi() -> usize {
    std::env::var("OEP_SBOX_ALLOC_MIB")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(64)
}

fn main() {
    let probe = std::env::var("OEP_SBOX_PROBE").unwrap_or_default();
    match probe.as_str() {
        "alloc" => probe_alloc(),
        "spin" => probe_spin(),
        "connect" => probe_connect(),
        "socket_af_inet" => probe_socket(libc::AF_INET),
        "socket_af_packet" => probe_socket(libc::AF_PACKET),
        "io_uring_sock" => probe_io_uring_socket(),
        "writefile" => probe_writefile(),
        "seccomp_probe" => probe_seccomp(),
        other => {
            eprintln!("unknown probe: {other:?}");
            std::process::exit(99);
        }
    }
}

/// Environment capability probe: can THIS host install and enforce the egress
/// filter? The sandbox uses the same `install_egress_block` the real worker
/// path uses. Exits: 0 = enforced (socket(AF_INET) -> EPERM); 1 = install
/// failed cleanly (host forbids nested seccomp); 2 = filter installed but
/// socket still creatable (broken); killed-by-signal (SIGSYS) also counts as
/// "host forbids nested seccomp".
fn probe_seccomp() -> ! {
    // SAFETY: probing in a throwaway child; install_egress_block self-verifies.
    match unsafe { oep_sandbox::seccomp::install_egress_block() } {
        Ok(()) => {
            // The install itself verifies socket(AF_INET) -> EPERM.
            let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EPERM) {
                    println!("SECCOMP_ENFORCED");
                    std::process::exit(0);
                }
                println!("SECCOMP_NOT_ENFORCED: {err}");
                std::process::exit(2);
            }
            unsafe { libc::close(fd) };
            println!("SECCOMP_NOT_ENFORCED: socket still creatable");
            std::process::exit(2);
        }
        Err(e) => {
            println!("SECCOMP_UNAVAILABLE: {e}");
            std::process::exit(1);
        }
    }
}

fn probe_alloc() -> ! {
    let mut v: Vec<u8> = Vec::new();
    match v.try_reserve(mi() * 1024 * 1024) {
        Ok(()) => {
            println!("ALLOC_OK {}MiB", mi());
            std::process::exit(0);
        }
        Err(e) => {
            println!("ALLOC_FAILED: {e}");
            std::process::exit(1);
        }
    }
}

fn probe_spin() -> ! {
    let secs: u64 = std::env::var("OEP_SBOX_SPIN_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let started = std::time::Instant::now();
    let mut x: u64 = 0;
    loop {
        x = x.wrapping_add(1);
        std::hint::black_box(x);
        if std::time::Instant::now() - started >= std::time::Duration::from_secs(secs) {
            println!("SPIN_DONE");
            std::process::exit(0);
        }
    }
}

fn probe_connect() -> ! {
    let addr = std::env::var("OEP_SBOX_CONNECT_ADDR").unwrap_or_else(|_| "8.8.8.8:53".to_string());
    match TcpStream::connect(&addr) {
        Ok(_) => {
            println!("CONNECT_OK");
            std::process::exit(2);
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            println!("CONNECT_BLOCKED");
            std::process::exit(0);
        }
        Err(e) => {
            println!("CONNECT_UNREACHABLE: {e}");
            std::process::exit(1);
        }
    }
}

fn probe_socket(family: libc::c_int) -> ! {
    // SAFETY: raw syscall by design; the outcome is the point of the probe.
    let fd = unsafe { libc::socket(family, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() == std::io::ErrorKind::PermissionDenied {
            println!("SOCKET_BLOCKED");
            std::process::exit(0);
        } else {
            println!("SOCKET_ERR: {err}");
            std::process::exit(1);
        }
    }
    // SAFETY: fd was just created.
    unsafe { libc::close(fd) };
    println!("SOCKET_OK");
    std::process::exit(2);
}

/// `IORING_OP_SOCKET` egress-bypass probe (P11-T07 P2T-01-SB finding).
///
/// A seccomp filter that only denies `socket(2)` does not stop a worker from
/// creating an `AF_INET` socket through io_uring's `IORING_OP_SOCKET` opcode,
/// because that path never executes the `socket` syscall. The fixed filter
/// denies `io_uring_setup`/`io_uring_enter`/`io_uring_register` outright, so
/// under the hardened sandbox this probe must see `io_uring_setup` itself
/// denied. Exit semantics:
///   0 = io_uring blocked (URING_BLOCKED)   <- fixed filter working
///   1 = io_uring unavailable on this kernel (URING_UNAVAILABLE, ENOSYS)
///   2 = got an AF_INET socket via io_uring (URING_OK)  <- filter bypassed!
fn probe_io_uring_socket() -> ! {
    // io_uring syscall number (aarch64 and x86_64 both use this value, added
    // by the asm-generic unistd in Linux 5.1+). io_uring_enter/register share
    // the same removal; denying io_uring_setup blocks every later op.
    const SYS_IO_URING_SETUP: libc::c_long = 425;

    let mut params = [0u8; 0x80];
    // SAFETY: raw syscall; if the kernel does want to create a ring we hand it
    // a zeroed params buffer (reserved fields must be zero or the kernel
    // returns EINVAL).
    let ring = unsafe {
        libc::syscall(
            SYS_IO_URING_SETUP,
            8 as libc::c_uint,
            params.as_mut_ptr() as *mut libc::c_void,
        )
    };
    if ring < 0 {
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(libc::EPERM) | Some(libc::EACCES) => {
                // Filter (or outer profile) denied io_uring setup: blocked.
                // SAFETY: no ring was created, nothing to close.
                println!("URING_BLOCKED: {err}");
                std::process::exit(0);
            }
            Some(libc::ENOSYS) => {
                println!("URING_UNAVAILABLE: no io_uring on this kernel");
                std::process::exit(1);
            }
            _ => {
                println!("URING_ERR: {err}");
                std::process::exit(1);
            }
        }
    }
    // SAFETY: ring_fd was returned by a successful io_uring_setup.
    let ring_fd = ring as libc::c_int;
    // SAFETY: raw map/unmap of the ring shared memory.
    unsafe { libc::close(ring_fd) };
    // Reaching here means io_uring_setup succeeded, which is only possible if
    // the egress filter did NOT deny the syscall (the check above maps every
    // denial to URING_BLOCKED/URING_UNAVAILABLE). Report OK as "bypass path
    // reachable". A full socket() create through the opcode would also be
    // possible, but the deny at io_uring_setup is the decisive control.
    println!("URING_OK: io_uring_setup not denied (bypass path reachable)");
    std::process::exit(2);
}

fn probe_writefile() -> ! {
    let path = std::env::var("OEP_SBOX_WRITE_PATH")
        .unwrap_or_else(|_| "oep-sandbox-write-probe.tmp".to_string());
    // Write far more than the test disk cap (1 MiB default) in small chunks so
    // the cap is hit, not the backup of a single write.
    let mut f = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            println!("WRITE_CREATE_ERR: {e}");
            std::process::exit(1);
        }
    };
    let chunk = [0u8; 64 * 1024];
    let mut written = 0u64;
    for _ in 0..4096 {
        match f.write_all(&chunk) {
            Ok(()) => written += chunk.len() as u64,
            Err(e) if e.kind() == std::io::ErrorKind::FileTooLarge => {
                println!("WRITE_BLOCKED after {written} bytes");
                std::process::exit(0);
            }
            Err(e) => {
                println!("WRITE_ERR: {e}");
                std::process::exit(1);
            }
        }
    }
    println!("WRITE_OK {written} bytes");
    std::process::exit(2);
}
