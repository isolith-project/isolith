//! P7-M5-T02: command metadata and help-index generation for the workbench
//! command surface.
//!
//! The workbench is a keyboard/CLI-first surface (P7-M5 power-user pass): every
//! command a user can type is described HERE, once. The interactive `help`
//! output, the `help <command>` per-command docs, the machine-readable JSON
//! index, and the committed markdown help index are all GENERATED from this
//! metadata, never hand-written in two places. A drift-guard test
//! (`crates/oep-shell/tests/help_index.rs`) fails if the committed index and
//! this registry diverge, so an undocumented command cannot be added. Missing
//! metadata is a test failure by construction.
//!
//! Accessibility contract (rows WB-A11Y-01..05 in
//! `workstreams/modeler/quality/accessibility-checklist.md`): every command is discoverable
//! and documented, rendered help carries no ANSI color escapes (nothing is
//! conveyed by color alone), an unknown command fails with an actionable error
//! that names the token and points at `help`, and the index serializes to JSON
//! for assistive tooling and agent automation.

/// Version tag embedded in every generated help artifact and asserted by the
/// drift-guard test. Bump when the help surface changes materially.
pub const HELP_INDEX_VERSION: &str = "1.0.0";

/// Label of the surface this index documents (used in the JSON and markdown
/// artifacts).
pub const SURFACE_LABEL: &str = "oep-shell workbench command surface";

/// One named argument of a command, indexed in the help.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArgSpec {
    /// Argument token as shown in the usage line, e.g. "PATH".
    pub name: &'static str,
    /// Whether the argument is required (vs optional, e.g. a summary).
    pub required: bool,
    /// One-line description of what the argument carries.
    pub help: &'static str,
}

/// One command the workbench accepts, described for the help index.
#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    /// Canonical command name used by `help:NAME`, e.g. "load:cgr".
    pub name: &'static str,
    /// The first token the dispatcher matches on (text before the first ':').
    pub token: &'static str,
    /// The canonical usage line, e.g. "load:cgr:PATH".
    pub usage: &'static str,
    /// Help-index group, e.g. "interchange".
    pub category: &'static str,
    /// One-line summary shown in the command index.
    pub summary: &'static str,
    /// Longer per-command help body shown by `help:NAME`.
    pub long_help: &'static str,
    /// Named arguments (empty when the command takes none).
    pub args: &'static [ArgSpec],
    /// Example invocations (each a bare command line, no binary prefix).
    pub examples: &'static [&'static str],
}

