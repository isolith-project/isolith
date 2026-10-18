//! Fleet model for the P9 GA gate: the real worker nodes the wave runs on.
//!
//! A fleet file declares every node that can execute wave jobs, its real
//! resources (as measured by the P9-M4-T01 discovery run-log or a live
//! probe), and how the driver reaches it:
//!
//! ```json
//! {
//!   "format": "oep-gate-fleet",
//!   "version": 1,
//!   "gpu_model": "NVIDIA GB10",
//!   "memory_mib": 124544,
//!   "coordinator": "u1",
//!   "nodes": [
//!     {"id": "u1", "local": true, "agent_dir": "/home/r1a/p9ga",
//!      "gpus": [{"name": "NVIDIA GB10", "uuid": "GPU-...",
//!                "driver_version": "580.178.04", "compute_cap": "12.1"}]},
//!     {"id": "u2", "local": false, "host": "192.168.0.69", "user": "r1a",
//!      "agent_dir": "/home/r1a/p9ga", "gpus": [...same...]},
//!     {"id": "u3", "local": false, "host": "192.168.0.70", "user": "r1a",
//!      "agent_dir": "/home/r1a/p9ga", "gpus": [...]}
//!   ]
//! }
//! ```
//!
//! Everything the gate claims about the fleet is real: node ids, real GPU
//! hardware (from the P9-M4-T01 nvidia-smi run-log), real SSH endpoints.

use std::path::Path;

use oep_lane::gpu::{GpuDevice, NodeResources};
use serde::{Deserialize, Serialize};

/// One declared compute node in the fleet.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetNode {
    /// Stable node id (the scheduler's placement key).
    pub id: String,
    /// `true` = the coordinator host (executed in-process).
    #[serde(default)]
    pub local: bool,
    /// SSH host for a remote node (`local: false`); empty for local.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub host: String,
    /// SSH user for a remote node.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub user: String,
    /// Directory on the node where `oep-ga-node` is deployed.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub agent_dir: String,
    /// GPU devices present (empty = CPU-only node).
    #[serde(default)]
    pub gpus: Vec<FleetGpu>,
    /// Total system memory in MiB (assumed from fleet default when absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mib: Option<u64>,
}

/// GPU device record (subset of the discovery types; the gate only needs the
/// placement-relevant fields).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FleetGpu {
    /// Device name, e.g. `NVIDIA GB10`.
    pub name: String,
    /// Driver UUID.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uuid: String,
    /// NVIDIA driver version.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub driver_version: String,
    /// CUDA compute capability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_cap: Option<String>,
    /// Device memory (MiB); `None` for DGX unified-memory devices.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mib: Option<u64>,
}

/// A resolved execution endpoint for a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeEndpoint {
    /// The node id.
    pub node_id: String,
    /// `true` when execution happens on the coordinator host (in-process).
    pub is_local: bool,
    /// SSH host (remote).
    pub host: String,
    /// SSH user (remote).
    pub user: String,
    /// Agent deploy dir (remote); empty for local.
    pub agent_dir: String,
}

/// The node fleet the GA gate registers with the scheduler.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Fleet {
    /// Must equal `oep-gate-fleet`.
    pub format: String,
    /// Must equal 1.
    pub version: u64,
    /// Optional fleet-wide GPU model hint for placement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_model: Option<String>,
    /// Optional fleet-wide memory default (MiB), used when a node omits it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mib: Option<u64>,
    /// The coordinator node id (where the driver runs / `--local-only`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub coordinator: String,
    /// The nodes.
    pub nodes: Vec<FleetNode>,
}

impl Fleet {
    /// Load + validate a fleet file.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
        let f: Fleet = serde_json::from_str(&text).map_err(|e| format!("parse {path:?}: {e}"))?;
        if f.format != "oep-gate-fleet" {
            return Err(format!("bad fleet format: {}", f.format));
        }
        if f.version != 1 {
            return Err(format!("unsupported fleet version: {}", f.version));
        }
        if f.nodes.is_empty() {
            return Err("fleet declares zero nodes".to_string());
        }
        Ok(f)
    }

    /// The coordinator node id (fallback: first local node else first node).
    pub fn coordinator_id(&self) -> String {
        if !self.coordinator.is_empty() && self.nodes.iter().any(|n| n.id == self.coordinator) {
            return self.coordinator.clone();
        }
        self.nodes
            .iter()
            .find(|n| n.local)
            .map(|n| n.id.clone())
            .unwrap_or_else(|| self.nodes[0].id.clone())
    }

    /// Resolve the execution endpoint for a node id.
    pub fn endpoint_for(&self, node_id: &str) -> NodeEndpoint {
        let node = self
            .nodes
            .iter()
            .find(|n| n.id == node_id)
            .expect("fleet node id registered");
        if node.local || node.host.is_empty() {
            NodeEndpoint {
                node_id: node.id.clone(),
                is_local: true,
                host: String::new(),
                user: String::new(),
                agent_dir: node.agent_dir.clone(),
            }
        } else {
            NodeEndpoint {
                node_id: node.id.clone(),
                is_local: false,
                host: node.host.clone(),
                user: node.user.clone(),
                agent_dir: node.agent_dir.clone(),
            }
        }
    }

    /// The node resources set the scheduler pool is built from.
    pub fn node_resources(&self) -> Vec<NodeResources> {
        self.nodes
            .iter()
            .map(|n| NodeResources {
                node_id: n.id.clone(),
                memory_mib: n.memory_mib.or(self.memory_mib).unwrap_or(128_000),
                gpus: n
                    .gpus
                    .iter()
                    .enumerate()
                    .map(|(idx, g)| GpuDevice {
                        index: idx as u32,
                        name: g.name.clone(),
                        uuid: g.uuid.clone(),
                        driver_version: g.driver_version.clone(),
                        compute_cap: g.compute_cap.clone(),
                        memory_mib: g.memory_mib,
                    })
                    .collect(),
            })
            .collect()
    }
}

/// Direct "is this node executed on the coordinator host?" check.
pub fn is_local(node_id: &str, fleet: &Fleet) -> bool {
    fleet.coordinator_id() == node_id
        || fleet
            .nodes
            .iter()
            .any(|n| n.id == node_id && (n.local || n.host.is_empty()))
}
