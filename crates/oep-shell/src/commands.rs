//! P7-M5-T02: command metadata and help-index generation for the workbench
//! command surface.
//!
//! The workbench is a keyboard/CLI-first surface (P7-M5 power-user pass; the
//! INTERACTIVE-SHELL editor is its interactive sibling and uses the SAME
//! command registry): every command a user can type is described HERE, once.
//! The interactive `help`
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
/// drift-guard test. Bump when the help surface changes materially (1.3.0:
/// MEDIA-RENDERER adds optional WIDTH:HEIGHT and present overlays to sk:render).
pub const HELP_INDEX_VERSION: &str = "1.3.0";

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
        name: "assem:open",
        token: "assem",
        usage: "assem:open",
        category: "assembly",
        summary: "Open the committed 3-part assembly fixture into the interactive surface",
        long_help: concat!(
            "Loads the committed editor fixture (oep-assem golden: base plate, ",
            "cap, peg) plus its mate set into the assembly editing surface. ",
            "The fixture opens deliberately mis-placed and tilted so a live ",
            "solve has visible work to do. Instance placement is visible with ",
            "assem:instances and in the viewport with assem:render."
        ),
        args: &[],
        examples: &["assem:open"],
    },
    CommandSpec {
        name: "assem:instances",
        token: "assem",
        usage: "assem:instances",
        category: "assembly",
        summary: "List the assembly instances and their current placements",
        long_help: concat!(
            "Prints one line per assembly instance: id, source part id and the ",
            "world placement of the part origin. After a live solve the lines ",
            "reflect the solved placements (the instances follow the mate edit)."
        ),
        args: &[],
        examples: &["assem:open", "assem:solve", "assem:instances"],
    },
    CommandSpec {
        name: "assem:mates",
        token: "assem",
        usage: "assem:mates",
        category: "assembly",
        summary: "List the current mate set (indexed)",
        long_help: concat!(
            "Prints one line per mate in the constraint set with its index and ",
            "kind (fixed, coincident, on-plane, plane-distance, parallel). ",
            "Indices are what assem:mate-edit and assem:mate-del consume."
        ),
        args: &[],
        examples: &["assem:open", "assem:mates"],
    },
    CommandSpec {
        name: "assem:mate-add",
        token: "assem",
        usage: "assem:mate-add:KIND:ARGS...",
        category: "assembly",
        summary: "Create a mate and re-solve live; instances follow or a typed error reports",
        long_help: concat!(
            "Appends a mate to the constraint set and immediately re-solves the ",
            "assembly through the vendor-neutral seam. On a convergent solve the ",
            "report prints and the instances follow. On a diverging solve the ",
            "mate is kept, a typed error is reported (solver-failed and friends), ",
            "and the previous converged placement is left untouched. KIND plus ",
            "ARGS: fixed INSTANCE; coincident INSTA ANCHA INSTB ANCHB; on-plane ",
            "INST ANCH P_INST P_ANCH; plane-distance INST ANCH P_INST P_ANCH DIST; ",
            "parallel INSTA ANCHA INSTB ANCHB. Args are colon-separated."
        ),
        args: &[ArgSpec {
            name: "KIND:ARGS...",
            required: true,
            help: "Mate kind and its colon-separated arguments (see long help)",
        }],
        examples: &[
            "assem:open",
            "assem:mate-add:plane-distance:cap:origin:base:inX:30",
        ],
    },
    CommandSpec {
        name: "assem:mate-edit",
        token: "assem",
        usage: "assem:mate-edit:INDEX:KIND:ARGS...",
        category: "assembly",
        summary: "Replace a mate by index and re-solve live",
        long_help: concat!(
            "Replaces the mate at INDEX in the constraint set and re-solves live. ",
            "Same outcome rules as assem:mate-add. An out-of-range index is a ",
            "typed rejection."
        ),
        args: &[ArgSpec {
            name: "INDEX:KIND:ARGS...",
            required: true,
            help: "Mate index, then the replacement mate (same grammar as mate-add)",
        }],
        examples: &[
            "assem:open",
            "assem:mate-edit:2:plane-distance:cap:origin:base:inX:30",
        ],
    },
    CommandSpec {
        name: "assem:mate-del",
        token: "assem",
        usage: "assem:mate-del:INDEX",
        category: "assembly",
        summary: "Delete a mate by index and re-solve live",
        long_help: concat!(
            "Removes the mate at INDEX and re-solves live. Deleting a ",
            "constraint relaxes the assembly, so this normally converges and ",
            "the instances settle into the now-looser placement. Out-of-range ",
            "indices are typed rejections."
        ),
        args: &[ArgSpec {
            name: "INDEX",
            required: true,
            help: "Index into the mate set (see assem:mates)",
        }],
        examples: &["assem:open", "assem:mate-del:4", "assem:solve"],
    },
    CommandSpec {
        name: "assem:solve",
        token: "assem",
        usage: "assem:solve",
        category: "assembly",
        summary: "Solve the current mate set and write placements back",
        long_help: concat!(
            "Runs the assembly solver over the current mate set through the ",
            "vendor-neutral seam and, on success, writes the solved placements ",
            "into the document. Prints the report (dof, moved instances, ",
            "residuals). On divergence it prints a typed error and leaves the ",
            "document poses untouched."
        ),
        args: &[],
        examples: &["assem:open", "assem:solve"],
    },
    CommandSpec {
        name: "assem:render",
        token: "assem",
        usage: "assem:render:PATH",
        category: "assembly",
        summary: "Render the current assembly pose to a PPM image (the viewport)",
        long_help: concat!(
            "Renders the current instance placements as bounding-box proxies to ",
            "a P6 PPM file at PATH using the deterministic CPU raster. Prints ",
            "the image SHA-256 so two consecutive renders prove whether the ",
            "instances followed a solve."
        ),
        args: &[ArgSpec {
            name: "PATH",
            required: true,
            help: "Output PPM path",
        }],
        examples: &[
            "assem:open",
            "assem:render:before.ppm",
            "assem:solve",
            "assem:render:after.ppm",
        ],
    },
    CommandSpec {
        name: "assem:animate",
        token: "assem",
        usage: "assem:animate:FRAMES:OUT_DIR",
        category: "assembly",
        summary: "Play mechanism motion (crank-slider) and record a deterministic trace",
        long_help: concat!(
            "Drives the kinematics lane (P7-M3-T02 damped Gauss-Newton) through ",
            "a full crank turn of the committed crank-slider mechanism built ",
            "from its joint set, recording FRAMES deterministic per-frame poses ",
            "to OUT_DIR/trace.json plus rendered PPM frames and a manifest. ",
            "Prints the trace SHA-256: identical on every run, which is the ",
            "determinism proof."
        ),
        args: &[
            ArgSpec {
                name: "FRAMES",
                required: true,
                help: "Number of animation frames (one crank turn)",
            },
            ArgSpec {
                name: "OUT_DIR",
                required: true,
                help: "Output directory for trace.json, manifest.json and frames",
            },
        ],
        examples: &["assem:open", "assem:animate:36:demos/editor-assem-motion"],
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
        name: "editor:open",
        token: "editor",
        usage: "editor:open",
        category: "editor",
        summary: "Open a fresh interactive editor session (blank sketch document)",
        long_help: concat!(
            "Starts an EditorSession: a canonical oep-geo document with one part, ",
            "one body, an empty sketch feature and an empty variable set. All ",
            "sk:/vars:/feat:/view:/export: commands then operate on this session. ",
            "The same command set is what a mouse-driven editor calls under the ",
            "pointer; here it is text-callable (the agent/automation lane)."
        ),
        args: &[],
        examples: &["editor:open", "sk:point:0:0", "sk:solve"],
    },
    CommandSpec {
        name: "sk:point",
        token: "sk",
        usage: "sk:point:X:Y",
        category: "editor",
        summary: "Add a point at (X, Y) in the sketch plane; returns its id (p0, p1, ...)",
        long_help: concat!(
            "Appends a point at the given sketch-plane coordinates. The assigned ",
            "id is printed (p0, p1, ...) so later sk:line and sk:constrain commands ",
            "can reference it."
        ),
        args: &[
            ArgSpec {
                name: "X",
                required: true,
                help: "Sketch-plane x coordinate",
            },
            ArgSpec {
                name: "Y",
                required: true,
                help: "Sketch-plane y coordinate",
            },
        ],
        examples: &["sk:point:0:0", "sk:point:3:1"],
    },
    CommandSpec {
        name: "sk:line",
        token: "sk",
        usage: "sk:line:START:END",
        category: "editor",
        summary: "Add a line segment between two existing points; returns its id (l0, l1, ...)",
        long_help: concat!(
            "Appends a line segment from point START to point END. Both points ",
            "must exist. A closed loop of lines is what extrude/revolve consume."
        ),
        args: &[
            ArgSpec {
                name: "START",
                required: true,
                help: "Start point id (e.g. p0)",
            },
            ArgSpec {
                name: "END",
                required: true,
                help: "End point id (e.g. p1)",
            },
        ],
        examples: &["sk:line:p0:p1"],
    },
    CommandSpec {
        name: "sk:constrain",
        token: "sk",
        usage: "sk:constrain:KIND:ARGS...",
        category: "editor",
        summary: "Add a sketch constraint (fixed, horizontal, vertical, coincident, length, distance)",
        long_help: concat!(
            "Appends a constraint to the sketch. KIND and ARGS:\n",
            "  fixed:POINT:X:Y          anchor a point at (X,Y)\n",
            "  horizontal:LINE          force a line horizontal\n",
            "  vertical:LINE            force a line vertical\n",
            "  coincident:A:B           coincide two points\n",
            "  length:LINE:VAL          driven length (VAL = number, or a variable-set entry name)\n",
            "  distance:A:B:VAL         driven distance between two points (number or variable name)\n",
            "When VAL is not a number it references the variable set by name, so an ",
            "edit to the variable re-solves the sketch (drive lane)."
        ),
        args: &[ArgSpec {
            name: "KIND:ARGS...",
            required: true,
            help: "Constraint kind and colon-separated arguments (see long help)",
        }],
        examples: &["sk:constrain:fixed:p0:0:0", "sk:constrain:length:l0:width"],
    },
    CommandSpec {
        name: "sk:solve",
        token: "sk",
        usage: "sk:solve",
        category: "editor",
        summary: "Solve the sketch against the variable set and write solved coordinates back",
        long_help: concat!(
            "Runs the oep-sketch solver over the current sketch + variable set. On ",
            "convergence the solved point coordinates are written back into the ",
            "canonical document and the report (dof, residual, driven dimensions) ",
            "is printed. A non-converged solve is a typed error, never a fabricated ",
            "solution."
        ),
        args: &[],
        examples: &["sk:solve"],
    },
    CommandSpec {
        name: "sk:render",
        token: "sk",
        usage: "sk:render:PATH[:WIDTH:HEIGHT][:present]",
        category: "editor",
        summary:
            "Render the current viewport to PPM or PNG; WIDTH:HEIGHT selects a real resolution, present adds grid/axes/label overlays",
        long_help: concat!(
            "Renders the current scene through the deterministic CPU raster to PATH. ",
            ".ppm writes a P6 PPM, .png a PNG. The resolution is a REAL render, ",
            "never an upscale: without WIDTH:HEIGHT the session's configured ",
            "resolution is used (config default 640x480, overridable for the ",
            "session via the OEP_VIEWPORT_WIDTH / OEP_VIEWPORT_HEIGHT ",
            "environment variables). Appending 'present' forces the present-mode ",
            "overlays into the same raster: the world grid on the sketch plane, ",
            "the XYZ axes triad, and the dimension/selection labels sourced from ",
            "the resolved variable set (e.g. width = 80.000) and the current ",
            "selection. Prints the image SHA-256 so two consecutive renders prove ",
            "whether the model moved."
        ),
        args: &[
            ArgSpec {
                name: "PATH",
                required: true,
                help: "Output image path (.ppm or .png); must not contain ':'",
            },
            ArgSpec {
                name: "WIDTH",
                required: false,
                help: "Render width in pixels (a real render at that resolution)",
            },
            ArgSpec {
                name: "HEIGHT",
                required: false,
                help: "Render height in pixels",
            },
        ],
        examples: &[
            "sk:render:before.png",
            "sk:render:before.png:1920:1080",
            "sk:render:before.png:1920:1080:present",
        ],
    },
    CommandSpec {
        name: "sk:props",
        token: "sk",
        usage: "sk:props",
        category: "editor",
        summary: "Print the sketch summary (points, lines, constraints, solved dof)",
        long_help: concat!(
            "Prints the current sketch geometry and constraint counts plus the last ",
            "solve's degrees of freedom and residual, if any."
        ),
        args: &[],
        examples: &["sk:solve", "sk:props"],
    },
    CommandSpec {
        name: "vars:list",
        token: "vars",
        usage: "vars:list",
        category: "editor",
        summary: "List the variable-set entries (sorted by name)",
        long_help: concat!(
            "Prints every variable-set entry: name = value. Dimensions that ",
            "reference a variable by name resolve against this set at solve time."
        ),
        args: &[],
        examples: &["vars:list"],
    },
    CommandSpec {
        name: "vars:set",
        token: "vars",
        usage: "vars:set:NAME:VALUE",
        category: "editor",
        summary: "Set (or add) a variable; a solid feature recomputes live",
        long_help: concat!(
            "Sets the variable NAME to VALUE. If the session already carries a ",
            "solid feature (extrude or revolve), the model recomputes immediately: ",
            "the sketch re-solves and the solid follows the parameter (drive lane). ",
            "Prints the solid volume before and after so the update is provable."
        ),
        args: &[
            ArgSpec {
                name: "NAME",
                required: true,
                help: "Variable name",
            },
            ArgSpec {
                name: "VALUE",
                required: true,
                help: "Numeric value",
            },
        ],
        examples: &["vars:set:width:120"],
    },
    CommandSpec {
        name: "feat:extrude",
        token: "feat",
        usage: "feat:extrude:DEPTH",
        category: "editor",
        summary: "Create an extrude from the solved sketch profile by DEPTH (number or variable name)",
        long_help: concat!(
            "Derives the solid by extruding the solved closed sketch profile by ",
            "DEPTH (a literal length or a variable-set entry name) along +Z. The ",
            "solid is tessellated deterministically from the real model, the body ",
            "bounding box is set from it, and the volume/faces/bbox are printed. ",
            "Requires the sketch to be solved and to form a single closed loop."
        ),
        args: &[ArgSpec {
            name: "DEPTH",
            required: true,
            help: "Extrude depth (number or variable name)",
        }],
        examples: &["feat:extrude:10", "feat:extrude:depth"],
    },
    CommandSpec {
        name: "feat:revolve",
        token: "feat",
        usage: "feat:revolve:ANGLE",
        category: "editor",
        summary: "Create a revolve from the solved sketch profile by ANGLE radians about Z",
        long_help: concat!(
            "Derives the solid by revolving the solved closed profile about the Z ",
            "axis by ANGLE radians (a literal or a variable-set entry name). The ",
            "profile is a meridian cross section: x is radial distance (>= 0), y ",
            "axial height, matching the kernel seam's revolve."
        ),
        args: &[ArgSpec {
            name: "ANGLE",
            required: true,
            help: "Revolve angle in radians (number or variable name)",
        }],
        examples: &["feat:revolve:3.14159265"],
    },
    CommandSpec {
        name: "feat:recompute",
        token: "feat",
        usage: "feat:recompute",
        category: "editor",
        summary: "Re-solve the sketch and re-derive the solid feature from the current model",
        long_help: concat!(
            "Full recompute: re-solves the sketch against the current variable set, ",
            "rebuilds the closed profile, re-tessellates the solid feature, and ",
            "updates the body bounding box and viewport mesh. This is the explicit ",
            "form of the live recompute vars:set already triggers."
        ),
        args: &[],
        examples: &["feat:recompute"],
    },
    CommandSpec {
        name: "view:frame",
        token: "view",
        usage: "view:frame",
        category: "editor",
        summary: "Frame the camera to the model bounds",
        long_help: concat!(
            "Fits the orbit camera to the union of the solid and sketch bounds so ",
            "the next render shows the whole model."
        ),
        args: &[],
        examples: &["view:frame"],
    },
    CommandSpec {
        name: "view:orbit",
        token: "view",
        usage: "view:orbit:YAW:PITCH",
        category: "editor",
        summary: "Orbit the camera by YAW / PITCH (radians)",
        long_help: "Rotates the orbit camera around the target (the viewport orbit gesture).",
        args: &[
            ArgSpec {
                name: "YAW",
                required: true,
                help: "Azimuth delta in radians",
            },
            ArgSpec {
                name: "PITCH",
                required: true,
                help: "Elevation delta in radians",
            },
        ],
        examples: &["view:orbit:0.8:0.5"],
    },
    CommandSpec {
        name: "view:zoom",
        token: "view",
        usage: "view:zoom:FACTOR",
        category: "editor",
        summary: "Zoom the camera by a multiplicative FACTOR (< 1 moves closer)",
        long_help: "Scales the camera distance (the viewport zoom gesture).",
        args: &[ArgSpec {
            name: "FACTOR",
            required: true,
            help: "Multiplicative zoom factor",
        }],
        examples: &["view:zoom:0.8"],
    },
    CommandSpec {
        name: "view:pan",
        token: "view",
        usage: "view:pan:DX:DY",
        category: "editor",
        summary: "Pan the camera target by DX / DY world units",
        long_help: concat!(
            "Moves the orbit target along the camera right/up screen axes (the ",
            "viewport pan gesture)."
        ),
        args: &[
            ArgSpec {
                name: "DX",
                required: true,
                help: "Pan along the camera right axis",
            },
            ArgSpec {
                name: "DY",
                required: true,
                help: "Pan along the camera up axis",
            },
        ],
        examples: &["view:pan:5:-3"],
    },
    CommandSpec {
        name: "view:pick",
        token: "view",
        usage: "view:pick:X:Y",
        category: "editor",
        summary: "Pick the scene primitive under screen pixel (X, Y) and select it",
        long_help: concat!(
            "Unprojects the pixel through the camera and selects the nearest ",
            "primitive (the solid, a sketch line or a point handle). Prints the ",
            "selected id; the next render highlights it in amber."
        ),
        args: &[
            ArgSpec {
                name: "X",
                required: true,
                help: "Pixel x (0..639)",
            },
            ArgSpec {
                name: "Y",
                required: true,
                help: "Pixel y (0..479)",
            },
        ],
        examples: &["view:pick:320:240"],
    },
    CommandSpec {
        name: "export:step",
        token: "export",
        usage: "export:step:PATH",
        category: "editor",
        summary: "Export the derived solid as an AP214 STEP file",
        long_help: concat!(
            "Writes the tessellated solid as a STEP (ISO 10303-21) faceted B-Rep ",
            "via the oep-geo exporter. Requires a solid feature (extrude or ",
            "revolve). The exported bytes re-import through the importer hub."
        ),
        args: &[ArgSpec {
            name: "PATH",
            required: true,
            help: "Output .step path",
        }],
        examples: &["export:step:edpart.step"],
    },
    CommandSpec {
        name: "props",
        token: "props",
        usage: "props",
        category: "editor",
        summary: "Print current model measures (volume, bbox, features, sketch stats, variables)",
        long_help: concat!(
            "Prints the derived solid volume and bounding box, the document feature ",
            "count, the last solve's degrees of freedom, and the variable set."
        ),
        args: &[],
        examples: &["props"],
    },
    CommandSpec {
        name: "undo",
        token: "undo",
        usage: "undo",
        category: "editor",
        summary: "Undo the last edit (restores the prior canonical document state)",
        long_help: concat!(
            "Rewinds the session to the previous document snapshot and re-derives ",
            "the solid. A typed error reports when nothing can be undone."
        ),
        args: &[],
        examples: &["vars:set:width:120", "undo", "redo"],
    },
    CommandSpec {
        name: "redo",
        token: "redo",
        usage: "redo",
        category: "editor",
        summary: "Redo the last undone edit",
        long_help: concat!(
            "Re-applies the most recently undone document snapshot and re-derives ",
            "the solid. A typed error reports when nothing can be redone."
        ),
        args: &[],
        examples: &["vars:set:width:120", "undo", "redo"],
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
pub fn categories() -> [&'static str; 6] {
    [
        "interchange",
        "pdm",
        "assembly",
        "editor",
        "inspection",
        "help",
    ]
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
        "cargo run --example workbench -- --write-help-index:",
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
