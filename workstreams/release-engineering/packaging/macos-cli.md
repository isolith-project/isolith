# Isolith OEP macOS install (command line only) — 2027.1

Status: DOCUMENTED SOURCE BUILD — **NOT TESTED on macOS in this card.**
No dmg is produced, there is no Developer ID signing, no notarisation, and no
Apple developer account is used. macOS delivers a command-line install only.

## Why "untested" is stated plainly

The 2027.1.0 desktop packaging card ran on U1 (aarch64 Linux) with the garage
Windows PC as the only other verified run target. No macOS machine was
available to execute a macOS build during the card, and cross-compiling
`.app`/dmg tooling from this host was not proven reliable. Per the card
rule "if no Mac is available to verify the build, mark macOS untested in the
docs rather than implying support", the macOS path is documented but marked
untested. **The download page must not claim macOS support for 2027.1.0.**

## What a macOS user gets (documented, untested)

These are the CLI binaries, built from source with the exact toolchain below.
There is no desktop `.app`, no bundle, no icon-grid integration.

| Tool              | Source target                              | Resulting binary name |
|-------------------|--------------------------------------------|-----------------------|
| Editor/interchange shell | `crates/oep-shell` example `workbench` | `oep-workbench` |
| Update checker    | `crates/oep-update` bin                    | `oep-update-check`     |
| Batch convert     | `crates/oep-worker` bin                    | `oep-batch-convert`    |
| CGR→glTF CLI      | `crates/oep-cgr` bin                       | `oep-cgr2gltf`         |
| CGR→STEP CLI      | `crates/oep-cgr` bin                       | `oep-cgr2step`         |
| PDM server (opt)  | `crates/oep-pdm` bin                       | `oep-pdm`              |

macOS is a CLI install only: the PDM server and workers ship via the Docker
distribution (DOCKER-NAS-DEPLOYMENT), not as a mac installer.

## Exact toolchain for a source build (what a maintainer runs on a Mac)

Workspace: `workstreams/release-engineering/packaging/`. Host prerequisites:

- Rust toolchain: `rustup` stable (pinned in `rust-toolchain.toml` in the repo
  root; this card verified `rustc 1.98.1` on the build host).
- No Xcode signing identity, no codesign, no `xcrun notarytool` — none are used.
- Cargo with the offline lock (`cargo build --release --offline`).

Per-binary commands (run from the repo root, architecture auto-detected):

```sh
# editor / interchange shell binary
cargo build --release --offline -p oep-shell --example workbench
cp target/release/examples/workbench target/release/oep-workbench

cargo build --release --offline --bin oep-update-check
cargo build --release --offline --bin oep-batch-convert
cargo build --release --offline --bin oep-cgr2gltf
cargo build --release --offline --bin oep-cgr2step
# optional (PDM, for a local single-host stack)
cargo build --release --offline --bin oep-pdm
```

Resulting binaries land in `target/release/` and are installed to a directory
on `PATH`, e.g. `install -m0755 target/release/oep-workbench /usr/local/bin/`.

## Why no macOS tarball ships in 2027.1

A tarball is only produced "if cross-building proves reliable." On this card
the only macOS-capable toolchain would be a Rust cross-target
(`aarch64-apple-darwin` / `x86_64-apple-darwin`) compiled from Linux, which
needs the Apple linker (not present) or `zig`/`osxcross` w/ SDK — none of
which are established in this release toolchain, and none of which could be
run on a Mac to verify. Shipping that artifact would imply support that was
not exercised. **2027.1.0 ships no macOS artifact; the download page says so.**

## Download page language (exact text the card requires)

> macOS: source build documented in this repo (see packaging docs). No
> prebuilt macOS artifact ships in 2027.1.0. Windows and Linux binaries
> are unsigned today and Windows SmartScreen will warn on install.

## Follow-up

If a Mac becomes an accepted run target, the same `build-deb` / AppImage
payload set applies to a `build-macos.sh` producing a plain CLI tarball
(`oep-<bin>-darwin-<arch>.tar.gz`) built natively on the Mac; this card
deliberately does not claim that path.
