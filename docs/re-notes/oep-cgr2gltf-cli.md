# oep-cgr2gltf CLI: CGR -> glTF/GLB converter

P2-M5-T01 · Keelworks / OEP P2 CGR Reader
Status: verified on real material 2026-10-15 (acceptance test GREEN; real
corpus classified-skip with reasons, per P2-M1-T04 tolerant-failure doctrine)
Version: 1.0

## 1. What this card delivers

A standalone command-line converter from a CATIA Graphics Representation
(.cgr) file to glTF/GLB:

- `crates/oep-cgr/src/bin/oep-cgr2gltf.rs` — the CLI binary, living inside
  the `oep-cgr` crate so it can call the crate's clean-room extractor without
  a cross-crate path dependency (layout.md convention: no cross-crate path
  deps until the P6 importer hub).
- `crates/oep-cgr/tests/cli_acceptance.rs` — the TDD acceptance test,
  written first (RED: `CARGO_BIN_EXE_` referenced a binary that did not yet
  exist), then GREEN.
- `docs/re-notes/oep-cgr2gltf-cli.md` — this document: usage, options, and
  the exit-code contract (also printed in `--help`).

Pipeline: read input bytes -> `oep_cgr::mesh::extract` (validates the CGR
header spine AND latches a structurally-verified triangle mesh, or returns a
classified skip) -> write GLB (default) / glTF (JSON with embedded base64
BIN). The CLI never panics on any input; every outcome maps to a documented
exit code.

## 2. Exit-code contract (the CLI's stability promise)

The exit-code contract is the CLI's machine-readable stability promise: any
automation may depend on `$?` meaning the same thing on every run and on
every host. Codes are stable and documented:

```text
  0  success (output written)
  1  usage error (bad/unknown arguments, more than one input)
  2  input I/O error (file missing or unreadable)
  3  not a CGR container (header spine mismatch)
  4  classified skip (valid CGR but no usable tessellation found)
  5  output I/O error (cannot create/write output file)
  6  internal error (extractor produced an invalid mesh)
```

Rules that make the contract trustworthy:

- Exits 2-6 are mutually exclusive: an invocation produces exactly one
  terminal code, and the diagnostic for it is written to stderr.
- Successful runs never write to stderr unless `--verbose` is given.
- No signal exits / no panics: even truncated, garbage, or adversarial
  inputs produce one of the documented codes.

## 3. Usage

```text
oep-cgr2gltf — convert a CATIA Graphics Representation (.cgr) file to glTF/GLB.

USAGE:
    oep-cgr2gltf [OPTIONS] <INPUT.cgr>

OPTIONS:
    -o, --output <PATH>   Output file. Extension .glb (default) or .gltf.
    -f, --format <FMT>    Force output format: 'glb' or 'gltf'
                          (default: inferred from output extension, else glb).
    -v, --verbose         Print extraction details to stderr.
    --version             Print version and exit.
    -h, --help            Print this help and exit.
```

Examples:

```text
oep-cgr2gltf part.cgr                              # -> part.glb
oep-cgr2gltf -o out.gltf part.cgr                  # -> JSON glTF with embedded BIN
oep-cgr2gltf -v part.cgr                           # verbose extraction info
```

## 4. Output format (clean-room, minimal)

The embedded writer emits a valid glTF 2.0 GLB container:

- Header: magic `glTF`, version 2, total length patched after padding.
- JSON chunk: `asset.version "2.0"`, one scene with one node, one mesh with
  one primitive (`mode: 4` = TRIANGLES); `POSITION` accessor (VEC3 float32)
  with computed `min`/`max`, `indices` accessor (SCALAR uint32, component
  type 5125), buffer/bufferView with `target: 34962` (ARRAY_BUFFER).
- BIN chunk: interleaved positions then triangle indices, aligned to 4 bytes.

The `.gltf` (JSON) variant is the same model with the BIN payload embedded as
a base64 data URI.

Assertions in the acceptance test prove the GLB is structurally sound:
magic present, version == 2, a `JSON` chunk is present, and (on success) the
output file exists. A full visual renderer check is the scope of the P2-M2-T03
crate-level glTF writer card.

