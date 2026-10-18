//! P9-M4-T01 -- GPU-aware placement for the compute lane.
//!
//! This module answers the second half of the card: *schedule GPU-tagged
//! jobs onto GPU nodes, with a clean CPU fallback.* It is a thin, pure
//! dialect on top of the device model in [`crate::gpu`]:
//!
//! - [`GpuRequest`] -- the *typing* that tags a job as CPU-only (`None`),
//!   GPU-preferred (`required: false`) or GPU-required (`required: true`).
//!   It mirrors `oep-job::spec::GpuRequest` field-for-field (the same three
//!   fields, the same `required` semantics) so the typed job spec and the
//!   lane scheduler agree; the crate stays std-only and does not depend on
//!   `oep-job` yet (the cross-crate seam opens at P6-M4-T02, per the P9-M2
//!   scheduler note).
//! - [`NodePool`] -- the registry of discovered nodes and their **free**
//!   devices. `reserve`/`release` give the scheduler an exclusive handshake:
//!   a device that is reserved for a running job cannot be handed to another
//!   until released.
//! - [`PlacementPolicy`] -- the decision: a required GPU job goes *only* on a
//!   GPU node with a free matching device (else it parks, never falls back);
//!   a preferred GPU job takes the GPU when one is free and, when none is,
//!   **falls back to a CPU node cleanly** (flagged `cpu_fallback: true`) --
//!   it is never allowed to fail or dead-letter merely for lack of a GPU.
//! - [`Placement`] -- the outcome: which node, which reserved device
//!   indices, and whether a CPU fallback happened.

use crate::gpu::NodeResources;

/// A GPU request for a job -- the placement typing.
///
/// Mirrors `oep-job::spec::GpuRequest` exactly (count, required, model).
/// `None` (absent) on a job means CPU-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuRequest {
    /// Number of devices requested (>=1; <= the lane cap of 8).
    pub count: u32,
    /// `true` = strict (no CPU fallback; park until a matching GPU is free).
    /// `false` = preferred (fall back to CPU cleanly).
    pub required: bool,
    /// Optional hardware/model hint (e.g. `NVIDIA GB10`); matched against
    /// the device name when present.
    pub model: Option<String>,
}

/// Where a dispatched job should run, decided by the placement policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// The node id the job should be sent to.
    pub node_id: String,
    /// Device indices reserved (and now exclusively held) for this job on
    /// `node_id`. Empty for a CPU placement or CPU fallback.
    pub gpu_devices: Vec<u32>,
    /// `true` means this is a CPU fallback for a GPU-*preferred* job that
    /// found no free GPU. Honest flag: dashboards and workers can see the
    /// job did not get the GPU it would have liked.
    pub cpu_fallback: bool,
}

impl Placement {
    /// A placement for a job dispatched without a node pool (pre-P9-M4 lane
    /// behavior / the P9-M2-T01 baseline): no node known, no fallback.
    pub fn unplaced() -> Self {
        Placement {
            node_id: String::new(),
            gpu_devices: Vec::new(),
            cpu_fallback: false,
        }
    }
}

/// Registry of known nodes and their free GPU devices.
///
/// Mutable, exclusive accounting: a device is in exactly one of {free,
/// reserved-by-a-running-job} at any time. All methods are pure in the sense
/// of no I/O / no clock; the scheduler drives the lifecycle.
#[derive(Clone, Debug, Default)]
pub struct NodePool {
    /// Ordered by registration; index is stable.
    nodes: Vec<NodeSlot>,
}

/// A registered node plus its free-device bookkeeping.
#[derive(Clone, Debug)]
struct NodeSlot {
    /// Resources as discovered.
    resources: NodeResources,
    /// Indices of device ordinals currently *free* (available to reserve).
    free: Vec<u32>,
}

impl NodePool {
    /// An empty pool.
    pub fn new() -> Self {
        NodePool::default()
    }

    /// Build a pool from discovered node resources (the probe output shape).
    pub fn from_nodes(nodes: Vec<NodeResources>) -> Self {
        let mut pool = NodePool::new();
        for n in nodes {
            pool.add(n);
        }
        pool
    }

