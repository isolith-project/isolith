//! P7-M5-T02: the help index is generated from command metadata, and the
//! workbench command surface meets the accessibility items that are checkable
//! on a headless keyboard/CLI surface.
//!
//! Asserted here (each maps to a row in workstreams/modeler/quality/accessibility-checklist.md):
//!
//! - WB-A11Y-01 registry covers every dispatchable command (help discoverability),
//! - WB-A11Y-02 every command is documented (summary, args, examples) and the
//!   committed help index is byte-identical to a fresh render from the
//!   metadata (generated, drift-guarded),
//! - WB-A11Y-03 rendered help carries no ANSI color escapes (no color-only
//!   encoding),
//! - WB-A11Y-04 an unknown command produces an actionable error naming the
//!   token and pointing at `help` (predictable syntax),
//! - WB-A11Y-05 the index serializes to JSON covering every command
//!   (machine-readable for assistive tooling and agents).

use std::fs;
use std::path::PathBuf;

use oep_shell::commands::{self, CommandSpec};

/// The canonical set of command names the workbench dispatches.
const CANONICAL_NAMES: &[&str] = &[
    "load:cgr",
    "load:3dxml",
    "load:step",
    "open-pdm",
    "save-pdm",
    "save",
    "assem:open",
    "assem:instances",
    "assem:mates",
    "assem:mate-add",
    "assem:mate-edit",
    "assem:mate-del",
    "assem:solve",
    "assem:render",
    "assem:animate",
    "journal",
    "help",
];

/// The canonical set of dispatch tokens (the text before the first ':').
const CANONICAL_TOKENS: &[&str] = &[
    "load", "open-pdm", "save-pdm", "save", "assem", "journal", "help",
];

fn committed_index_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("re-notes")
        .join("oep-workbench-help-index.md")
}

#[test]
fn registry_covers_every_dispatchable_command() {
    let specs: Vec<CommandSpec> = commands::commands().to_vec();
    assert!(!specs.is_empty(), "registry must not be empty");
    for name in CANONICAL_NAMES {
        assert!(
            specs.iter().any(|c| c.name == *name),
            "registry is missing command {name}"
        );
    }
    let mut tokens: Vec<&str> = specs.iter().map(|c| c.token).collect();
    tokens.sort_unstable();
    tokens.dedup();
    let mut expected: Vec<&str> = CANONICAL_TOKENS.to_vec();
    expected.sort_unstable();
    assert_eq!(tokens, expected, "dispatch tokens must match exactly");
}

#[test]
fn every_command_is_documented() {
    for c in commands::commands() {
        assert!(!c.summary.trim().is_empty(), "{}: missing summary", c.name);
        assert!(
            !c.long_help.trim().is_empty(),
            "{}: missing long help",
            c.name
        );
        assert!(!c.usage.trim().is_empty(), "{}: missing usage", c.name);
        assert!(
            !c.category.trim().is_empty(),
            "{}: missing category",
            c.name
        );
        assert!(!c.examples.is_empty(), "{}: missing examples", c.name);
        for a in c.args {
            assert!(!a.name.trim().is_empty(), "{}: unnamed argument", c.name);
            assert!(
                !a.help.trim().is_empty(),
                "{}: undocumented arg {}",
                c.name,
                a.name
            );
        }
        assert!(
            commands::render_help(c.name).is_some(),
            "{}: help renders",
            c.name
        );
    }
}

#[test]
fn rendered_help_has_no_ansi_color_escapes() {
    let renders = [
        commands::render_index(),
        commands::render_index_markdown(),
        commands::render_index_json(),
    ];
    for (i, text) in renders.iter().enumerate() {
        assert!(
            !text.contains('\u{001b}'),
            "render {i} contains an ANSI escape (help must not be color-only)"
        );
    }
    for c in commands::commands() {
        let h = commands::render_help(c.name).unwrap();
        assert!(
            !h.contains('\u{001b}'),
            "{} per-command help contains an ANSI escape",
            c.name
        );
    }
}

#[test]
fn json_index_covers_all_commands_and_is_stable() {
    let json = commands::render_index_json();
    assert!(
        json.contains(&format!("\"version\":\"{}\"", commands::HELP_INDEX_VERSION)),
        "json index carries the metadata version"
    );
    for name in CANONICAL_NAMES {
        assert!(
            json.contains(&format!("\"name\":\"{name}\"")),
            "json index is missing {name}"
        );
    }
    assert_eq!(
        json,
        commands::render_index_json(),
        "json index must be byte-stable"
    );
}

#[test]
fn help_index_generated_from_metadata_is_committed() {
    let generated = commands::render_index_markdown();
    let committed = fs::read_to_string(committed_index_path()).unwrap_or_else(|_| {
        panic!(
            "missing committed help index {} (regenerate with: \
             cargo run --example workbench -- --write-help-index \
             docs/re-notes/oep-workbench-help-index.md)",
            committed_index_path().display()
        )
    });
    assert_eq!(
        committed, generated,
        "committed help index drifted from command metadata (regenerate with \
         --write-help-index); help must be GENERATED, never hand-edited"
    );
}

#[test]
fn unknown_command_error_points_to_help() {
    let err = commands::validate_token("bogus").unwrap_err();
    assert!(err.contains("bogus"), "error names the token: {err}");
    assert!(
        err.to_lowercase().contains("help"),
        "error points at help: {err}"
    );
    assert_eq!(commands::validate_token("load"), Ok("load"));
    assert!(
        commands::render_help("bogus").is_none(),
        "bogus command has no help"
    );
}