/// The command metadata registry: the single source of truth for the help
/// surface. Order is index order within each category.
pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "load:cgr",
        token: "load",
        usage: "load:cgr:PATH",
        category: "interchange",
        summary: "Load a CGR file (classic .cgr or modern V5_CFV2) into the workbench",
        long_help: concat!(
            "Reads CGR bytes and runs them through the importer hub ",
            "(oep-import). Both eras are latched: the classic polygon-soup .cgr ",
            "extractor and the modern V5_CFV2 spatial-mesh decoder. The loaded ",
            "canonical document becomes the current model."
        ),
        args: &[ArgSpec {
            name: "PATH",
            required: true,
            help: "Path to the CGR file (classic .cgr or modern V5_CFV2)",
        }],
        examples: &["load:cgr:nist_stc_06_part.cgr"],
    },
    CommandSpec {
        name: "load:3dxml",
        token: "load",
        usage: "load:3dxml:PATH",
        category: "interchange",
        summary: "Load a 3DXML file (ZIP or standalone XML) into the workbench",
        long_help: concat!(
            "Reads 3DXML bytes and runs them through the importer hub. ",
            "Both the ZIP container and the standalone XML rep are accepted; ",
            "the loaded canonical document becomes the current model."
        ),
        args: &[ArgSpec {
            name: "PATH",
            required: true,
            help: "Path to the 3DXML file (ZIP or standalone .3dxml XML)",
        }],
        examples: &["load:3dxml:cube1.3dxml"],
    },
    CommandSpec {
        name: "load:step",
        token: "load",
        usage: "load:step:PATH",
        category: "interchange",
        summary: "Load an ISO 10303-21 STEP file into the workbench",
        long_help: concat!(
            "Reads STEP (ISO 10303-21) bytes through the vendor-neutral ",
            "oep-step lane and maps the faceted part onto the canonical ",
            "oep-geo model. The loaded document becomes the current model."
        ),
        args: &[ArgSpec {
            name: "PATH",
            required: true,
            help: "Path to the ISO 10303-21 STEP file",
        }],
        examples: &["load:step:box-reference.step"],
    },
    CommandSpec {
        name: "save",
        token: "save",
        usage: "save:cgr|3dxml|step",
        category: "interchange",
        summary: "Request an interchange save lane (honest pending where no writer exists yet)",
        long_help: concat!(
            "Calls the interchange save lane for the named format. ",
            "Writers land on their own lanes (3DXML writer P3-M4, ",
            "STEP/OBJ/glTF exporters P6-M4); until a writer reaches this ",
            "stack the command answers with a typed InterchangeSavePending ",
            "that names the owning lane. It never fabricates output."
        ),
        args: &[ArgSpec {
            name: "FORMAT",
            required: true,
            help: "One of cgr, 3dxml or step",
        }],
        examples: &["save:step"],
    },
    CommandSpec {
        name: "open-pdm",
        token: "open-pdm",
        usage: "open-pdm:ITEM_ID:REVISION",
        category: "pdm",
        summary: "Open a revision from the PDM vault and make it the current document",
        long_help: concat!(
            "Reads a vault revision through the P8 PdmClient contract ",
            "(today the in-memory local stub), restores the canonical oep-geo ",
            "JSON and makes it the current document."
        ),
        args: &[
            ArgSpec {
                name: "ITEM_ID",
                required: true,
                help: "Vault item id",
            },
            ArgSpec {
                name: "REVISION",
                required: true,
                help: "Vault revision to open",
            },
        ],
        examples: &["open-pdm:BOX-01:A"],
    },
    CommandSpec {
        name: "save-pdm",
        token: "save-pdm",
        usage: "save-pdm:ITEM_ID:REVISION[:SUMMARY]",
        category: "pdm",
        summary: "Publish the current document as a new PDM vault revision",
        long_help: concat!(
            "Serializes the current canonical document to oep-geo JSON ",
            "and publishes it as a new revision through the P8 PdmClient ",
            "contract. The summary is optional free-form change text; a save ",
            "with no document loaded fails with a clear message."
        ),
        args: &[
            ArgSpec {
                name: "ITEM_ID",
                required: true,
                help: "Vault item id",
            },
            ArgSpec {
                name: "REVISION",
                required: true,
                help: "Vault revision to write",
            },
            ArgSpec {
                name: "SUMMARY",
                required: false,
                help: "Optional free-form change summary",
            },
        ],
        examples: &["save-pdm:BOX-01:A:initial"],
    },
    CommandSpec {
        name: "journal",
        token: "journal",
        usage: "journal",
        category: "inspection",
        summary: "Print the ordered operation journal",
        long_help: concat!(
            "Prints every action outcome recorded on the workbench ",
            "journal for this process, oldest first. The journal is the ",
            "observable, machine-callable record of the session."
        ),
        args: &[],
        examples: &["journal"],
    },
    CommandSpec {
        name: "help",
        token: "help",
        usage: "help [COMMAND]",
        category: "help",
        summary: "Print this command index, or the full help for one command",
        long_help: concat!(
            "With no argument, prints the full command index grouped by ",
            "category. With a command name (for example help:load:cgr) prints ",
            "that command's full documentation: usage, arguments, description ",
            "and examples. Prefix --json for the machine-readable index, ",
            "--markdown for the generated markdown document, or ",
            "--write-help-index PATH to regenerate the committed help index."
        ),
        args: &[ArgSpec {
            name: "COMMAND",
            required: false,
            help: "Command name to document, e.g. load:cgr",
        }],
        examples: &["help", "help:load:cgr", "help:--json"],
    },
];

/// The full command registry (the metadata single source of truth).
pub fn commands() -> &'static [CommandSpec] {
    COMMANDS
}

/// Look a command up by its canonical name (e.g. "load:cgr").
pub fn find(name: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|c| c.name == name)
}

/// Look a command up by its dispatch token (text before the first ':').
pub fn find_by_token(token: &str) -> Option<&'static CommandSpec> {
    COMMANDS.iter().find(|c| c.token == token)
}

/// Validate a dispatch token against the registry.
///
/// Returns the token unchanged when it is a known command token, otherwise an
/// actionable error that names the token and points at `help` (accessible
/// failure for an unknown command).
pub fn validate_token(token: &str) -> Result<&'static str, String> {
    match find_by_token(token) {
        Some(spec) => Ok(spec.token),
        None => Err(format!(
            "unknown command '{token}': run 'help' for the full command index"
        )),
    }
}

/// The help-index categories in display order.
pub fn categories() -> [&'static str; 4] {
    ["interchange", "pdm", "inspection", "help"]
}

