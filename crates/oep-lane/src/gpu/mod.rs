//! P9-M4-T01 -- GPU / heterogeneous hardware discovery for the compute lane.
//!
//! This module answers the first half of the card: *discover* which nodes
//! carry GPU devices and what those devices are, honestly, from the real
//! hardware. Two entry points:
//!
//! - [`parse_smi_csv`] -- a pure, testable parser for the exact
//!   `nvidia-smi --query-gpu=... --format=csv,noheader` dialect, including
//!   the unified-memory DGX/GB10 behavior of reporting `[N/A]` for
//!   `memory.total` (an observed real-hosts quirk: DGX-class devices report
//!   `[N/A]` because their memory is unified and not a per-device BAR).
//! - [`GpuProbe`] -- the discovery dialect: a probe yields [`NodeResources`]
//!   (node id + memory + GPU devices). Deterministic tests use
//!   [`StaticProbe`]; live hosts use the `gpu_discover` example, which shells
//!   out to `nvidia-smi`, parses it through [`parse_smi_csv`] and records the
//!   result to `P9-M4/spike/run-log.json`.
//!
//! Deliberately std-only (matching the scheduler crate): discovery is
//! I/O-to-nothing that must be testable in CI without any GPU present. The
//! real `nvidia-smi` invocation lives in the example binary, not here, so CI
//! (which has no GPU) builds and runs the pure parser while live measurement
//! is an explicit, recorded human action on real hardware.

use std::fmt;

/// A single GPU device as reported by discovery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuDevice {
    /// Device ordinal within the node (as nvidia-smi reports it, 0-based).
    pub index: u32,
    /// Marketing/model name, e.g. `NVIDIA GB10`.
    pub name: String,
    /// Driver-reported UUID (stable across runs).
    pub uuid: String,
    /// NVIDIA driver version, e.g. `580.178.04`.
    pub driver_version: String,
    /// CUDA compute capability, e.g. `12.1`. `None` when the driver reports
    /// no capability (rare; kept optional rather than fabricated).
    pub compute_cap: Option<String>,
    /// Device memory in MiB. `None` when the driver reports `[N/A]` (DGX
    /// unified-memory devices); an honest unknown, never a 0 or a guess.
    pub memory_mib: Option<u64>,
}

impl fmt::Display for GpuDevice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} #{} ({}) driver {} compute {} mem {}MiB",
            self.name,
            self.index,
            self.uuid,
            self.driver_version,
            self.compute_cap.as_deref().unwrap_or("n/a"),
            self.memory_mib
                .map(|m| m.to_string())
                .unwrap_or_else(|| "n/a".to_string()),
        )
    }
}

/// Hardware resources of one compute node as seen by discovery.
///
/// `gpus` is empty for a CPU-only node; a node is "GPU-capable" iff
/// `!gpus.is_empty()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeResources {
    /// Stable node id used on the message bus (`oep.nodes.<node-id>.*`).
    pub node_id: String,
    /// Total system memory in MiB (an honest approximation for placement).
    pub memory_mib: u64,
    /// GPU devices present on this node (empty = CPU-only node).
    pub gpus: Vec<GpuDevice>,
}

impl NodeResources {
    /// A CPU-only node resource set.
    pub fn cpu_only(node_id: impl Into<String>, memory_mib: u64) -> Self {
        NodeResources {
            node_id: node_id.into(),
            memory_mib,
            gpus: Vec::new(),
        }
    }

    /// Whether this node carries at least one GPU device.
    pub fn has_gpu(&self) -> bool {
        !self.gpus.is_empty()
    }

    /// GPU device indices present on this node.
    pub fn gpu_indices(&self) -> Vec<u32> {
        self.gpus.iter().map(|g| g.index).collect()
    }
}

