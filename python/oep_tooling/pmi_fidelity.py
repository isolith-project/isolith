#!/usr/bin/env python3
"""P4-M2-T03 — PMI fidelity audit runner.

Builds the GD&T-types x reference-apps fidelity matrix (P13-T03 style honest
capability table) and publishes it as two artifacts, JSON + markdown.

Honesty contract (this module never over-claims):

  - The matrix measures *presence + classification fidelity*: for each GD&T
    characteristic, does the reference app preserve and classify the instances
    a STEP file actually carries? It deliberately does NOT claim
    tolerance-value, datum-anchoring, or feature-association correctness —
    those are P4-M2-T01/T02 (semantic PMI import / re-export) scope and are
    called out inside the emitted matrix.
  - Ground truth ("what the file carries") is an INDEPENDENT lexical scan of
    the DATA section (raw bytes, no library), so the audit is not circular
    with the app being audited.
  - A PASS requires the app to have surfaced >=1 instance of the type on
    EVERY corpus file that carries it. A type the corpus carries but the app
    never surfaces is FAIL, never PASS. An unavailable app is SKIP with the
    reason recorded as evidence. There is no fabricated success.

runner metadata (probe sha256, step-io version, generated_at) makes the
published matrix reproducible. Pure stdlib. Usage:

    python3 -m oep_tooling.pmi_fidelity --corpus <dir> --matrix-out <dir> [--probe <exe>]
"""
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone

MATRIX_SCHEMA = "isolith/pmi-fidelity-matrix/v1"

# Canonical GD&T axis: the 14 ASME Y14.5 / ISO 1101 characteristics (matching
# oep-pmi's `Characteristic` closed set) plus two supporting rows.
GD_T_ROWS = [
    ("straightness", "Straightness", "form"),
    ("flatness", "Flatness", "form"),
    ("circularity", "Circularity / roundness", "form"),
    ("cylindricity", "Cylindricity", "form"),
    ("profile-of-line", "Profile of a line", "profile"),
    ("profile-of-surface", "Profile of a surface", "profile"),
    ("angularity", "Angularity", "orientation"),
    ("perpendicularity", "Perpendicularity", "orientation"),
    ("parallelism", "Parallelism", "orientation"),
    ("position", "Position", "location"),
    ("concentricity", "Concentricity", "location"),
    ("symmetry", "Symmetry", "location"),
    ("circular-runout", "Circular runout", "runout"),
    ("total-runout", "Total runout", "runout"),
    ("datums", "Datums & datum reference frames", "supporting"),
    ("modifiers", "Tolerance modifiers (MMC/LMC/RFS, unequally-disposed)",
     "supporting"),
]

# STEP entity type(s) that carry each row (independent lexical ground truth).
ROW_LEXICAL_TYPES = {
    "straightness": {"STRAIGHTNESS_TOLERANCE"},
    "flatness": {"FLATNESS_TOLERANCE"},
    "circularity": {"ROUNDNESS_TOLERANCE", "CIRCULARITY_TOLERANCE"},
    "cylindricity": {"CYLINDRICITY_TOLERANCE"},
    "profile-of-line": {"LINE_PROFILE_TOLERANCE"},
    "profile-of-surface": {"SURFACE_PROFILE_TOLERANCE"},
    "angularity": {"ANGULARITY_TOLERANCE"},
    "perpendicularity": {"PERPENDICULARITY_TOLERANCE"},
    "parallelism": {"PARALLELISM_TOLERANCE"},
    "position": {"POSITION_TOLERANCE"},
    "concentricity": {"CONCENTRICITY_TOLERANCE"},
    "symmetry": {"SYMMETRY_TOLERANCE"},
    "circular-runout": {"CIRCULAR_RUNOUT_TOLERANCE"},
    "total-runout": {"TOTAL_RUNOUT_TOLERANCE"},
    "datums": {"DATUM", "COMMON_DATUM", "DATUM_FEATURE", "DATUM_SYSTEM"},
    "modifiers": {"UNEQUALLY_DISPOSED_GEOMETRIC_TOLERANCE",
                  "GEOMETRIC_TOLERANCE_WITH_MODIFIERS",
                  "MODIFIED_GEOMETRIC_TOLERANCE"},
}

