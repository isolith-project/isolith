# OCC probe (independent STEP reader) — P4-M3-T02

`occ_probe.cpp` + `build_occ_probe.sh` build a tiny C++ program that wraps the
Open CASCADE Technology (OCCT) STEP importer (`STEPControl_Reader`) — the exact
STEP reader FreeCAD uses for its Part/Import workbench. The P4-M3-T02 golden-diff
gate (`python/oep_tooling/step_export_validation.py`) runs this probe on our
exported `.step` files and keys the gate on what the independent kernel actually
recovers, never on our own parsers or writers.

## Why an independent reader

The P6-M4-T01 / P2-M5-T02 exporters are byte-locked to an independent *encoder*
(the golden generator), which proves the exported bytes match a spec-conformant
encoding. That does NOT prove an independent *reader* can import the file. OCCT
is the dominant OSS STEP engine (FreeCAD is a GUI over it); if OCCT transfers no
shape from our export, real users importing our STEP in mainstream tools get
nothing. This gate makes that relationship explicit and committed as a golden.

## Build

```bash
# one-time deps (Debian/Ubuntu):
sudo apt-get install libocct-data-exchange-dev libocct-modeling-algorithms-dev \
    libocct-modeling-data-dev libocct-foundation-dev

# build:
bash corpus/tools/build_occ_probe.sh
# -> corpus/tools/occ_probe  (not committed; a build artifact)
```

## Run

```bash
corpus/tools/occ_probe <file.step>
```

Deterministic `key=value` stdout:

```
read_status=1            # IFSelect_RetDone
roots=1                  # NbRootsForTransfer
shape_type=0|NULL|...    # TopAbs shape type; NULL = parsed but no transferable shape
solids=0 shells=12 faces=12 edges=36 vertices=72
volume=2.7755575615628914e-17
area=6
bbox_min_x=... bbox_min_y=... bbox_min_z=...
bbox_max_x=... bbox_max_y=... bbox_max_z=...
```

## FreeCAD cross-check (optional second leg)

FreeCAD 1.1 (snap `freecad.cmd`) is an app-level cross-check on the same
kernel. For the as-committed export it reports the same result as the raw probe
(parse OK / no shapes): `Part.insert` -> "No shapes found in file". For the
AP214-conventional reference it recovers a Compound with 12 faces. The gate's
canonical reader is the OCC probe (deterministic); FreeCAD is documented
evidence, not the gating path.
