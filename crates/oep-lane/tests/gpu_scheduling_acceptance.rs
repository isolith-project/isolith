//! P9-M4-T01 acceptance tests: GPU discovery + GPU-aware placement scheduling
//! with CPU fallback, over real oep-lane code (injected clock, no sleeps).
//!
//! The card acceptance, mapped to real test evidence:
//!
//! 1. **Discover real GPU nodes** -- the probe layer turns raw `nvidia-smi`
//!    CSV (including a real DGX/GB10 line with a `[N/A]` memory field) into a
//!    [`NodeResources`]; the same parsing path that a live probe uses is
//!    exercised here against committed fixture text and a deterministic
//!    `StaticProbe`.
//! 2. **Schedule GPU-tagged jobs onto GPU nodes** -- a GPU *required* job only
//!    dispatches to a node that has a matching free device; the reservation is
//!    exclusive (two jobs on a one-device node run one at a time), and a GPU
//!    job parked on no capacity re-dispatches the moment a GPU node (or free
//!    device) appears.
//! 3. **Fall back to CPU cleanly** -- a GPU *preferred* job takes the GPU when
//!    one is free and, when none is, dispatches to a CPU node with
//!    `cpu_fallback == true` instead of failing or dead-lettering.
//! 4. **Measured on real dgx/gpu hardware with honest numbers** -- the probe
//!    executes the real `nvidia-smi` on the host it runs on and records what
//!    it found into `P9-M4/spike/run-log.json`; this file is asserted to be
//!    present and to parse. The live host measurement itself lives in that
//!    run-log plus the `gpu_discover` example, not in CI (CI has no GPU).
//!
//! Layering: `oep-lane` stays std-only and injected-clock. The placement
//! layer is a thin, pure dialect on top of the P9-M2-T01 scheduler: it owns
//! node device accounting, the scheduler owns ordering/lease semantics, and
//! the two exchange a tiny reservation handshake.

use oep_lane::gpu::{parse_smi_csv, GpuDevice, GpuProbe, NodeResources, StaticProbe};
use oep_lane::placement::{GpuRequest, NodePool, PlacementPolicy};
use oep_lane::sched::{JobSpec, JobStatus, Scheduler};

// ---------------------------------------------------------------------------
// Fixtures -- a real NVIDIA GB10 line (driver 580.178.04, compute 12.1) where
// the unified-memory DGX reports `[N/A]` for `memory.total`, plus a plain
// workstation line. These are observation-based fixtures (see P9-M4 doc).
// ---------------------------------------------------------------------------

/// `nvidia-smi --query-gpu=index,name,uuid,driver_version,compute_cap,memory.total`
/// CSV header followed by one GB10 device line (as observed on the reference
/// host, 2026-10-17).
const GB10_SMI_FIXTURE: &str = "\
index, name, uuid, driver_version, compute_cap, memory.total [MiB]\n\
0, NVIDIA GB10, GPU-331f26d3-21e3-86c0-3574-fa1c79e58297, 580.178.04, 12.1, [N/A]\n";

/// A workstation NVIDIA A2000 line with a real memory number.
const A2000_SMI_FIXTURE: &str = "\
index, name, uuid, driver_version, compute_cap, memory.total [MiB]\n\
0, NVIDIA RTX A2000, GPU-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee, 535.104.05, 8.6, 6144\n";

fn gb10_static_probe() -> StaticProbe {
    StaticProbe::new(vec![NodeResources {
        node_id: "dgx-gb10-01".into(),
        memory_mib: 64 * 1024,
        gpus: vec![GpuDevice {
            index: 0,
            name: "NVIDIA GB10".into(),
            uuid: "GPU-331f26d3-21e3-86c0-3574-fa1c79e58297".into(),
            driver_version: "580.178.04".into(),
            compute_cap: Some("12.1".into()),
            memory_mib: None, // DGX unified memory reports [N/A]
        }],
    }])
}

// ---------------------------------------------------------------------------
// 1. Discovery
// ---------------------------------------------------------------------------

#[test]
fn smi_csv_parses_gb10_with_na_memory() {
    let probes = parse_smi_csv(GB10_SMI_FIXTURE).expect("GB10 fixture parses");
    assert_eq!(probes.len(), 1);
    let dev = &probes[0];
    assert_eq!(dev.name, "NVIDIA GB10");
    assert_eq!(dev.driver_version, "580.178.04");
    assert_eq!(dev.compute_cap.as_deref(), Some("12.1"));
    assert_eq!(dev.memory_mib, None, "unified-memory DGX reports [N/A]");
    assert_eq!(dev.index, 0);
}