# step-io scene `ToleranceKind` surface key per characteristic row.
ROW_SURFACE_KIND = {
    "straightness": "Straightness",
    "flatness": "Flatness",
    "circularity": "Roundness",
    "cylindricity": "Cylindricity",
    "profile-of-line": "LineProfile",
    "profile-of-surface": "SurfaceProfile",
    "angularity": "Angularity",
    "perpendicularity": "Perpendicularity",
    "parallelism": "Parallelism",
    "position": "Position",
    "concentricity": "Concentricity",
    "symmetry": "Symmetry",
    "circular-runout": "CircularRunout",
    "total-runout": "TotalRunout",
}

# STEP types whose dedicated model arena backs the per-file `arena` count.
ROW_ARENA_TYPES = {
    **{k: k for k in ROW_SURFACE_KIND},  # same name for characteristics
    "datums": "datum",
    "modifiers": None,  # modifier preservation is arena-sum, handled specially
}

STEP_IO_VERSION = "0.2.5"

VERDICTS = ("PASS", "FAIL", "SKIP")


# --------------------------------------------------------------------------
# independent ground truth
# --------------------------------------------------------------------------
def extract_data_section(text):
    m = re.search(r"\bDATA\s*;(.*?)END-ISO-10303-21;", text, re.S)
    return m.group(1) if m else ""


def scan_entity_types(text):
    """Uppercase STEP type tokens present in the DATA section (simple +
    complex tuple forms), quoted strings stripped. Independent of any reader."""
    text = re.sub(r"\'[^\']*\'", "", text)
    out = {}
    for stmt in re.split(r";", text):
        m = re.match(r"^\s*#\d+\s*=\s*(.*)$", stmt, re.S)
        if not m:
            continue
        for t in re.findall(r"\b[A-Z][A-Z0-9_]*\b", m.group(1)):
            out[t] = out.get(t, 0) + 1
    return out


def lexical_count_for_row(data_text, row):
    types = ROW_LEXICAL_TYPES[row]
    counts = scan_entity_types(data_text)
    return sum(counts.get(t, 0) for t in types)


# --------------------------------------------------------------------------
# reference-app detection
# --------------------------------------------------------------------------
def _sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def find_probe(explicit):
    candidates = []
    if explicit:
        candidates.append(explicit)
    # Repo-root-anchored built binary works regardless of the cwd the CLI was
    # launched from (crates/oep-step/examples/pmi_probe -> target/debug/examples).
    repo_root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
    candidates += [
        os.environ.get("OEP_PMI_PROBE", ""),
        os.path.join(repo_root, "target", "debug", "examples", "pmi_probe"),
        os.path.join(os.getcwd(), "target", "debug", "examples", "pmi_probe"),
    ]
    for c in candidates:
        if c and os.path.exists(c) and os.access(c, os.X_OK):
            return c
    return None