    /// Register (or refresh) a node. Re-adding an existing id refreshes its
    /// resources and frees all its devices (a rejoin after a drop).
    pub fn add(&mut self, resources: NodeResources) {
        let node_id = resources.node_id.clone();
        let free = resources.gpu_indices();
        if let Some(slot) = self
            .nodes
            .iter_mut()
            .find(|s| s.resources.node_id == node_id)
        {
            slot.resources = resources;
            slot.free = free;
            return;
        }
        self.nodes.push(NodeSlot { resources, free });
    }

    /// Remove a node (it left the fleet). Any devices it held are gone with
    /// it; jobs holding them must be reclaimed by the scheduler.
    pub fn remove(&mut self, node_id: &str) {
        self.nodes.retain(|s| s.resources.node_id != node_id);
    }

    /// Node ids currently known, in registration order.
    pub fn node_ids(&self) -> Vec<String> {
        self.nodes
            .iter()
            .map(|s| s.resources.node_id.clone())
            .collect()
    }

    /// Whether `node_id` is known.
    pub fn has_node(&self, node_id: &str) -> bool {
        self.nodes.iter().any(|s| s.resources.node_id == node_id)
    }

    /// Resources of `node_id`, if known.
    pub fn resources(&self, node_id: &str) -> Option<&NodeResources> {
        self.nodes
            .iter()
            .find(|s| s.resources.node_id == node_id)
            .map(|s| &s.resources)
    }

    /// The device indices of `node_id` that are currently free.
    pub fn free_devices(&self, node_id: &str) -> Vec<u32> {
        self.nodes
            .iter()
            .find(|s| s.resources.node_id == node_id)
            .map(|s| s.free.clone())
            .unwrap_or_default()
    }

    /// A node with GPUs (prefer one with the most free devices).
    pub fn gpu_node(&self) -> Option<&str> {
        self.nodes
            .iter()
            .filter(|s| !s.free.is_empty())
            .max_by_key(|s| s.free.len())
            .map(|s| s.resources.node_id.as_str())
    }

    /// A CPU-only node, if any (a node with no GPUs at all).
    pub fn cpu_only_node(&self) -> Option<&str> {
        self.nodes
            .iter()
            .find(|s| s.resources.gpus.is_empty())
            .map(|s| s.resources.node_id.as_str())
    }

    /// Whether exactly enough *matching* free devices exist on `node_id`.
    /// `model` filters by device name (case-insensitive substring) when set.
    fn has_matching_free(&self, node_id: &str, count: u32, model: Option<&str>) -> bool {
        let Some(slot) = self.nodes.iter().find(|s| s.resources.node_id == node_id) else {
            return false;
        };
        if (slot.free.len() as u32) < count {
            return false;
        }
        match model {
            None => true,
            Some(m) => {
                let want = m.to_lowercase();
                let hits = slot
                    .resources
                    .gpus
                    .iter()
                    .filter(|g| slot.free.contains(&g.index))
                    .filter(|g| g.name.to_lowercase().contains(&want))
                    .count();
                (hits as u32) >= count
            }
        }
    }

    /// Whether any registered node currently has at least `count` free
    /// devices matching `model` (non-mutating; the scheduler uses this to
    /// re-promote parked jobs without disturbing reservations).
    pub fn has_capacity(&self, count: u32, model: Option<&str>) -> bool {
        self.nodes.iter().any(|s| {
            let matches: Vec<u32> = match model {
                None => s.free.clone(),
                Some(m) => {
                    let want = m.to_lowercase();
                    s.resources
                        .gpus
                        .iter()
                        .filter(|g| s.free.contains(&g.index))
                        .filter(|g| g.name.to_lowercase().contains(&want))
                        .map(|g| g.index)
                        .collect()
                }
            };
            (matches.len() as u32) >= count
        })
    }