#[test]
fn smi_csv_parses_real_memory_number() {
    let probes = parse_smi_csv(A2000_SMI_FIXTURE).expect("A2000 fixture parses");
    assert_eq!(probes.len(), 1);
    assert_eq!(probes[0].memory_mib, Some(6144));
    assert_eq!(probes[0].compute_cap.as_deref(), Some("8.6"));
}

#[test]
fn smi_csv_empty_yields_no_devices() {
    assert!(parse_smi_csv("").unwrap().is_empty());
    assert!(
        parse_smi_csv("index, name, uuid, driver_version, compute_cap, memory.total [MiB]\n")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn static_probe_reports_discovered_gpus() {
    let probe = gb10_static_probe();
    let nodes = probe.probe();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].gpus.len(), 1);
    assert_eq!(nodes[0].gpus[0].name, "NVIDIA GB10");
}

#[test]
fn resources_have_zero_gpus_without_gpu() {
    let probe = StaticProbe::new(vec![NodeResources {
        node_id: "cpu-only-01".into(),
        memory_mib: 16 * 1024,
        gpus: vec![],
    }]);
    assert_eq!(probe.probe()[0].gpus.len(), 0);
}

// ---------------------------------------------------------------------------
// 2. NodePool device accounting
// ---------------------------------------------------------------------------

fn pool_with_gb10() -> NodePool {
    NodePool::from_nodes(gb10_static_probe().probe())
}

fn assert_devices_are_exclusive(pool: &mut NodePool, node: &str, max: usize) {
    // Two sequential 1-device reservations on a 1-device node must occupy the
    // same single device, so the second reservation cannot overlap the first
    // while it is held.
    let first = pool.reserve(node, 1).expect("first reserve succeeds");
    assert_eq!(first.len(), 1);
    let second = pool.reserve(node, 1);
    assert!(second.is_none(), "device is exclusively held");
    pool.release(node, &first);
    assert!(pool.reserve(node, 1).is_some(), "device free after release");
    let _ = max;
}

#[test]
fn gpu_device_reservation_is_exclusive() {
    let mut pool = pool_with_gb10();
    let node = "dgx-gb10-01".to_string();
    assert_devices_are_exclusive(&mut pool, &node, 1);
}

#[test]
fn model_hint_filters_reservation() {
    let mut pool = pool_with_gb10();
    let node = "dgx-gb10-01".to_string();
    // Matching hint reserves; non-matching hint finds nothing.
    assert!(pool.reserve_matching(&node, 1, Some("GB10")).is_some());
    assert!(pool.reserve_matching(&node, 1, Some("A100")).is_none());
}

// ---------------------------------------------------------------------------
// 3. Placement policy + scheduler integration
// ---------------------------------------------------------------------------

fn cpu_pool() -> NodePool {
    NodePool::from_nodes(vec![NodeResources {
        node_id: "cpu-only-01".into(),
        memory_mib: 16 * 1024,
        gpus: vec![],
    }])
}

#[test]
fn gpu_required_schedules_to_gpu_node() {
    let mut pool = pool_with_gb10();
    let policy = PlacementPolicy::default();
    let placement = policy
        .place(
            &mut pool,
            Some(&GpuRequest {
                count: 1,
                required: true,
                model: None,
            }),
            "cpu-any",
        )
        .expect("GPU job places on the GB10 node");
    assert_eq!(placement.node_id, "dgx-gb10-01");
    assert!(!placement.cpu_fallback);
    assert_eq!(placement.gpu_devices, vec![0u32]);
}

#[test]
fn gpu_required_parks_when_no_gpu_node() {
    let mut pool = cpu_pool();
    let policy = PlacementPolicy::default();
    // No GPU node at all: a required GPU job must NOT be placed (it parks).
    let placement = policy.place(
        &mut pool,
        Some(&GpuRequest {
            count: 1,
            required: true,
            model: None,
        }),
        "cpu-any",
    );
    assert!(
        placement.is_none(),
        "required GPU job is not placed without a GPU"
    );
}

#[test]
fn gpu_preferred_falls_back_to_cpu_when_no_gpu() {
    let mut pool = cpu_pool();
    let policy = PlacementPolicy::default();
    let placement = policy
        .place(
            &mut pool,
            Some(&GpuRequest {
                count: 1,
                required: false,
                model: None,
            }),
            "cpu-any",
        )
        .expect("preferred GPU job falls back to CPU, not fail");
    assert!(placement.cpu_fallback, "fallback is flagged honestly");
    assert_eq!(placement.gpu_devices.len(), 0);
}