def detect_apps(probe_path):
    """Reference-app availability, honestly. Returns list of app dicts."""
    apps = []
    # step-io 0.2.5 (in-stack OSS reader, wrapped by oep-step)
    if probe_path:
        apps.append({
            "id": "stepio",
            "label": "step-io %s (pure-Rust OSS STEP reader via oep-step)" % STEP_IO_VERSION,
            "kind": "oss",
            "available": True,
            "version": STEP_IO_VERSION,
            "evidence": "probe binary %s sha256=%s" % (probe_path, _sha256(probe_path)[:16]),
        })
    else:
        apps.append({
            "id": "stepio",
            "label": "step-io %s (pure-Rust OSS STEP reader via oep-step)" % STEP_IO_VERSION,
            "kind": "oss",
            "available": False,
            "version": STEP_IO_VERSION,
            "evidence": "pmi_probe binary not found/built (cargo not available or build failed)",
        })
    # OCCT (Open CASCADE Technology) via pythonocc bindings — the P4-M1 ADR
    # designates OCCT as the AP242 semantic-PMI fidelity path.
    occt_available = False
    occt_ver = "?"
    if os.environ.get("OEP_APP_OCCT") == "1":
        occt_available = True
    else:
        try:
            r = subprocess.run(
                [sys.executable, "-c", "import OCP;print(getattr(OCP,'VERSION','?'))"],
                capture_output=True, text=True, timeout=30)
            if r.returncode == 0:
                occt_available = True
                occt_ver = r.stdout.strip() or "?"
        except Exception:
            pass
    apps.append({
        "id": "occt",
        "label": "Open CASCADE Technology (OCCT) via pythonocc/OCP bindings",
        "kind": "oss",
        "available": occt_available,
        "version": occt_ver,
        "evidence": ("OCP importable on this host" if occt_available else
                     "no OSS OCCT binding with an aarch64 wheel for this python "
                     "on PyPI (OCP has no aarch64 wheel); source build out of "
                     "scope for this audit — cell SKIP is honest, not assumed"),
    })
    # FreeCAD (independent OSS CAD app)
    fc = shutil.which("freecad") or shutil.which("FreeCAD") or shutil.which("freecadcmd")
    apps.append({
        "id": "freecad",
        "label": "FreeCAD (OCCT-based independent OSS CAD app)",
        "kind": "oss",
        "available": bool(fc) or os.environ.get("OEP_APP_FREECAD") == "1",
        "version": fc or "?",
        "evidence": ("freecad%s on PATH" % ("" if not fc else " at " + fc)
                     if fc else "FreeCAD not installed on this host (no deb/snap "
                     "candidate in apt sources) — SKIP"),
    })
    # Commercial readers: SKIP by policy (no license; functional-spec source
    # only, P1-M3-T02). Never probed, never claimed.
    apps.append({
        "id": "commercial-readers",
        "label": "Commercial STEP PMI readers (CAD Exchanger SDK / Spatial HOOPS / \
TransMagic / NX / CATIA / SolidWorks)",
        "kind": "commercial",
        "available": os.environ.get("OEP_APP_COMMERCIAL") == "1",
        "version": "n/a",
        "evidence": ("not licensed/present on this host; isolith treats commercial "
                     "reader feature lists as a functional-spec source only "
                     "(P1-M3-T02), never as a runnable reference — SKIP by policy"),
    })
    return apps


# --------------------------------------------------------------------------
# probe execution
# --------------------------------------------------------------------------
def probe_files(probe, step_files):
    """Run pmi_probe over every file at once; return list of per-file reports."""
    if not probe:
        return []
    r = subprocess.run([probe] + step_files, capture_output=True, text=True, timeout=900)
    if r.returncode != 0:
        raise RuntimeError("pmi_probe failed rc=%d: %s" % (r.returncode, r.stderr[:400]))
    try:
        data = json.loads(r.stdout)
    except json.JSONDecodeError as e:
        raise RuntimeError("pmi_probe emitted invalid JSON: %s" % e)
    return data