Scope note / seam: the crate-level glTF/GLB writer is P2-M2-T03 (a parallel
card). This CLI embeds a minimal self-contained GLB writer so the CLI is
independently verifiable today; when P2-M2-T03 lands its `oep_cgr::gltf`
module, the CLI's `write_glb` / `write_gltf_json` calls are one function-call
seam to swap.

## 5. What the TDD acceptance test locks

Part 1 (deterministic, no real bytes needed):

- `--version` exits 0 and names the program; `--help` exits 0, prints usage,
  and documents the exit-code contract.
- Usage errors (too many positionals, unknown option, no arguments at all)
  exit 1.
- A missing input file exits 2.
- Non-CGR bytes (garbage) exit 3 (header spine mismatch).
- A synthetic indexed-triangle payload (the SAME observed-facts pattern the
  P2-M2-T01 golden test builds, so the same code path is exercised) exits 0
  and writes a GLB with correct magic/version/JSON chunk.

Part 2 (real material, skip-clean): when a private corpus is supplied via
`OEP_CGR_SAMPLES`, every `.cgr` sample yields a *classified* outcome — a
valid GLB or one of the documented exit codes — and the CLI never crashes /
signals on any sample. The test skips cleanly when no corpus is supplied so
CI stays green before the (license-clean, not-committed) corpus sync.

## 6. Real-material verification transcript (2026-10-15)

Run: `OEP_CGR_SAMPLES=<11 real .cgr files> cargo test --test cli_acceptance -- --nocapture`

```text
classified (exit 4) "ATOMIZER.cgr":  classified skip: no self-consistent triangle stream found
classified (exit 4) "BODY1.cgr":     classified skip: no self-consistent triangle stream found
classified (exit 4) "BODY2.cgr":     classified skip: no self-consistent triangle stream found
classified (exit 4) "LOCK.cgr":      classified skip: no self-consistent triangle stream found
classified (exit 4) "NOZZLE1.cgr":   classified skip: no self-consistent triangle stream found
classified (exit 4) "NOZZLE2.cgr":   classified skip: no self-consistent triangle stream found
classified (exit 4) "REGULATION_COMMAND.cgr": classified skip: no self-consistent triangle stream found
classified (exit 4) "REGULATOR.cgr": classified skip: no self-consistent triangle stream found
classified (exit 4) "TRIGGER.cgr":   classified skip: no self-consistent triangle stream found
classified (exit 4) "VALVE.cgr":     classified skip: no self-consistent triangle stream found
classified (exit 4) "piston.cgr":    classified skip: no self-consistent triangle stream found
summary: 0 success, 11 classified of 11 samples
test result: ok. 6 passed; 0 failed
```

Honesty note: 0 meshes extracted today because the P2-M2-T01 extractor's
structural hypotheses do not yet lock the real old-era payload's triangle
stream (see `docs/re-notes/oep-cgr-mesh-extraction.md` §3 for the full
byte-level recon). The CLI faithfully reports exit 4 (classified skip) with
the reason instead of fabricating geometry — exactly the P2-M1-T04 doctrine
the reader is built on. As P2-M1-T03 (block walker) and P2-M1-T06 (diff
harness) land and `extract` flips corpus samples to `Ok(mesh)`, this same
unmodified CLI will produce GLB output for them with no code change.

A modern-era 0x55-family file (`Downloads/VALVE.cgr`, magic `55 00 00 00
11 44 41 53`) is a different container and correctly exits 3 (not a CGR
container under the observed header spine) — recorded as the known
not-a-CGR boundary, not a bug.

## 7. Open items

- Land P2-M2-T03 and switch `write_glb`/`write_gltf_json` to `oep_cgr::gltf`
  (one-call seam; drop the embedded writer).
- P2-M3-T01 (normals) / P2-M4-T01 (materials) will add NORMAL and material
  slots to the primitive; the CLI should accept and forward them once those
  crates land (documented seam, not implemented today).
- Add `-q/--quiet`, `--binary` alias, and a machine-readable `--exit-code`
  doc drawer only if automation asks (YAGNI).

## 8. Clean-room statement

All byte-level layout facts encoded by the extractor are observed facts from
public sample files, read by pure-stdlib probes on 2026-10-15. No
Dassault/Spatial library read any CGR; no vendor binary was executed; no
proprietary CGR bytes are committed. The synthetic success fixture is built
from the same observed header-spine + indexed-triangle pattern the P2-M2-T01
golden test uses, so the success path is proven without proprietary input.
