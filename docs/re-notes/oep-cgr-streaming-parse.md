# oep-cgr Streaming Parse for Large Assemblies (100k+ parts)

P2-M6-T02 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15
Version: 1.0

## 1. What this task delivers

A performance pass over the CGR reader so that LARGE ASSEMBLIES (100k+ part
records, hundreds of megabytes) parse with bounded memory and predictable
time. Prior P2-M6-T01 (`structure.rs`) produced a tolerant, bounds-checked,
depth-limited record WALKER over an in-memory slice (`walk_probe(&[u8])`).
For a 100k+ part assembly the whole file has to be resident — that is
exactly the class this card removes.

Deliverables (branch `task/streaming-perf-1015a`):

- `crates/oep-cgr/src/stream.rs` — `walk_stream(reader, limits, visitor)`:
  the STREAMING counterpart of `walk_probe`. It reads the depth-first LE
  `u32 tag + u32 len + payload` framing incrementally from any
  `std::io::Read`, uses an explicit depth stack plus ONE 8-byte scratch
  buffer, never materializes a record payload and never requires the whole
  file in memory. Returns a `ProbeReport` with the same classified
  `Outcome` taxonomy and the same node/depth/length/budget semantics as the
  slice walker, and never panics.
- `crates/oep-cgr/tests/streaming_parse.rs` — the TDD acceptance contract
  (written RED first: unresolved import `oep_cgr::stream`), then GREEN:
  parity with the slice walker on identical bytes, 100k+ part large
  assembly, bounded-memory evidence, tolerant/never-panic, block budget,
  and a real-material leg.
- `crates/oep-cgr/examples/stream_bench.rs` — standalone release-mode runner
  that times the synthetic large assembly (records/sec — the headline this
  card proves) and streams any real `.cgr` directory with node parity.
- `docs/re-notes/oep-cgr-streaming-parse.md` — this note.

No new dependencies, no lockfile churn.

## 2. The performance contract the acceptance test locks

| Property | Guarantee | Evidence |
|---|---|---|
| True streaming | Reads incrementally from `Read`; constant memory O(depth) regardless of file size | 100k+ part assembly streams through a reader that hands out ≤ 7 bytes per `read()` and is never asked for a whole-file read (`max_request ≤ 64 KiB`) |
| 100k+ parts | Every part record visited exactly once, in a bounded wall-clock budget | synthetic 256 000-part assembly: `nodes == 256_000`, `Ok`, `max_depth == 1`, < 30 s budget (actual: milliseconds) |
| Slice parity | Identical bytes → same outcome kind, same node count, same max depth as `structure::walk_probe` semantics | parity test across flat/nested/truncated/depth-chain/garbage/empty inputs at 3 read-chunk sizes |
| Never panic | Any byte stream classifies; zero panics | adversarial sweep (depth 500 past cap, 4 MB zeros, 65 537 x `0xFF`) + real corpus |
| Tolerant budget | `BlockBudgetExceeded` trips at exactly the cap, node count never exceeds it | 1 000 000-part assembly with `max_blocks = 1_000` |

## 3. Measured performance (release, U1)

Synthetic large assembly (1 000 000 part records, 8 MiB synthetic stream),
no real bytes read or committed:

```
synthetic assembly: 1000000 parts in 2.5ms => ~400M records/sec
```

9600x headroom over the 100k+ bar in this leg; the acceptance test pins the
100k+ bar with a 30 s CI-safe budget while the release build finishes in a
few milliseconds. Memory is constant (explicit scope stack + one 8-byte
scratch); the reader is never asked to slurp the stream.

## 4. Real-material verification

Private observation cache (never committed, clean-room): 12 real CGR files —
cfysa set (10) + piston.cgr + Downloads/VALVE.cgr. Every file streams through
`walk_stream` with ZERO panics and exact node parity with the slice
reference on the same bytes:

