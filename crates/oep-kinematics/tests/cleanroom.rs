//! P7-M3-T02 clean-room invariant test.
//!
//! The demo crate is an **own implementation with no vendor libraries**: it
//! must never link, vendor, embed or reference vendor CAD/kinematics
//! implementations, and must carry no prebuilt artifacts. This test turns that
//! invariant into an executable gate:
//!
//! 1. The crate's declared runtime dependencies are exactly `{ oep-geo }`
//!    (a path dependency). `serde_json` is a dev-dependency for the
//!    trace-parsing test only.
//! 2. No **code** in any source file references a forbidden vendor geometry /
//!    kinematics stack (OCCT, FreeCAD, CGAL, nalgebra, ...). Documentation
//!    comments are allowed to name them (the whole crate's docs explain the
//!    decision), so only non-comment code is scanned.
//! 3. No prebuilt artifact (DLL/SO/DYLIB/LIB/OBJ/A/O) and no vendored
//!    third-party directory lives inside the crate.
//!
//! The forbid-list deliberately includes the would-be OSS solver candidates:
//! the point of P7-M3-T02 is a dependency-free reference demo. The production
//! solver adoption is P7-M3-T01's separate ADR and does not touch this crate.

const FORBIDDEN: &[&str] = &[
    "opencascade",
    "occ",
    "cascade",
    "occt",
    "freecad",
    "cadcraft",
    "cgal",
    "nalgebra",
    "parry",
    "kiss3d",
    "cgmath",
    "ultraviolet",
    "vek",
    "glam",
    "corefoundation",
];

const FORBIDDEN_ARTIFACT_EXT: &[&str] =
    &["dll", "so", "dylib", "lib", "obj", "a", "o", "pdb", "rlib"];

/// Walk `dir`, collecting all file paths (no dirs).
fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read dir") {
        let p = entry.expect("entry").path();
        if p.is_dir() {
            walk(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// Pull the `[dependencies]` block out of a Cargo.toml as raw text.
fn dependencies_section(toml: &str) -> &str {
    let start = toml.find("[dependencies]").expect("has [dependencies]");
    let rest = &toml[start + "[dependencies]".len()..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    rest[..end].trim()
}

#[test]
fn declared_runtime_dependencies_are_exactly_oep_geo() {
    let toml = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let section = dependencies_section(&toml);
    // Lines: "oep-geo = { path = ... }" and nothing else under [dependencies].
    let keys: Vec<&str> = section
        .lines()
        .filter(|l| l.trim().contains('=') && !l.trim().starts_with('#'))
        .map(|l| l.trim().split('=').next().unwrap().trim())
        .collect();
    assert_eq!(
        keys,
        vec!["oep-geo"],
        "runtime deps must be exactly [oep-geo]; got {keys:?}"
    );
    assert!(
        section.contains("path"),
        "[dependencies] must be a path dep, not a registry dep"
    );
}

#[test]
fn source_code_has_no_vendor_geometry_or_kinematics_references() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let mut files = Vec::new();
    walk(root, &mut files);
    let mut hits = Vec::new();
    for f in files {
        if f.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&f).expect("read source");
        for (ln, raw) in text.lines().enumerate() {
            // Cut everything after a line comment: docs may name the stacks.
            let code = raw.split("//").next().unwrap_or("");
            let lower = code.to_ascii_lowercase();
            for token in FORBIDDEN {
                if lower.contains(token) {
                    hits.push(format!("{}:{}: {}", f.display(), ln + 1, raw.trim()));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "forbidden vendor references in crate code:\n{}",
        hits.join("\n")
    );
}

#[test]
fn no_prebuilt_artifacts_or_vendored_trees_inside_the_crate() {
    let root = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR")));
    let mut files = Vec::new();
    walk(root, &mut files);
    let mut bad = Vec::new();
    for f in files {
        let rel = f.strip_prefix(root).unwrap();
        if rel.starts_with("target") {
            continue;
        }
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let dot = name.rfind('.');
        if let Some(pos) = dot {
            let e = name[pos + 1..].to_ascii_lowercase();
            if FORBIDDEN_ARTIFACT_EXT.contains(&e.as_str()) {
                bad.push(format!("{}: prebuilt artifact ({e})", rel.display()));
            }
        }
        if f.extension().and_then(|e| e.to_str()) == Some("rs") && rel.starts_with("src") {
            let text = std::fs::read_to_string(&f).unwrap_or_default();
            if text.contains("extern crate") {
                bad.push(format!("{}: uses `extern crate`", rel.display()));
            }
        }
        let comps: Vec<&str> = rel
            .components()
            .map(|c| c.as_os_str().to_str().unwrap_or(""))
            .collect();
        for vend in ["third_party", "third-party", "vendor", "3rdparty"] {
            if comps.contains(&vend) {
                bad.push(format!("{}: vendored third-party dir", rel.display()));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "clean-room violations in crate:\n{}",
        bad.join("\n")
    );
}
