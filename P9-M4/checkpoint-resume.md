# Checkpoint / Resume for Long Jobs (P9-M4-T02)

Version 1.0, 2026-10-17. Milestone: P9-M4-T02 (wave-2 downstream P9 compute
lane). Program: Keelworks / Open Engineering Platform (OEP).

## 1. What this is

The **checkpoint/resume** mechanism for long compute jobs, in the same
spirit as Apache Beam checkpoints or a MapReduce farm: while a long job runs,
the worker periodically persists a durable snapshot of its progress; if the
worker is evicted mid-job (node loss, preemption, OOM kill), a replacement
worker **resumes from the last checkpoint** instead of restarting the whole
job, and the final output is **byte-equivalent** to what a never-evicted run
would have produced.

Crate: `crates/oep-farm`. Binary (drill/proof): `oep-farm-drill`.

The acceptance, mapped 1:1 to test evidence:

1. a long job survives worker eviction via checkpoints;
2. it resumes from the last checkpoint;
3. resume is verified byte-equivalent to a clean run on output.

## 2. Contract placement in the P9 wave

- The **scheduler** (P9-M2-T01, `oep-lane`) decides *when* a job is given to
  a worker: leases, expiry, and re-dispatch with the same idempotency key
  (at-least-once delivery). This crate answers *how* the redelivered worker
  makes that cheap and **correct**: it reads the job's last checkpoint and
  continues, rather than re-running everything.
- The **job spec** (P9-M1-T01, `oep-job`) names the job's inputs, executor,
  caps and output refs. This crate treats a job as an opaque id plus a
  *deterministic work-item source* and stays deliberately self-contained
  (no cross-crate path deps until the P6 importer hub, matching the sibling
  `oep-state` crate). Binding onto `JobSpec` happens in a later milestone.
- The **artifact** this crate streams into is where the job's output refs
  will point; the GA gate (P9-M5) can content-verify the bytes.

## 3. The three moving pieces

### 3.1 Deterministic work items (`items`)

A long job is an ordered sequence of work items. `ItemSource` is the
contract: `item_count()` and `item_bytes(index)`. Item output is a **pure
function of the index** (given the seed/job inputs), which is the root of
byte-equivalence: the items after a checkpoint are fully determined, so a
resumed worker reproduces them exactly. `SeededItemSource` is the
host-independent implementation used by the drills.

### 3.2 Checkpoints (`record` + `store`)

While running, every `checkpoint_every` items the worker writes a
`Checkpoint { job_id, next_item, output_len, output_fnv }` where
`output_fnv` is the FNV-1a64 hash of the artifact prefix `[0..output_len)`.
That hash is the **byte-equivalence gate**. The `CheckpointStore` trait is
the durability seam: `MemoryStore` for tests, `DirStore` (atomic
temp-file + rename) for the real eviction drill. The format is self-contained
and integrity-checked (magic `OEPF`, version, bounds, trailing FNV tag).

### 3.3 The runner (`farm`)

`FarmRunner::run` runs (or resumes) a job to completion; `run_bounded` with
`Some(n)` stops after `n` items of this attempt with `completed = false`,
simulating worker eviction. On (re)start the runner:

1. loads the latest checkpoint;
2. **verifies** the artifact prefix matches the recorded hash/length and
   refuses on mismatch (`FarmError::Corruption`) or a missing artifact
   (`ArtifactShorterThanCheckpoint`) - it never silently diverges from a
   clean run;
3. rewinds the artifact to the checkpoint boundary (dropping any partial
   item a dying worker started);
4. resumes from `next_item`.

## 4. Acceptance proof

`crates/oep-farm/tests/checkpoint_resume_acceptance.rs` and the
`oep-farm-drill` binary run one deterministic long job (400 items, 512 bytes
each) twice against a real temporary directory:

- a **clean run** (never evicted);
- a **resumed run**: a fresh worker evicted after 117 items (a non-boundary
  count), then a brand-new worker object resumes from the last checkpoint
  (item 112) and completes.

The two artifacts are compared byte-for-byte. Full evidence file:
`P9-M4/proof/checkpoint-resume-proof.json` (clean and resumed artifact bytes
are SHA-256 identical). The Python acceptance test
(`tests/test_p9_m4_checkpoint_resume.py`) recomputes SHA-256 over both proof
artifacts independently.

Additional cases covered by the Rust suite:

- repeated evictions (multiple worker deaths) still converge to the clean
  artifact bytes;
- a corrupted artifact prefix refuses to resume (byte-equivalence gate);
- a lost artifact refuses to resume (no fabricated output);
- clean runs are reproducible across separate workers and configs.

## 5. What "verified" means and does not

- Verified: resume is byte-for-byte identical to a clean run on output, on
  a deterministic job, in CI. This is the card acceptance.
- Deliberately out of scope here: GPU scheduling (P9-M4-T01, sibling card),
  the ops dashboard (P9-M5-T01), and wiring this onto the live NATS bus and
  the real P8 vault artifact store (later milestones).

## 6. Verification commands

```text
cargo test -p oep-farm
cargo test --workspace
python3 tests/monorepo_layout_test.py
python3 -m pytest -q
python3 tests/test_p9_m4_checkpoint_resume.py
cargo run -p oep-farm --bin oep-farm-drill -- --out-dir <dir>
```