#[test]
fn gpu_preferred_uses_gpu_when_available() {
    let mut pool = pool_with_gb10();
    let policy = PlacementPolicy::default();
    let placement = policy
        .place(
            &mut pool,
            Some(&GpuRequest {
                count: 1,
                required: false,
                model: None,
            }),
            "cpu-any",
        )
        .expect("preferred GPU job uses the GPU when free");
    assert!(!placement.cpu_fallback);
    assert_eq!(placement.gpu_devices, vec![0u32]);
}

// --- scheduler level -----------------------------------------------------

fn sched_with_pool(pool: NodePool) -> Scheduler {
    Scheduler::with_pool(pool)
}

fn gpu_spec(payload: &str, required: bool) -> JobSpec {
    JobSpec::new(payload).with_gpu(GpuRequest {
        count: 1,
        required,
        model: None,
    })
}

#[test]
fn dispatch_assigns_gpu_placement_to_gpu_job() {
    let mut s = sched_with_pool(pool_with_gb10());
    let job = s.enqueue(gpu_spec("render", true)).unwrap();

    let disp = s.dispatch(0).expect("job dispatches to the GPU node");
    assert_eq!(disp.job, job);
    assert_eq!(disp.placement.node_id, "dgx-gb10-01");
    assert!(!disp.placement.cpu_fallback);
    assert_eq!(disp.placement.gpu_devices, vec![0u32]);

    s.ack(job, disp.generation).unwrap();
    assert_eq!(s.status(job), Some(JobStatus::Succeeded));
}

#[test]
fn required_gpu_parked_until_node_joins() {
    let mut s = sched_with_pool(cpu_pool());
    let job = s.enqueue(gpu_spec("render", true)).unwrap();

    // No GPU node yet: nothing dispatches, the job is honestly parked.
    assert!(s.dispatch(0).is_none());
    assert_eq!(
        s.status(job),
        Some(JobStatus::Ready),
        "parked job stays ready"
    );
    assert_eq!(s.parked_count(), 1);

    // A GB10 node registers: the parked job immediately becomes dispatchable
    // onto it.
    s.add_nodes(gb10_static_probe().probe());
    let disp = s
        .dispatch(0)
        .expect("parked job dispatches once GPU arrives");
    assert_eq!(disp.placement.node_id, "dgx-gb10-01");
    assert!(!disp.placement.cpu_fallback);
}

#[test]
fn required_gpu_waits_for_free_device() {
    // One GPU, one device; two required jobs. The second must wait until the
    // first acks and releases the device, then run on the same device.
    let mut s = sched_with_pool(pool_with_gb10());
    let a = s.enqueue(gpu_spec("a", true)).unwrap();
    let b = s.enqueue(gpu_spec("b", true)).unwrap();

    let d1 = s.dispatch(0).expect("first GPU job dispatches");
    assert_eq!(d1.job, a);
    assert_eq!(d1.placement.gpu_devices, vec![0u32]);

    // Device held: second job dispatches nowhere (parked) even though Ready.
    assert!(s.dispatch(0).is_none());
    assert_eq!(s.parked_count(), 1);

    s.ack(a, d1.generation).unwrap();
    let d2 = s
        .dispatch(0)
        .expect("second GPU job dispatches after device free");
    assert_eq!(d2.job, b);
    assert_eq!(d2.placement.gpu_devices, vec![0u32], "same device reused");
    s.ack(b, d2.generation).unwrap();
}

#[test]
fn preferred_gpu_falls_back_to_cpu_end_to_end() {
    let mut s = sched_with_pool(cpu_pool());
    let job = s.enqueue(gpu_spec("render", false)).unwrap();

    let disp = s
        .dispatch(0)
        .expect("preferred job dispatches even on a CPU pool");
    assert_eq!(disp.job, job);
    assert!(disp.placement.cpu_fallback, "CPU fallback is explicit");
    s.ack(job, disp.generation).unwrap();
    assert_eq!(
        s.status(job),
        Some(JobStatus::Succeeded),
        "fallback is a clean success"
    );
}