/// Parses the `nvidia-smi --query-gpu=... --format=csv,noheader` field set:
///
/// ```text
/// index, name, uuid, driver_version, compute_cap, memory.total [MiB]
/// 0, NVIDIA GB10, GPU-..., 580.178.04, 12.1, [N/A]
/// ```
///
/// Tolerant by design (the "no crash on unknown input" doctrine): a blank
/// line contributes nothing, a `[N/A]` memory field becomes `memory_mib:
/// None`, and only well-formed device lines are emitted. Errors are reserved
/// for a genuinely unparseable structure (missing header semantics), so the
/// live probe can never wedge the lane on a probe surprise.
pub fn parse_smi_csv(text: &str) -> Result<Vec<GpuDevice>, String> {
    let mut devices = Vec::new();
    for (lineno, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        // Skip any header line (starts with the word "index" or contains
        // "memory.total").
        if line.starts_with("index") || line.contains("memory.total") {
            continue;
        }
        // nvidia-smi csv,noheader uses ", " between fields.
        let fields: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if fields.len() < 5 {
            return Err(format!(
                "smi line {lineno}: expected >=5 fields, got {} ({line:?})",
                fields.len()
            ));
        }
        let index: u32 = fields[0]
            .parse()
            .map_err(|_| format!("smi line {lineno}: bad index {:?}", fields[0]))?;
        let name = fields[1].to_string();
        let uuid = fields[2].to_string();
        let driver_version = fields[3].to_string();
        let compute_cap = if fields[4] == "[N/A]" || fields[4].is_empty() {
            None
        } else {
            Some(fields[4].to_string())
        };
        // memory field may be absent (5-field query) or [N/A].
        let memory_mib = if fields.len() >= 6 {
            match fields[5] {
                "[N/A]" | "" => None,
                other => other.parse::<u64>().ok(),
            }
        } else {
            None
        };
        devices.push(GpuDevice {
            index,
            name,
            uuid,
            driver_version,
            compute_cap,
            memory_mib,
        });
    }
    Ok(devices)
}

/// Discovery dialect: yields the hardware of every node the probe can see.
///
/// The scheduler-facing contract is small so a real probe (shelling
/// `nvidia-smi`), a static probe (fixtures) and, later, a bus-fed probe
/// (P9-M1-T02 `oep.nodes.*.heartbeat`) can all implement it.
pub trait GpuProbe {
    /// The resources of all visible nodes. Never fails: a host with no GPU
    /// simply yields nodes with empty `gpus`.
    fn probe(&self) -> Vec<NodeResources>;
}

/// A deterministic probe backed by caller-supplied [`NodeResources`]:
/// for unit/integration tests only, never for live discovery.
#[derive(Clone, Debug, Default)]
pub struct StaticProbe {
    nodes: Vec<NodeResources>,
}

impl StaticProbe {
    /// Build from an explicit node list.
    pub fn new(nodes: Vec<NodeResources>) -> Self {
        StaticProbe { nodes }
    }
}

impl GpuProbe for StaticProbe {
    fn probe(&self) -> Vec<NodeResources> {
        self.nodes.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_real_gb10_line() {
        let text = "0, NVIDIA GB10, GPU-xy, 580.178.04, 12.1, [N/A]\n";
        let devs = parse_smi_csv(text).unwrap();
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].name, "NVIDIA GB10");
        assert_eq!(devs[0].memory_mib, None);
        assert_eq!(devs[0].compute_cap.as_deref(), Some("12.1"));
    }

    #[test]
    fn parses_memory_number() {
        let text = "0, NVIDIA RTX A2000, GPU-a, 535.104.05, 8.6, 6144\n";
        let devs = parse_smi_csv(text).unwrap();
        assert_eq!(devs[0].memory_mib, Some(6144));
    }

    #[test]
    fn header_and_blank_lines_skipped() {
        let text = "\
index, name, uuid, driver_version, compute_cap, memory.total [MiB]

0, NVIDIA GB10, GPU-xy, 580.178.04, 12.1, [N/A]
";
        let devs = parse_smi_csv(text).unwrap();
        assert_eq!(devs.len(), 1);
    }

    #[test]
    fn bad_line_errors() {
        assert!(parse_smi_csv("0\n").is_err());
    }

    #[test]
    fn cpu_only_node_reporting() {
        let g = NodeResources::cpu_only("cpu-1", 4096);
        assert!(!g.has_gpu());
        assert!(g.gpu_indices().is_empty());
    }
}