    /// Reserve `count` free devices on `node_id` (optionally model-filtered).
    /// Returns the reserved device indices; they are removed from the free
    /// set until [`NodePool::release`] is called.
    pub fn reserve_matching(
        &mut self,
        node_id: &str,
        count: u32,
        model: Option<&str>,
    ) -> Option<Vec<u32>> {
        if !self.has_matching_free(node_id, count, model) {
            return None;
        }
        let slot = self
            .nodes
            .iter_mut()
            .find(|s| s.resources.node_id == node_id)?;
        let mut taken = Vec::new();
        let mut remaining = Vec::new();
        for &idx in &slot.free {
            let matches = match model {
                None => true,
                Some(m) => {
                    let want = m.to_lowercase();
                    slot.resources
                        .gpus
                        .iter()
                        .any(|g| g.index == idx && g.name.to_lowercase().contains(&want))
                }
            };
            if matches && (taken.len() as u32) < count {
                taken.push(idx);
            } else {
                remaining.push(idx);
            }
        }
        slot.free = remaining;
        Some(taken)
    }

    /// Reserve any `count` free devices on `node_id`.
    pub fn reserve(&mut self, node_id: &str, count: u32) -> Option<Vec<u32>> {
        self.reserve_matching(node_id, count, None)
    }

    /// Return device indices to the free set on `node_id` (idempotent).
    pub fn release(&mut self, node_id: &str, indices: &[u32]) {
        if let Some(slot) = self
            .nodes
            .iter_mut()
            .find(|s| s.resources.node_id == node_id)
        {
            for &idx in indices {
                if !slot.free.contains(&idx) {
                    slot.free.push(idx);
                }
            }
            slot.free.sort_unstable();
        }
    }
}

/// The placement decision dialect: given a request and the fleet state, where
/// does the job go?
#[derive(Clone, Debug, Default)]
pub struct PlacementPolicy {
    /// No config today; kept as a type so policy knobs (e.g. multi-GPU
    /// affinity) can be added without changing call sites.
    _priv: (),
}

