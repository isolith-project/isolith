//! oep-fairshare -- P9: P2P compute lane (workstream P9).
//!
//! P9-M2-T02 delivers **fair share per team/node + quota enforcement**: the
//! admission policy the P9 scheduler (P9-M2-T01, sibling `oep-lane` crate)
//! calls before dispatching a job, so that no single team can monopolize the
//! compute pool and an explicitly-oversubscribed team is **throttled** while
//! teams inside their share proceed.
//!
//! Two enforcement layers, exactly the two card groups:
//!
//! - **Quota enforcement** (cluster-wide, per team). Every team has a maximum
//!   number of concurrently-running CPU millicores, memory MiB, and jobs. A
//!   submission that would push a team past any of these caps is throttled
//!   **even when the cluster has idle capacity** -- this is what proves
//!   "oversubscribed teams get throttled" under load. Quotas are *configured*
//!   (a real JSON `Config`, [`config::Config`]), and a team with no explicit
//!   entry gets a **conservative default**: an equal share of the whole
//!   cluster (total capacity / team count), never more.
//! - **Per-node fair-share scheduling**. On each node the scheduler
//!   recomputes a weighted max-min allocation of the node's CPU/memory among
//!   the teams that have demand there (weights from the team quota; default
//!   `1`). A team's running usage on a node may never exceed its allocation,
//!   with the node's idle capacity redistributed to other demanding teams.
//!   This is what proves "per-node fair-share scheduling works": two teams
//!   of equal weight saturate a node at 50/50 no matter how aggressively one
//!   of them submits.
//!
//! The wire format is our own (`format = "oep-fairshare"`, `version = 1`);
//! `Config` round-trips through `serde_json` and `docs/P9-M2/fairshare.sample.json`
//! is the committed example the acceptance tests validate against the same
//! bytes the library deserializes.
//!
//! Pure integer arithmetic throughout (u64/u128) -- no floats -- so
//! enforcement is exact on every host and CI run is reproducible.
//!
//! [36. Design contract]: weights and caps are only meaningful together:
//! `weight` scales the max-min allocation (fair share), the explicit caps are
//! hard ceilings. Defaults are conservative on purpose (equal cluster share),
//! so an unconfigured team cannot hoard the pool.
#![deny(missing_docs)]

pub mod config;
pub mod enforce;
pub mod fair;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-COMPUTE";

/// Canonical `format` header value identifying P9 fair-share configs.
pub const FORMAT: &str = "oep-fairshare";
/// Current schema version of the fair-share config format.
pub const VERSION: u32 = 1;

/// Node/team/cluster configuration with conservative defaults.
pub use config::{Cluster, Config, Node, NodeCapacity, TeamQuota};
/// The live fair-share scheduler that enforces quotas and per-node fairness.
pub use enforce::FairShareScheduler;
/// The admission decision for one job-submission attempt.
pub use enforce::{Admission, ThrottleReason};
/// Weighted max-min fair-share allocation (pure, testable).
pub use fair::weighted_max_min;