#[test]
fn cpu_only_job_places_on_cpu_when_gpu_node_present() {
    // A CPU-only job must dispatch even when a GPU node exists, and must land
    // on a CPU-capable node (the GPU node is only for GPU jobs here).
    let mut pool = pool_with_gb10();
    pool.add(NodeResources {
        node_id: "cpu-only-01".into(),
        memory_mib: 16 * 1024,
        gpus: vec![],
    });
    let mut s = sched_with_pool(pool);

    let job = s.enqueue(JobSpec::new("cpu-job")).unwrap();
    let disp = s.dispatch(0).expect("CPU job dispatches");
    assert_eq!(disp.job, job);
    assert!(!disp.placement.cpu_fallback);
    assert_eq!(disp.placement.gpu_devices.len(), 0);
}

#[test]
fn parked_gpu_does_not_starve_cpu_jobs() {
    // A high-priority required-GPU job that cannot run must not block
    // lower-priority CPU work (honest scheduling: park, don't head-of-line
    // block). Priority: GPU job p=100, CPU job p=1.
    let mut s = sched_with_pool(cpu_pool());
    let gpu_job = s
        .enqueue(gpu_spec("gpu", true).with_priority(oep_lane::sched::Priority::new(100)))
        .unwrap();
    let cpu_job = s
        .enqueue(JobSpec::new("cpu").with_priority(oep_lane::sched::Priority::new(1)))
        .unwrap();

    // First dispatch shows the GPU job (empty ready) after parking the GPU one.
    let first = s.dispatch(0).expect("a job dispatches");
    assert_eq!(
        first.job, cpu_job,
        "CPU job runs; GPU job is parked, not blocking"
    );
    assert_eq!(s.parked_count(), 1);

    s.ack(cpu_job, first.generation).unwrap();
    assert_eq!(s.status(gpu_job), Some(JobStatus::Ready));
}

#[test]
fn fail_release_makes_device_available_for_next() {
    let mut s = sched_with_pool(pool_with_gb10());
    // a retries on a long backoff so the window between fail and retry lets
    // another GPU job use the freed device (honest FIFO: a would otherwise
    // be first to re-dispatch when its short backoff expires at the same
    // instant).
    let long_backoff = oep_lane::sched::RetryPolicy {
        max_attempts: 2,
        base_backoff_ms: 500,
        backoff_factor: 2,
        max_backoff_ms: 5_000,
    };
    let a = s
        .enqueue(gpu_spec("a", true).with_retry(long_backoff))
        .unwrap();
    let b = s.enqueue(gpu_spec("b", true)).unwrap();

    let d1 = s.dispatch(0).unwrap();
    assert_eq!(d1.job, a);
    s.fail(a, d1.generation, 0).unwrap();

    // a's backoff (500ms) has not elapsed: b takes the freed device.
    let d2 = s
        .dispatch(100)
        .expect("device released after fail frees it");
    assert_eq!(d2.job, b);
    assert_eq!(d2.placement.gpu_devices, vec![0u32]);
    s.ack(b, d2.generation).unwrap();

    // After a's backoff window (t>=500): a retries and re-reserves GPU 0.
    let d3 = s.dispatch(600).expect("failed a retries onto the GPU");
    assert_eq!(d3.job, a);
    assert_eq!(d3.placement.gpu_devices, vec![0u32]);
    s.ack(a, d3.generation).unwrap();
    assert_eq!(s.status(a), Some(JobStatus::Succeeded));
}

#[test]
fn no_pool_scheduler_behaves_unchanged() {
    // Regression: a Scheduler built without a pool must dispatch identically
    // to the P9-M2-T01 baseline (no placement on Dispatch).
    let mut s = Scheduler::new();
    let job = s.enqueue(JobSpec::new("x")).unwrap();
    let disp = s.dispatch(0).unwrap();
    assert_eq!(disp.job, job);
    assert_eq!(disp.placement.node_id, "");
    s.ack(job, disp.generation).unwrap();
    assert_eq!(s.status(job), Some(JobStatus::Succeeded));
}

#[test]
fn run_log_committed_for_hardware_evidence() {
    // The real-dgx/gpu measurement record must exist (the Python acceptance
    // test tests/test_p9m4t01_gpu_scheduling.py validates its JSON + content;
    // keeping the Rust crate std-only by not pulling serde_json here).
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../P9-M4/spike/run-log.json"
    );
    let text = std::fs::read_to_string(path).expect("run-log.json is committed");
    assert!(
        text.contains("nvidia-smi"),
        "run-log records the nvidia-smi measure"
    );
    assert!(
        text.contains("gb10") || text.contains("GB10"),
        "run-log names the discovered gpu"
    );
}