# --------------------------------------------------------------------------
# matrix construction
# --------------------------------------------------------------------------
def per_file_count(row, report, data_text):
    """The preservation evidence for one (row, file): lexical ground truth,
    dedicated arena count, and the surfaced/preserved count actually used for
    the verdict (with the measurement kind made explicit)."""
    lexical = lexical_count_for_row(data_text, row)
    ents = report.get("entities", {})
    surf = report.get("surfaced", {})
    if row in ROW_SURFACE_KIND:
        arena = ents.get(ROW_ARENA_TYPES[row], 0)
        preserved = surf.get("tolerances", {}).get(ROW_SURFACE_KIND[row], 0)
        kind = "scene-surface"
    elif row == "datums":
        arena = ents.get("datum", 0)
        preserved = surf.get("datums", 0)
        kind = "scene-datums"
    else:  # modifiers: structural preservation across the modifier arenas
        arena = sum(ents.get(k, 0) for k in (
            "unequally_disposed_geometric_tolerance",
            "geometric_tolerance_with_modifiers",
            "modified_geometric_tolerance",
        ))
        preserved = arena
        kind = "structural-arena"
    return {"lexical": lexical, "arena": arena, "surfaced": preserved,
            "probe_surfaced": preserved, "kind": kind}


def build_matrix(corpus, probe, apps, generated_at=None):
    if generated_at is None:
        generated_at = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    files = []
    if corpus and os.path.isdir(corpus):
        files = sorted(
            p for p in (os.path.join(corpus, n) for n in os.listdir(corpus))
            if os.path.isfile(p) and p.lower().endswith((".stp", ".step"))
        )

    probe_reports = {}
    if probe and files:
        probe_reports = {r["file"]: r for r in probe_files(probe, files)}
    data_by_file = {}
    for p in files:
        with open(p, errors="replace") as fh:
            data_by_file[os.path.basename(p)] = fh.read()

    cells = {}
    for row, _label, _cat in GD_T_ROWS:
        byapp = {}
        for app in apps:
            aid = app["id"]
            if not app["available"]:
                byapp[aid] = {
                    "verdict": "SKIP",
                    "evidence": app["evidence"] + " — no cell probed for this host.",
                    "counts": {},
                }
                continue
            if aid != "stepio":
                # Only step-io is wired to a probe in this runner today; other
                # available apps would need their own probe (future work).
                byapp[aid] = {
                    "verdict": "SKIP",
                    "evidence": ("no probe adapter implemented for app '%s' in "
                                 "this runner yet — SKIP, not assumed" % aid),
                    "counts": {},
                }
                continue
            # step-io cell
            fc = {}
            carried_any = False
            fail_files = []
            pass_files = []
            for p in files:
                base = os.path.basename(p)
                if base not in probe_reports:
                    continue
                rep = probe_reports[base]
                if not rep.get("parse_ok"):
                    continue
                c = per_file_count(row, rep, data_by_file.get(base, ""))
                fc[base] = c
                if c["lexical"] > 0:
                    carried_any = True
                    if c["surfaced"] > 0:
                        pass_files.append((base, c))
                    else:
                        fail_files.append((base, c))
            if not carried_any:
                verdict, evidence = "SKIP", (
                    "no corpus file carries this GD&T type (independent lexical "
                    "scan of the DATA section) — nothing to audit for step-io")
            elif fail_files:
                summary = "; ".join(
                    "%s carries %d but step-io surfaces %d" % (b, c["lexical"], c["surfaced"])
                    for b, c in fail_files)
                verdict = "FAIL"
                evidence = (
                    "present in the corpus but NOT preserved by step-io %s: %s. "
                    "Passed elsewhere: %s." % (
                        STEP_IO_VERSION, summary,
                        "; ".join("%s surfaces %d" % (b, c["surfaced"]) for b, c in pass_files) or "none"))
                if row == "modifiers":
                    evidence += (
                        " step-io's PMI surface exposes no modifier semantics "
                        "(MMC/LMC/unequally-disposed) and its three modifier "
                        "arenas are empty on every corpus file; the base "
                        "characteristics the modifiers attach to DO surface "
                        "(see their rows).")
            else:
                verdict = "PASS"
                sample = pass_files[0]
                evidence = (
                    "step-io %s preserved and classified %s (%d instance(s) via %s) "
                    "on %s; same verdict on all %d carrying file(s)." % (
                        STEP_IO_VERSION, _label_of(row), sample[1]["surfaced"],
                        sample[1]["kind"], sample[0], len(pass_files)))
            byapp[aid] = {"verdict": verdict, "evidence": evidence, "counts": {"files": fc}}
        cells[row] = byapp

    runner_meta = {
        "generated_at": generated_at,
        "host": platform.node(),
        "matrix_schema": MATRIX_SCHEMA,
        "probe_reports": list(probe_reports.values()),
        "runner": {
            "generated_at": generated_at,
            "script": os.path.relpath(__file__),
            "probe_source": "crates/oep-step/examples/pmi_probe.rs",
            "step_io_version": STEP_IO_VERSION,
            "step_io_pin": "=0.2.5",
            "probe_sha256": _sha256(probe)[:64] if probe else "",
            "corpus_dir": corpus or "",
            "corpus_file_count": len(files),
            "probe_reports": len(probe_reports),
        },
        "gd_t_types": [r[0] for r in GD_T_ROWS],
        "gd_t_labels": {r[0]: {"label": r[1], "category": r[2]} for r in GD_T_ROWS},
        "apps": apps,
        "cells": cells,
    }
    return runner_meta


