# Contributor build path proof (CONTRIBUTOR-BUILD-PATH, card t_845b253d)

Date: 2026-10-27 (PDT). Evidence directory for the claim in
`docs/contributing/build.md` that a new contributor can build and test this
repository without asking anyone.

## What was proven, and how

Everything was measured on a CLEAN container built from
`.devcontainer/Dockerfile` (`isolith-devcontainer`, Ubuntu 24.04 ARM64 base)
with `--rm`, no cached toolchain, no cargo registry, no OCCT. The host was
U1 (20 cores, 121 GB RAM). All gate runs used the proven commands a
contributor runs; nothing was special-cased.

1. The devcontainer image builds from the Dockerfile: 118 s, 1.61 GB.
   See `image-build.log`.
2. A fresh `cargo xtask ci` (the whole gate) runs inside a clean container
   and passes every leg: rustfmt, xtask rustfmt, workspace clippy (deny
   warnings), xtask clippy, unit tests, integration tests, pytest (542
   passed, 28 skipped). First-run wall clock: 173 s, exit 0, gate PASS.
   See `gate-clean-container.log`.
3. Pytest alone on a clean source tree, empty HOME and no machine
   credentials: 542 passed, 28 skipped.
4. The OCCT tier (only for geometry-kernel changes) works clean:
   `cargo xtask occt` / `scripts/kernel/build-occt.sh` fetched and built the
   pinned OCCT 8.0.1 in 137 s (885 MB cache total, 169 MB static install),
   then `kernel_seam` (16 tests) and `kernel_replay` (9 tests) both green
   against real OCCT. See `kernel-tier-clean-proof.log`.

## Measured numbers (recorded in build.md)

| Quantity | Value |
| --- | --- |
| Devcontainer image build | 118 s, 1.61 GB |
| First `cargo xtask ci` in a clean container | 173 s, gate PASS (exit 0) |
| `uv sync` (pinned python + pytest) | 2 s |
| `cargo xtask check-sysdeps` | ~2 s |
| OCCT fetch + static build (20 cores) | 137 s |
| OCCT cache total / static install | 885 MB / 169 MB |
| Kernel seam + replayer test runs | 16 + 9 passed |
| pytest full suite inside the clean container | 542 passed, 28 skipped, 95.95 s |
| Cargo registry inside the container | 0.2 GB |
| Workspace target dir (full workspace + test binaries) | 12 GB |

## Real failures found and fixed while proving (not cosmetic)

- `xtask/Cargo.toml` needed its own empty `[workspace]` table. Without it
  cargo auto-discovery treated the dev tool as an implied member of the root
  workspace and a clean `cargo xtask` failed on a fresh machine
  ("current package believes it's in a workspace when it's not"). This is
  exactly the class of first-error a contributor bounces on.
- `python/oep_tooling/release_train.py` resolved the canary remote URL from
  `~/.git-credentials` BEFORE checking whether a working hermetic `canary`
  remote already existed, so the release-cut acceptance test failed on any
  machine with no credentials, and a hermetic local cut was impossible to
  prove. Moved the credentials lookup inside the branches that need it. The
  p12 hermetic release-cut test now passes with an empty HOME (6/6).

## Environment note

Measurements are for linux/arm64 (the devcontainer builds natively on both
arm64 and x86-64; the Dockerfile derives the rustup/uv arch from `uname -m`).
Expect similar but faster wall clocks on a comparable x86-64 machine and
slower on fewer cores; the numbers are the honest floor for the reference
host, not a promise for every laptop.