/// Render the plain-text command index (the `help` output).
pub fn render_index() -> String {
    let mut out = String::new();
    out.push_str("oep-workbench: OEP interchange workbench command surface (P7-M5-T02)\n");
    out.push_str(&format!("help-index-version: {HELP_INDEX_VERSION}\n\n"));
    out.push_str("usage: oep-workbench <command> [<command> ...]\n\n");
    for category in categories() {
        let specs: Vec<&CommandSpec> = COMMANDS.iter().filter(|c| c.category == category).collect();
        if specs.is_empty() {
            continue;
        }
        out.push_str(&format!("{category}:\n"));
        for c in specs {
            out.push_str(&format!("  {:<32} {}\n", c.usage, c.summary));
        }
        out.push('\n');
    }
    out.push_str("For full per-command help run: oep-workbench help:COMMAND\n");
    out
}

/// Render the full documentation of one command, or `None` when the name is
/// not a known command.
pub fn render_help(name: &str) -> Option<String> {
    let c = find(name)?;
    let mut out = String::new();
    out.push_str(&format!("{}\n", c.name));
    out.push_str(&format!("  usage:     {}\n", c.usage));
    out.push_str(&format!("  category:  {}\n", c.category));
    out.push('\n');
    out.push_str(c.long_help);
    out.push_str("\n\nArguments:\n");
    if c.args.is_empty() {
        out.push_str("  (none)\n");
    } else {
        for a in c.args {
            let req = if a.required { "required" } else { "optional" };
            out.push_str(&format!("  - {} ({req}): {}\n", a.name, a.help));
        }
    }
    out.push_str("\nExamples:\n");
    for e in c.examples {
        out.push_str(&format!("  oep-workbench {e}\n"));
    }
    Some(out)
}

/// Render the machine-readable index (JSON, std-only, deterministic).
pub fn render_index_json() -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{{\"version\":\"{HELP_INDEX_VERSION}\",\"surface\":\"{}\",\"commands\":[",
        escape_json(SURFACE_LABEL)
    ));
    for (i, c) in COMMANDS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"name\":\"{}\",\"token\":\"{}\",\"usage\":\"{}\",\"category\":\"{}\",\"summary\":\"{}\",\"long_help\":\"{}\",\"args\":[",
            escape_json(c.name),
            escape_json(c.token),
            escape_json(c.usage),
            escape_json(c.category),
            escape_json(c.summary),
            escape_json(c.long_help),
        ));
        for (j, a) in c.args.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"name\":\"{}\",\"required\":{},\"help\":\"{}\"}}",
                escape_json(a.name),
                a.required,
                escape_json(a.help)
            ));
        }
        out.push_str("],\"examples\":[");
        for (j, e) in c.examples.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push_str(&format!("\"{}\"", escape_json(e)));
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

/// Render the generated markdown help index (committed to
/// `docs/re-notes/oep-workbench-help-index.md` and drift-guarded by test).
pub fn render_index_markdown() -> String {
    let mut out = String::new();
    out.push_str("# OEP workbench command help index (generated)\n\n");
    out.push_str(concat!(
        "This file is GENERATED from the command metadata in ",
        "`crates/oep-shell/src/commands.rs` (the single source of truth for ",
        "the workbench help surface). Do not hand-edit. Regenerate and keep ",
        "the committed copy in sync with:\n\n"
    ));
    out.push_str("```\n");
    out.push_str(concat!(
        "cargo run --example workbench -- --write-help-index ",
        "docs/re-notes/oep-workbench-help-index.md\n"
    ));
    out.push_str("```\n\n");
    out.push_str(&format!("help-index-version: {HELP_INDEX_VERSION}\n\n"));
    out.push_str(&format!("surface: {SURFACE_LABEL}\n\n"));
    out.push_str("## Index\n\n");
    out.push_str("| Command | Usage | Category | Summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for c in COMMANDS {
        out.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            c.name, c.usage, c.category, c.summary
        ));
    }
    out.push_str("\n## Commands\n\n");
    for c in COMMANDS {
        out.push_str(&format!("### {}\n\n", c.name));
        out.push_str(&format!("Usage: `{}`\n\n", c.usage));
        out.push_str(&format!("Category: {}\n\n", c.category));
        out.push_str(c.long_help);
        out.push_str("\n\nArguments:\n\n");
        if c.args.is_empty() {
            out.push_str("- (none)\n\n");
        } else {
            for a in c.args {
                let req = if a.required { "required" } else { "optional" };
                out.push_str(&format!("- `{}` ({}): {}\n", a.name, req, a.help));
            }
            out.push('\n');
        }
        out.push_str("Examples:\n\n");
        for e in c.examples {
            out.push_str(&format!("- `oep-workbench {e}`\n"));
        }
        out.push('\n');
    }
    out
}

/// Escape the small JSON-relevant subset (`"` and `\`) without dependencies.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(ch),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_and_json_cover_all_committed_commands() {
        assert_eq!(render_index(), render_index());
        for c in COMMANDS {
            assert!(render_index().contains(c.usage));
            assert!(render_index_json().contains(&format!("\"name\":\"{}\"", c.name)));
        }
    }
}