impl PlacementPolicy {
    /// Place a job:
    ///
    /// - `request == None` (CPU-only) -> a CPU-only node if one exists, else
    ///   any known node, never flagged as fallback.
    /// - `request.required == false` (GPU preferred) -> a GPU node with a
    ///   free matching device when available; otherwise **fall back to a CPU
    ///   node** with `cpu_fallback: true`. Never returns `None` for a
    ///   preferred job as long as any node exists.
    /// - `request.required == true` (GPU required) -> a GPU node with a free
    ///   matching device, or `None` (the job parks until capacity frees).
    ///
    /// Reservation happens atomically inside: a returned placement owns its
    /// device indices until released back to the pool.
    pub fn place(
        &self,
        pool: &mut NodePool,
        request: Option<&GpuRequest>,
        default_cpu_node: &str,
    ) -> Option<Placement> {
        match request {
            None => {
                // CPU-only: prefer a CPU-only node, else any registered node.
                let node = pool
                    .cpu_only_node()
                    .or_else(|| pool.gpu_node())
                    .unwrap_or(default_cpu_node)
                    .to_string();
                Some(Placement {
                    node_id: node,
                    gpu_devices: Vec::new(),
                    cpu_fallback: false,
                })
            }
            Some(req) => {
                let req = req.clone();

                // A unified scan across every node with a matching free device.
                let mut target: Option<(String, Vec<u32>)> = None;
                for node in pool.node_ids() {
                    if let Some(devices) =
                        pool.reserve_matching(&node, req.count, req.model.as_deref())
                    {
                        target = Some((node, devices));
                        break;
                    }
                }

                match target {
                    Some((node, devices)) => Some(Placement {
                        node_id: node,
                        gpu_devices: devices,
                        cpu_fallback: false,
                    }),
                    None if req.required => None, // park; never CPU-fallback a required job
                    None => {
                        // GPU preferred, none free: clean CPU fallback.
                        let node = pool
                            .cpu_only_node()
                            .or_else(|| pool.gpu_node())
                            .unwrap_or(default_cpu_node)
                            .to_string();
                        Some(Placement {
                            node_id: node,
                            gpu_devices: Vec::new(),
                            cpu_fallback: true,
                        })
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu::{GpuDevice, NodeResources};

    fn gb10_node(id: &str, gpus: u32) -> NodeResources {
        NodeResources {
            node_id: id.into(),
            memory_mib: 64 * 1024,
            gpus: (0..gpus)
                .map(|i| GpuDevice {
                    index: i,
                    name: "NVIDIA GB10".into(),
                    uuid: format!("GPU-{id}-{i}"),
                    driver_version: "580.178.04".into(),
                    compute_cap: Some("12.1".into()),
                    memory_mib: None,
                })
                .collect(),
        }
    }

    fn a100_node(id: &str, gpus: u32) -> NodeResources {
        NodeResources {
            node_id: id.into(),
            memory_mib: 64 * 1024,
            gpus: (0..gpus)
                .map(|i| GpuDevice {
                    index: i,
                    name: "NVIDIA A100-SXM4-40GB".into(),
                    uuid: format!("GPU-{id}-{i}"),
                    driver_version: "535.104.05".into(),
                    compute_cap: Some("8.0".into()),
                    memory_mib: Some(40960),
                })
                .collect(),
        }
    }

    #[test]
    fn exclusive_reservation_lifecycle() {
        let mut pool = NodePool::from_nodes(vec![gb10_node("n1", 2)]);
        let r1 = pool.reserve("n1", 1).unwrap();
        assert_eq!(r1, vec![0]);
        let r2 = pool.reserve("n1", 2); // only 1 free left
        assert!(r2.is_none());
        pool.release("n1", &r1);
        assert_eq!(pool.reserve("n1", 2).unwrap().len(), 2);
    }

    #[test]
    fn model_filter() {
        let mut pool = NodePool::from_nodes(vec![gb10_node("n1", 1), a100_node("n2", 1)]);
        assert!(pool.reserve_matching("n1", 1, Some("GB10")).is_some());
        assert!(pool.reserve_matching("n1", 1, Some("A100")).is_none());
        assert!(pool.reserve_matching("n2", 1, Some("A100")).is_some());
        assert_eq!(
            pool.free_devices("n2"),
            Vec::<u32>::new(),
            "n2 device reserved"
        );
        pool.release("n2", &[0]);
        assert!(
            pool.reserve_matching("n2", 1, Some("a100")).is_some(),
            "case-insensitive"
        );
    }

    #[test]
    fn policy_required_picks_gpu() {
        let mut pool = NodePool::from_nodes(vec![gb10_node("n1", 1)]);
        let p = PlacementPolicy::default();
        let r = p
            .place(
                &mut pool,
                Some(&GpuRequest {
                    count: 1,
                    required: true,
                    model: None,
                }),
                "cpu",
            )
            .unwrap();
        assert_eq!(r.node_id, "n1");
        assert_eq!(r.gpu_devices, vec![0]);
        assert!(!r.cpu_fallback);
    }

    #[test]
    fn policy_required_returns_none_on_no_gpu() {
        let mut pool = NodePool::from_nodes(vec![crate::gpu::NodeResources::cpu_only("c1", 4096)]);
        let p = PlacementPolicy::default();
        let r = p.place(
            &mut pool,
            Some(&GpuRequest {
                count: 1,
                required: true,
                model: None,
            }),
            "cpu",
        );
        assert!(r.is_none());
    }

    #[test]
    fn policy_preferred_falls_back_to_cpu() {
        let mut pool = NodePool::from_nodes(vec![crate::gpu::NodeResources::cpu_only("c1", 4096)]);
        let p = PlacementPolicy::default();
        let r = p
            .place(
                &mut pool,
                Some(&GpuRequest {
                    count: 1,
                    required: false,
                    model: None,
                }),
                "cpu",
            )
            .unwrap();
        assert!(r.cpu_fallback);
        assert!(r.gpu_devices.is_empty());
    }

    #[test]
    fn policy_cpu_only_uses_cpu_node() {
        let mut pool = NodePool::from_nodes(vec![
            gb10_node("g1", 1),
            crate::gpu::NodeResources::cpu_only("c1", 4096),
        ]);
        let p = PlacementPolicy::default();
        let r = p.place(&mut pool, None, "cpu").unwrap();
        assert_eq!(r.node_id, "c1");
        assert!(!r.cpu_fallback);
    }
}
