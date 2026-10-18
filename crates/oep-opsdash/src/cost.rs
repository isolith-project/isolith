//! Cost/energy ledger for the ops dashboard.
//!
//! The energy model is deliberately simple, explicit and *derived from real
//! runtimes the dashboard measured between real dispatch and real ack*:
//!
//! ```text
//!   energy_wh      = node_power_w * runtime_ms / 3_600_000    (W x h)
//!   energy_mwh     = energy_wh * 1000
//!   cost_usd_micro = energy_wh * cents_per_kwh * 10
//! ```
//!
//! Runtimes come from [`crate::clock::Clock`] deltas the dashboard records
//! when it actually hands a dispatch to a node (`claim` start) and when it
//! resolves (`ack`/`fail`/`poison`/`reclaim` end). If a job is reclaimed,
//! its partial runtime still counts as energy the node consumed. Totals are
//! per node and cluster-wide. The model is pure integer math on `u64`/`u128`
//! (no floats), so it is exact on every host and every CI run.
//!
//! The rate is expressed in **US-cents per kWh** (integer; 15 = $0.15/kWh) so
//! no floating point is needed: `1 Wh at C cents/kWh = C x 10 micro-USD`.

/// One recorded job run: real start/end on a node.
#[derive(Debug, Clone, Copy)]
pub struct JobRun {
    /// Total declared draw of the node, watts.
    pub node_power_w: u64,
    /// Energy rate for billing, US-cents per kWh (integer).
    pub cents_per_kwh: u64,
    /// Real elapsed wall time of the run, ms (measured by the dashboard
    /// clock between claim and ack/reclaim).
    pub runtime_ms: u64,
}

impl JobRun {
    /// Energy consumed in watt-hours (integer math: wh = w * ms / 3.6e6).
    pub fn energy_wh(&self) -> u64 {
        self.node_power_w.saturating_mul(self.runtime_ms) / 3_600_000
    }

    /// Energy in milli-watt-hours (mWh = W * (ms/3600)), fractional-safe for
    /// the totals view even for sub-second runs.
    pub fn energy_mwh(&self) -> u128 {
        self.node_power_w as u128 * self.runtime_ms as u128 / 3_600
    }

    /// Cost in US dollars for this run, integer micro-dollars.
    pub fn cost_usd_micro(&self) -> u128 {
        // energy_mwh / 1e6 = kWh; cost_usd = kWh * (cents/100);
        // cost_usd_micro = cost_usd * 1e6 = energy_mwh * cents / 100.
        self.energy_mwh() * self.cents_per_kwh as u128 / 100
    }
}

/// A per-node energy/cost accumulator.
#[derive(Debug, Clone, Default)]
pub struct NodeEnergy {
    /// Total energy consumed by this node, mWh (fractional-safe).
    pub energy_mwh: u128,
    /// Total cost incurred by this node, micro-USD.
    pub cost_usd_micro: u128,
    /// Number of recorded job runs (finite or reclaimed).
    pub runs: u64,
}

impl NodeEnergy {
    /// Add one real job run.
    pub fn add(&mut self, run: JobRun) {
        self.energy_mwh += run.energy_mwh();
        self.cost_usd_micro += run.cost_usd_micro();
        self.runs += 1;
    }
}

/// The full cost/energy ledger, keyed by node id.
#[derive(Debug, Clone, Default)]
pub struct EnergyLedger {
    /// Node id -> accumulated energy/cost.
    pub nodes: std::collections::BTreeMap<String, NodeEnergy>,
}

impl EnergyLedger {
    /// A new empty ledger.
    pub fn new() -> Self {
        EnergyLedger::default()
    }

    /// Record a real job run on `node_id`.
    pub fn record(&mut self, node_id: &str, run: JobRun) {
        self.nodes.entry(node_id.to_string()).or_default().add(run);
    }

    /// Cluster-wide energy consumed, mWh.
    pub fn total_energy_mwh(&self) -> u128 {
        self.nodes.values().map(|n| n.energy_mwh).sum()
    }

    /// Cluster-wide cost, micro-USD.
    pub fn total_cost_usd_micro(&self) -> u128 {
        self.nodes.values().map(|n| n.cost_usd_micro).sum()
    }

    /// Cluster-wide recorded job runs.
    pub fn total_runs(&self) -> u64 {
        self.nodes.values().map(|n| n.runs).sum()
    }

    /// A node's accumulator.
    pub fn get(&self, node_id: &str) -> Option<&NodeEnergy> {
        self.nodes.get(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_is_watts_times_hours() {
        // 1000 W for 1h = 1000 Wh.
        let run = JobRun {
            node_power_w: 1000,
            cents_per_kwh: 10,
            runtime_ms: 3_600_000,
        };
        assert_eq!(run.energy_wh(), 1000);
        assert_eq!(run.energy_mwh(), 1_000_000);
        // 1000 Wh at 10c/kWh = 1 kWh * $0.10 = $0.10 = 100_000 micro.
        assert_eq!(run.cost_usd_micro(), 100_000);
    }

    #[test]
    fn sub_hour_run_reduces_energy() {
        // 1000 W for 36s = 10 Wh.
        let run = JobRun {
            node_power_w: 1000,
            cents_per_kwh: 1,
            runtime_ms: 36_000,
        };
        assert_eq!(run.energy_wh(), 10);
        // 0.01 kWh at 1c/kWh = $0.0001 = 100 micro.
        assert_eq!(run.cost_usd_micro(), 100);
    }

    #[test]
    fn dashboard_shaped_run() {
        // 350 W for 3.6s = 0.35 Wh = 350 mWh.
        let run = JobRun {
            node_power_w: 350,
            cents_per_kwh: 15,
            runtime_ms: 3_600,
        };
        assert_eq!(run.energy_mwh(), 350);
        // 0.00035 kWh at 15c/kWh = $0.0000525 = 52 micro-USD (integer).
        assert_eq!(run.cost_usd_micro(), 52);
    }

    #[test]
    fn ledger_accumulates() {
        let mut l = EnergyLedger::new();
        l.record(
            "u1",
            JobRun {
                node_power_w: 1000,
                cents_per_kwh: 10,
                runtime_ms: 3_600_000,
            },
        );
        l.record(
            "u1",
            JobRun {
                node_power_w: 1000,
                cents_per_kwh: 10,
                runtime_ms: 3_600_000,
            },
        );
        l.record(
            "nas",
            JobRun {
                node_power_w: 50,
                cents_per_kwh: 10,
                runtime_ms: 3_600_000,
            },
        );
        assert_eq!(l.total_energy_mwh(), 2_000_000 + 50_000);
        assert_eq!(l.total_runs(), 3);
        // 2 * $0.10 + 1 * $0.005 = $0.205 = 205_000 micro.
        assert_eq!(l.total_cost_usd_micro(), 205_000);
    }
}
