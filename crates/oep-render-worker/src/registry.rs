//! Worker registry: how the render worker registers with the P9 scheduler.
//!
//! The P9-M1 job spec (docs/P9-M1/job-spec.md, "Executor") says a job's
//! `executor` names `kind` and `function`, and that **worker registry /
//! function resolution is owned by P9-M3-T01** -- this card. The scheduler
//! maps `executor.function` onto a worker through a registry; this module is
//! that registry seam for the render worker. Registering here means the
//! scheduler can resolve:
//!
//! ```text
//!   executor.kind     = "render"
//!   executor.function = "oep-geo-render"
//! ```
//!
//! The scheduler crate (oep-lane, P9-M2-T01, in-flight on a sibling card)
//! will call [`registry`] / [`get`] to resolve this vocabulary; until it
//! merges, the CLI `--describe` flag prints the same record as JSON so the
//! registration is CI-detectable without linking the scheduler crate.

/// Executor kind for this worker, matching the P9-M1 closed vocabulary
/// (`EXECUTOR_RENDER` in `oep-job`).
pub const EXECUTOR_KIND_RENDER: &str = "render";

/// Worker function name, matching the P9-M1 example render job
/// (`executor.function = "oep-geo-render"`).
pub const WORKER_FUNCTION_RENDER: &str = "oep-geo-render";

/// Worker binary name.
pub const WORKER_BIN: &str = "oep-render-worker";

/// Worker version, mirroring the workspace package version.
pub const WORKER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Render options this worker accepts, in the P9-M1 `executor.args` shape.
pub const WORKER_ARGS: [&str; 2] = ["--resolution", "--fidelity"];

/// Fidelity level vocabulary this worker advertises (supersample factors),
/// in the canonical workstreams/P9-M1/GUI order. Low = 1x, Medium = 2x, High = 4x.
pub const FIDELITY_LEVELS: [&str; 3] = ["low", "medium", "high"];

/// A single worker registration record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRegistration {
    /// Executor kind (`executor.kind`).
    pub kind: &'static str,
    /// Worker function name (`executor.function`), the registry key.
    pub function: &'static str,
    /// Worker binary name.
    pub binary: &'static str,
    /// Worker version.
    pub version: &'static str,
    /// One-line description of what the worker does.
    pub description: &'static str,
    /// Fidelity levels this worker supports (unbounded supersample).
    pub fidelity_levels: &'static [&'static str],
    /// Supported CLI arguments (the `executor.args` parameterization).
    pub args: &'static [&'static str],
}

/// The single render worker this card ships.
pub const RENDER_WORKER: WorkerRegistration = WorkerRegistration {
    kind: EXECUTOR_KIND_RENDER,
    function: WORKER_FUNCTION_RENDER,
    binary: WORKER_BIN,
    version: WORKER_VERSION,
    description: "Headless oep-geo renderer: renders a real oep-geo model to a PNG image behind the P6-M5-T01 render seam, parameterized by resolution and fidelity.",
    fidelity_levels: &FIDELITY_LEVELS,
    args: &WORKER_ARGS,
};

/// The full worker registry (all worker functions this lane ships).
pub fn registry() -> &'static [WorkerRegistration] {
    &[RENDER_WORKER]
}

/// Resolve a worker registration by `(executor.kind, executor.function)`.
pub fn get(kind: &str, function: &str) -> Option<&'static WorkerRegistration> {
    registry()
        .iter()
        .find(|w| w.kind == kind && w.function == function)
}

/// Render the registration as a JSON object (std-only, no serde needed in
/// the lib so the crate stays dependency-light and deterministic).
pub fn registration_json(w: &WorkerRegistration) -> String {
    let fid = w
        .fidelity_levels
        .iter()
        .map(|f| format!("\"{}\"", f))
        .collect::<Vec<_>>()
        .join(", ");
    let args = w
        .args
        .iter()
        .map(|a| format!("\"{}\"", a))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{{\n  \"kind\": \"{}\",\n  \"function\": \"{}\",\n  \"binary\": \"{}\",\n  \
         \"version\": \"{}\",\n  \"description\": \"{}\",\n  \"fidelity_levels\": [{}],\n  \
         \"args\": [{}]\n}}",
        w.kind,
        w.function,
        w.binary,
        w.version,
        w.description.replace('"', "\\\""),
        fid,
        args
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_resolves_render_worker() {
        let w = get("render", "oep-geo-render").expect("render/oep-geo-render must be registered");
        assert_eq!(w.binary, "oep-render-worker");
        assert_eq!(w.fidelity_levels, &["low", "medium", "high"]);
    }

    #[test]
    fn registry_rejects_unknown_worker() {
        assert!(get("render", "nope").is_none());
        assert!(get("convert", "oep-geo-render").is_none());
    }

    #[test]
    fn registration_json_is_parseable() {
        let json = registration_json(&RENDER_WORKER);
        // Minimal structural check: it is a JSON object with key: value pairs.
        assert!(json.starts_with('{') && json.ends_with('}'));
        assert!(json.contains("\"function\": \"oep-geo-render\""));
        assert!(json.contains("\"kind\": \"render\""));
    }
}