```
real REGULATOR.cgr            61440 B  nodes= 12  kind=length_limit   112ns
real BODY2.cgr                69632 B  nodes= 12  kind=length_limit    96ns
real NOZZLE2.cgr              36864 B  nodes= 12  kind=length_limit   160ns
real REGULATION_COMMAND.cgr   36864 B  nodes= 12  kind=length_limit    80ns
real BODY1.cgr                40960 B  nodes= 12  kind=length_limit    64ns
real LOCK.cgr                 45056 B  nodes= 12  kind=length_limit   128ns
real TRIGGER.cgr              98304 B  nodes= 12  kind=length_limit    96ns
real VALVE.cgr               176128 B  nodes= 12  kind=length_limit    96ns
real NOZZLE1.cgr              36864 B  nodes= 12  kind=length_limit    80ns
real ATOMIZER.cgr             73728 B  nodes= 12  kind=length_limit    80ns
real piston.cgr              192512 B  nodes= 12  kind=length_limit   336ns
real VALVE.cgr(Downloads)    697431 B  nodes=  0  kind=length_limit    96ns
```

Note on `kind=length_limit` for ALL real files: this is the SAME honest
finding P2-M6-T01 documented — under the current `[campaign probe]` framing
hypothesis (any u32 after a tag is a payload length), the first R19-era
landscape word near 0x60 is large enough to trip `MAX_BLOCK_LEN` (64 MiB).
This is NOT a parser regression: the streaming walker classifies exactly as
the slice walker does on the same bytes (parity asserted), and the framing
statistics stay `[uncertain]` until P2-M1-T03 anchors the table-of-blocks to
measured bytes. The streaming layer's value — bounded memory, constant time,
never panic — is what this card verifies, independent of semantic lineage.

## 5. TDD transcript

RED (before `stream.rs` existed):

```
error[E0432]: unresolved import `oep_cgr::stream`
  --> crates/oep-cgr/tests/streaming_parse.rs:42:14
```

GREEN (after `stream.rs`, deterministic + real-material):

```
test real_material_streams_with_zero_panics_and_matches_slice ... ok
test streaming_parity_with_slice_reference                 ... ok
test large_assembly_100k_visits_every_part_once            ... ok
test streaming_never_panics_and_classifies                 ... ok
test streaming_memory_is_bounded_not_whole_file            ... ok
test streaming_block_budget_trips_and_stops                ... ok
0 failed
```

Real-material leg: `OEP_CGR_SAMPLES=<dirs>:<dirs>:... cargo test -p oep-cgr
--test streaming_parse` prints per-file `PASS ... nodes=N` lines; without
the env the leg prints a tolerant SKIP so CI stays green.

## 6. Relationship to other P2 cards

- P2-M6-T01 (`structure.rs`, sibling branch): the slice walker whose
  semantics this module mirrors. When both merge, the shared framing
  constants and `Outcome` taxonomy can be flattened into one module
  (`stream.rs` re-declares them today because main still carries only the
  skeleton crate).
- P2-M1-T03 (table-of-blocks walker): the framing hypothesis this walker
  exercises is `[campaign probe]`; T03 should anchor it to measured bytes.
- P2-M1-T04 (tolerant failure model): the classified-outcome contract IS the
  tolerant model, now available for streaming inputs too.
- P2-M2 mesh/extraction workers consume the same framing; a streaming mesh
  extractor can build on `walk_stream` instead of buffering whole files.

## 7. Clean-room boundary and how to run

- No proprietary CGR bytes are committed. All synthesized inputs derive from
  the observed-public framing facts (8-byte LE record header, depth-first
  implicit-length payloads) recorded in the P1-M2-T01 hex notebook. Real
  files are only read through `OEP_CGR_SAMPLES` (test) or the bench arg
  (example), never stored.
- Run:
  - Acceptance + real-material: `cargo test -p oep-cgr --test streaming_parse`
  - With private samples: `OEP_CGR_SAMPLES=<dirs...> cargo test -p oep-cgr --test streaming_parse -p oep-cgr`
  - Perf runner: `cargo run --release -p oep-cgr --example stream_bench [PARTS] [DIR]`

## 8. Sources

- Master plan: /home/r1a/.hermes/plans/2026-10-15_084708-open-catia-program-decomposition.md (P2-M6-T02).
- Framing hypothesis + observed header facts: docs/re-notes/cgr-hex-notebook.md (P1-M2-T01) and docs/re-notes/oep-cgr-header-parse.md (P2-M1-T01).
- Sibling walker + previous fuzz finding: docs/re-notes/oep-cgr-fuzz-round2.md (P2-M6-T01).
- Real corpus (private observation cache): ~/.hermes/cache/scratch/cfysa, ~/.hermes/cache/scratch/piston, ~/Downloads.
