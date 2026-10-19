//! SOLVER-REPLACE license clean-room invariant test.
//!
//! Program decision #16 (`workstreams/P7-M3/solver-adr.md`) replaced the vendored
//! GPL-3.0 SolveSpace engine with a permissive engine behind this crate's
//! vendor-neutral seam. This test turns the license consequence into an
//! executable gate so no GPL-3.0 dependency can silently return:
//!
//! 1. The crate's declared runtime dependencies are exactly
//!    `{ oep-geo, oep-kinematics, serde, serde_json }`. `serde_json` is also
//!    a dev-dependency for the fixture tests. Adding any third-party solver
//!    (or anything else) fails this gate.
//! 2. The workspace working tree contains no SolveSpace tree: nothing under
//!    the workspace root is named after the vendored engine and no
//!    `third_party/` directory exists inside the crate.
//! 3. The workspace `Cargo.lock` contains no package that names the GPL
//!    engines (SolveSpace, `slvs`, `solverang` is Apache-2.0 but excluded
//!    here for the same reason we did not integrate it) and no package whose
//!    name carries a GPL marker.
//!
//! The crate's own documentation is allowed to name SolveSpace and the ADR
//! (the decision record explains why it was removed); the gate scans paths,
//! manifest dependency names and lockfile entries, not prose.

const ALLOWED_RUNTIME_DEPS: &[&str] = &["oep-geo", "oep-kinematics", "serde", "serde_json"];

const FORBIDDEN_LEAF: &[&str] = &["solvespace", "slvs", "solverang", "third_party"];

/// Collect relative paths under `dir`, skipping build/output noise.
fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let Ok(entry) = entry else { continue };
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "target" || name == ".git" {
            continue;
        }
        let rel = p
            .strip_prefix(dir)
            .unwrap_or(&p)
            .to_string_lossy()
            .to_string();
        if p.is_dir() {
            out.push(rel.clone());
            walk(&p, out);
        } else {
            out.push(rel);
        }
    }
}

fn workspace_root(manifest_dir: &std::path::Path) -> std::path::PathBuf {
    // crates/oep-assem -> crates -> workspace root.
    manifest_dir
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

/// Pull the `[dependencies]` block out of a Cargo.toml as raw text.
fn dependencies_section(toml: &str) -> &str {
    let start = toml.find("[dependencies]").expect("has [dependencies]");
    let rest = &toml[start + "[dependencies]".len()..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    rest[..end].trim()
}

#[test]
fn runtime_dependencies_are_only_the_two_owned_crates_and_serde() {
    let manifest = include_str!("../Cargo.toml");
    let deps = dependencies_section(manifest);
    for line in deps.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let key = line.split_once('=').map(|(k, _)| k.trim()).unwrap_or(line);
        // Values may be paths or versions; only the key matters for the gate.
        let key = key.trim().trim_matches('"');
        assert!(
            ALLOWED_RUNTIME_DEPS.contains(&key),
            "dependency '{key}' is not in the allowed runtime set {ALLOWED_RUNTIME_DEPS:?}"
        );
    }
}

#[test]
fn no_vendored_tree_and_no_gpl_engine_named_anywhere_in_the_workspace() {
    let root = workspace_root(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    let mut paths = Vec::new();
    walk(&root, &mut paths);
    for rel in &paths {
        let low = rel.to_ascii_lowercase();
        for bad in FORBIDDEN_LEAF {
            assert!(
                !low.contains(bad),
                "workspace contains a forbidden path component: '{rel}' (matches '{bad}')"
            );
        }
    }
    // Belt and braces: the crate directory must not even contain a
    // third_party directory.
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !crate_dir.join("third_party").exists(),
        "crates/oep-assem/third_party must not exist"
    );
}

#[test]
fn cargo_lock_contains_no_gpl_solver_engine() {
    let root = workspace_root(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
    let lock = root.join("Cargo.lock");
    let text = std::fs::read_to_string(&lock).expect("workspace Cargo.lock present");
    let low = text.to_ascii_lowercase();
    for bad in ["solvespace", "slvs", "solverang"] {
        assert!(
            !low.contains(bad),
            "Cargo.lock must not reference the GPL/non-integrated engine '{bad}'"
        );
    }
    // Every locked package name is conventional (crates.io-style) and carries
    // no GPL marker; this guards the exact wording of the acceptance.
    assert!(
        !low.contains("gpl"),
        "Cargo.lock must contain no package/version carrying a GPL marker"
    );
}
