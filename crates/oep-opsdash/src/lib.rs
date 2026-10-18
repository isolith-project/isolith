//! oep-opsdash -- P9-M5-T01: ops dashboard for the P9 compute lane.
//!
//! A live operational view of the P9 scheduler: **queue depth**, **node
//! health**, **job logs** and a **cost/energy view**. Every number shown is
//! read from the real [`oep_lane`] scheduler or derived from its real
//! lease/ack lifecycle events -- nothing is mocked.
//!
//! ```text
//!   oep-opsdash serve --listen 0.0.0.0:8080      serve the live dashboard
//!   oep-opsdash snapshot --out snapshot.json      dump a JSON snapshot (proof)
//!   oep-opsdash render   --out dashboard.html     render the HTML view (proof)
//! ```
//!
//! Architecture (each concern is one module):
//!
//! - [`dashboard`] -- the aggregate root. Owns the real
//!   [`oep_lane::sched::Scheduler`], a [`NodeRegistry`](registry::NodeRegistry),
//!   a [`JobLog`](log::JobLog) and an [`EnergyLedger`](cost::EnergyLedger).
//!   All public methods are the ops API: `enqueue`, `dispatch_to` (hand a
//!   real dispatch to a node), `ack`, `fail`, `poison`, `reap` (real
//!   scheduler `reclaim_expired`) and `snapshot` (read current state).
//! - [`clock`] -- injected clock (the same doctrine as `oep-lane` time): a
//!   [`Clock`](clock::Clock) with a [`SystemClock`](clock::SystemClock) for
//!   the live binary and a [`ManualClock`](clock::ManualClock) for
//!   deterministic tests.
//! - [`registry`] -- real node registry. A node's health transitions come
//!   ONLY from real lifecycle events: heartbeat on claim, `Degraded` when one
//!   of its jobs is reclaimed by the scheduler, `Down` when its heartbeat
//!   goes stale.
//! - [`log`] -- job log ring buffer: real lifecycle events with timestamps.
//! - [`cost`] -- cost/energy ledger: `energy_wh = node_power_w * runtime_h`,
//!   `cost = energy_kwh * rate`; runtimes are measured by the caller's clock
//!   between real dispatch and real ack, so the totals are only as good as
//!   the real work actually ran.
//! - [`model`] -- the serde snapshot wire format (queue / nodes / jobs /
//!   cost-energy) that both the JSON API and the HTML render consume.
//! - [`html`] -- render a [`DashboardSnapshot`](model::DashboardSnapshot) to
//!   one self-contained HTML page (all four views).
//! - [`http`] -- std-only HTTP/1.1 server exposing `/`, `/api/snapshot`,
//!   `/api/queue`, `/api/nodes`, `/api/jobs`, `/api/cost`.
//!
//! The lane doctrine (per P9-M2-T01) is std-only + injected time, so this
//! crate uses the standard library for everything except JSON serialization
//! and the scheduler dependency itself.
#![deny(missing_docs)]

pub mod clock;
pub mod cost;
pub mod dashboard;
pub mod demo;
pub mod html;
pub mod http;
pub mod log;
pub mod model;
pub mod registry;

/// Workstream identity for the crate.
pub const WORKSTREAM: &str = "P9-COMPUTE";

/// The crate's canonical `format` header for dashboard snapshots.
pub const FORMAT: &str = "oep-opsdash";
/// Current snapshot schema version.
pub const VERSION: u32 = 1;