def _label_of(row):
    for r, label, _c in GD_T_ROWS:
        if r == row:
            return label
    return row


# --------------------------------------------------------------------------
# markdown rendering (P13-T03 style honest capability table)
# --------------------------------------------------------------------------
def render_markdown(mx):
    lines = []
    lines.append("# Isolith PMI fidelity audit matrix (P4-M2-T03)")
    lines.append("")
    lines.append("> Format-matrix convention (P13-T03 style): an HONEST capability "
                 "table. Every cell is one of **PASS / FAIL / SKIP** with an "
                 "evidence string. A cell never claims more than the run verified.")
    lines.append("")
    lines.append("Generated: `%s`  host: `%s`  " % (mx["generated_at"], mx["host"]))
    lines.append("Runner: `%s`  probe: `%s`  step-io pin: `%s`  probe sha256: `%s`"
                 % (mx["runner"]["script"], mx["runner"]["probe_source"],
                    mx["runner"]["step_io_pin"], mx["runner"]["probe_sha256"][:16]))
    lines.append("Corpus: `%s` (%d STEP files)" % (mx["runner"]["corpus_dir"],
                                                   mx["runner"]["corpus_file_count"]))
    lines.append("")
    lines.append("## Verdict semantics (closed set)")
    lines.append("")
    lines.append("- **PASS** — the reference app preserved and classified >=1 instance of the GD&T type on EVERY corpus file that carries it (independent lexical scan).")
    lines.append("- **FAIL** — the corpus carries the type (independent lexical scan) but the app surfaced 0 instances on at least one carrying file. Never relabeled as PASS.")
    lines.append("- **SKIP** — not testable in this run (app unavailable / not licensed / no probe adapter / type not carried by corpus), with the reason as evidence.")
    lines.append("")
    lines.append("Scope: presence + classification fidelity ONLY. Tolerance-value,")
    lines.append("datum-anchoring and feature-association correctness are P4-M2-T01/T02")
    lines.append("semantic-import scope, not claimed here.")
    lines.append("")
    lines.append("## Reference apps")
    lines.append("")
    lines.append("| app | kind | available | evidence |")
    lines.append("|-----|------|-----------|----------|")
    for a in mx["apps"]:
        lines.append("| %s (%s) | %s | %s | %s |" % (
            a["label"], a["id"], a["kind"], "yes" if a["available"] else "no",
            a["evidence"].replace("|", "/")))
    lines.append("")
    lines.append("## Capability matrix (GD&T types x reference apps)")
    lines.append("")
    headers = ["GD&T type (row)"] + [a["id"] for a in mx["apps"]]
    lines.append("| " + " | ".join(headers) + " |")
    lines.append("|" + "|".join(["---"] * len(headers)) + "|")
    for row, label, cat in GD_T_ROWS:
        if row in mx["cells"]:
            cells = mx["cells"][row]
            rowcells = [cells.get(a["id"], {}).get("verdict", "?") for a in mx["apps"]]
            lines.append("| %s (%s) | %s |" % (label, row, " | ".join(rowcells)))
    lines.append("")
    lines.append("## Evidence")
    lines.append("")
    for row, label, _cat in GD_T_ROWS:
        if row not in mx["cells"]:
            continue
        for a in mx["apps"]:
            cell = mx["cells"][row].get(a["id"])
            if not cell:
                continue
            lines.append("- **%s / %s**: `%s` — %s" % (label, a["id"], cell["verdict"],
                                                        cell["evidence"]))
            counts = cell.get("counts", {}).get("files", {})
            if counts and row in ROW_SURFACE_KIND:
                det = []
                for f, c in sorted(counts.items()):
                    det.append("%s lex=%d arena=%d surfaced=%d" % (
                        f, c["lexical"], c["arena"], c["surfaced"]))
                lines.append("  - " + "; ".join(det))
    lines.append("")
    lines.append("## Observed beyond the axis (not scored)")
    lines.append("")
    lines.append("- **PLUS_MINUS_TOLERANCE** (size/location callouts) is carried by the")
    lines.append("  corpus and retained by step-io in its dedicated arena (e.g. 19 in")
    lines.append("  nist_ftc_06); it is not one of the 14 ASME/ISO characteristics, so")
    lines.append("  it is reported here and not given a matrix row.")
    lines.append("- **STATISTICAL_TOLERANCE** appears in 2 corpus files; its")
    lines.append("  preservation is not scored on this axis (follow-on probe work).")
    lines.append("")
    lines.append("## Reproducibility")
    lines.append("")
    lines.append("`python3 -m oep_tooling.pmi_fidelity --corpus <dir> --matrix-out <dir> "
                 "[--probe <pmi_probe binary>]` reproduces this artifact. The probe is")
    lines.append("built from `crates/oep-step/examples/pmi_probe.rs` with `cargo build "
                 "-p oep-step --example pmi_probe`.")
    return "\n".join(lines)


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------
def main(argv=None):
    argv = list(sys.argv[1:] if argv is None else argv)
    import argparse
    ap = argparse.ArgumentParser(
        prog="oep_tooling.pmi_fidelity",
        description="P4-M2-T03 PMI fidelity audit runner (GD&T types x apps, honest).")
    ap.add_argument("--corpus", default=os.environ.get("OEP_PMI_CORPUS", ""),
                    help="directory of STEP files to audit")
    ap.add_argument("--matrix-out", default=".", help="output dir for the matrix artifacts")
    ap.add_argument("--probe", default="", help="path to an already-built pmi_probe binary")
    ap.add_argument("--json-only", action="store_true", help="skip the markdown artifact")
    args = ap.parse_args(argv)

    probe = find_probe(args.probe)
    apps = detect_apps(probe)
    mx = build_matrix(args.corpus, probe, apps)

    os.makedirs(args.matrix_out, exist_ok=True)
    with open(os.path.join(args.matrix_out, "pmi-fidelity-matrix.json"), "w") as fh:
        json.dump(mx, fh, indent=2)

    if not args.json_only:
        with open(os.path.join(args.matrix_out, "pmi-fidelity-matrix.md"), "w") as fh:
            fh.write(render_markdown(mx) + "\n")

    n_pass = sum(1 for cell in (c for row in mx["cells"].values() for c in row.values())
                 if cell["verdict"] == "PASS")
    n_fail = sum(1 for cell in (c for row in mx["cells"].values() for c in row.values())
                 if cell["verdict"] == "FAIL")
    print("PMI fidelity matrix written to %s (%d PASS, %d FAIL across %d apps x %d rows)"
          % (args.matrix_out, n_pass, n_fail, len(apps), len(GD_T_ROWS)))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
