//! Build script for oep-engine-occ.
//!
//! With the `build-engine` feature off (the workspace default), this script is a
//! no-op and the crate compiles to a typed stub (every call returns
//! `EngineUnavailable`) so `cargo build --workspace` needs no C++ toolchain.
//!
//! With `build-engine` on, the script:
//!   1. Locates the pinned OCCT 8.0.x build in `${OEP_OCCT_ROOT}`
//!      (default `${HOME}/.cache/oep-occt/<version>/install`), running
//!      `scripts/kernel/build-occt.sh` to fetch+build it when absent. This is
//!      the "fetched/built as a dependency" path of ADR v1.1.0 section 9 --
//!      no OCCT source is ever vendored into the repo.
//!   2. Compiles the cxx bridge + the C++ glue with cxx-build.
//!   3. Links OCCT as STATIC archives (the `build-occt.sh` default) in strict
//!      dependency order (glue first, then OCCT consumers before the libs they
//!      reference), so every binary/test built anywhere in the workspace is
//!      self-contained -- no rpath, LD_LIBRARY_PATH, or system OCCT needed.

use std::env;
use std::path::PathBuf;
use std::process::Command;

/// Pinned OCCT line per ADR section 9 -- keep in sync with
/// scripts/kernel/fetch-occt.sh.
const OCCT_VERSION: &str = "8.0.1";

/// Canonical OCCT static-archive link order (consumers first, dependencies
/// last): the linker extracts from each archive in one pass to satisfy refs
/// recorded earlier, so a library must be listed after the archives that
/// reference it. Hand-maintained to match the pinned 8.0.1 module set built by
/// `build-occt.sh`; any lib outside the list fails the build loudly instead of
/// linking wrong.
#[cfg(feature = "build-engine")]
const OCCT_LINK_ORDER: &[&str] = &[
    "TKBO",
    "TKBool",
    "TKFillet",
    "TKFeat",
    "TKHLR",
    "TKOffset",
    "TKShHealing",
    "TKTopAlgo",
    "TKPrim",
    "TKGeomAlgo",
    "TKBRep",
    "TKMesh",
    "TKXMesh",
    "TKExpress",
    "TKHelix",
    "TKGeomBase",
    "TKG3d",
    "TKG2d",
    "TKMath",
    "TKernel",
];

fn repo_root() -> PathBuf {
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate root is two levels below the repo root")
        .to_path_buf()
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cpp/glue.cc");
    println!("cargo:rerun-if-changed=cpp/glue.hh");
    println!("cargo:rerun-if-env-changed=OEP_OCCT_ROOT");
    println!("cargo:rerun-if-env-changed=OEP_OCCT_VERSION");

    let build_engine = env::var("CARGO_FEATURE_BUILD_ENGINE").is_ok();
    if !build_engine {
        // Stub build: nothing to compile or link.
        return;
    }

    // --- 1. Ensure the pinned static OCCT install exists -------------------
    let version = env::var("OEP_OCCT_VERSION").unwrap_or_else(|_| OCCT_VERSION.to_string());
    let root = env::var("OEP_OCCT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").expect("HOME");
            PathBuf::from(home).join(".cache").join("oep-occt")
        });
    let install = if root.join("lib").join("cmake").join("opencascade").exists() {
        root
    } else {
        root.join(&version).join("install")
    };

    if !install
        .join("lib")
        .join("cmake")
        .join("opencascade")
        .exists()
    {
        eprintln!("oep-engine-occ: pinning OCCT {version} missing; building it (one time).");
        let script = repo_root().join("scripts/kernel/build-occt.sh");
        let status = Command::new("bash")
            .arg(&script)
            .env("OEP_OCCT_VERSION", &version)
            .env("OEP_OCCT_SHARED", "OFF")
            .env(
                "OEP_OCCT_ROOT",
                env::var("OEP_OCCT_ROOT").unwrap_or_default(),
            )
            .status()
            .expect("failed to launch scripts/kernel/build-occt.sh");
        if !status.success() {
            panic!(
                "oep-engine-occ: scripts/kernel/build-occt.sh failed (exit {status:?}); see the \
                 log above. Set OEP_OCCT_ROOT to an existing OCCT {version} install to skip the \
                 build."
            );
        }
    }

    // --- 2+3. cxx bridge + glue, and static OCCT link -----------------------
    // cfg-gated so the stub build (feature off) does not need the optional
    // cxx-build dependency to even compile this script.
    #[cfg(feature = "build-engine")]
    {
        let lib_dir = install.join("lib");
        let inc_dir = install.join("include").join("opencascade");
        if !lib_dir.exists() || !inc_dir.exists() {
            panic!(
                "oep-engine-occ: expected OCCT {version} static libraries at {} and headers at {}",
                lib_dir.display(),
                inc_dir.display()
            );
        }

        // --- 2. Compile cxx bridge + glue (emits the glue static archive) ---
        let cxx_path = repo_root().join("crates/oep-engine-occ");
        cxx_build::bridge("src/lib.rs")
            .file("cpp/glue.cc")
            .include(&cxx_path)
            .include(cxx_path.join("cpp"))
            .include(&inc_dir)
            .flag_if_supported("-std=c++20")
            .warnings(true)
            .compile("oep_engine_occ_glue");

        // --- 3. Link OCCT static archives (glue first, dependency order) ----
        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        // Verify the installed archive set matches the canonical list exactly,
        // so a bump that changes the module set is caught rather than linked
        // wrong.
        let mut on_disk: Vec<String> = Vec::new();
        for entry in std::fs::read_dir(&lib_dir).expect("read OCCT lib dir") {
            let path = entry.expect("entry").path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if let Some(base) = name.strip_prefix("lib") {
                if base.ends_with(".a") {
                    let stem = base.strip_suffix(".a").unwrap_or(base);
                    on_disk.push(stem.to_string());
                }
            }
        }
        for (i, want) in OCCT_LINK_ORDER.iter().enumerate() {
            if !on_disk.iter().any(|n| n == want) {
                panic!(
                    "oep-engine-occ: OCCT static archive lib{want}.a missing at {} (expected by \
                     OCCT_LINK_ORDER[{i}]); the pinned OCCT manifest changed",
                    lib_dir.display()
                );
            }
            println!("cargo:rustc-link-lib=static={want}");
        }
        let extra: Vec<&String> = on_disk
            .iter()
            .filter(|n| !OCCT_LINK_ORDER.contains(&n.as_str()))
            .collect();
        assert!(
            extra.is_empty(),
            "oep-engine-occ: OCCT install contains libraries outside OCCT_LINK_ORDER: {extra:?}; \
             extend the canonical list (topological order) in build.rs"
        );
    }
}
