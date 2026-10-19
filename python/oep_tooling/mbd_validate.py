"""P4-M5-T01: MBD validator service (rule engine + honest report).

Checks a *supplied model/PMI bundle* against a documented rule catalog with
three layers, mirroring the program master plan (missing PMI, dangling datums,
unit errors) on top of the oep-pmi (P6-M1-T02) structural invariants:

  STRUCTURAL  s01..s09  oep-pmi schema invariants (1:1 parity with
                        crates/oep-pmi/src/validate.rs rule codes).
  Y14.41      y01..y05  ASME Y14.41 MBD expectations on the PMI layer:
                        PMI present, datum features anchored, tolerances
                        anchored, no dangling datums, no orphan frames.
  UNITS       u01..u02  model units statement + PMI/model unit consistency.
  CROSS-MODEL c01       datum/tolerance/note id references resolve against
                        the supplied model facts (feature/body inventory).

Honesty doctrine (the "report output honest" acceptance): a rule emits PASS
only when the supplied data actually demonstrates it; a rule that cannot run
on the supplied data emits UNVERIFIED with a reason; the report carries a
scope/limits block stating it certifies the supplied data, never the
manufactured part. Nothing is ever fabricated into a PASS or FAIL.

Pure stdlib (pyproject dependencies stay []). Thin runner at
`python -m oep_tooling.mbd_validate`; the product CLI + exit codes are the
P4-M5-T02 card. Workstream: P4-STEP lane tooling (docs/layout.md: MBD
validator first cut = python/oep_tooling).
"""

from __future__ import annotations

import argparse
import json
import math
import os
import sys

# --------------------------------------------------------------------------
# Report / input format identity (schema documented in docs/re-notes).
# --------------------------------------------------------------------------
REPORT_FORMAT = "oep-mbd-validate"
REPORT_VERSION = 1
INPUT_SCHEMA_VERSION = 1
MODEL_FACTS_FORMAT = "oep-model-facts"
MODEL_FACTS_VERSION = 1
TOOL = {"name": "mbd_validate", "version": "0.1.0"}

STATUSES = ("pass", "fail", "warn", "unverified")


class MbdInputError(Exception):
    """Typed, recoverable input error: missing/malformed file or bundle.

    Raised by loaders; the runner catches it per fixture and records an error
    row in the report instead of crashing (tolerant-failure doctrine).
    """


# --------------------------------------------------------------------------
# Unit vocabulary (mirrors crates/oep-geo/src/units.rs LengthUnit::parse).
# --------------------------------------------------------------------------
_LENGTH_ALIASES = {
    "mm": "mm",
    "cm": "cm",
    "m": "m",
    "um": "um",
    "µm": "um",
    "\u03bcm": "um",
    "in": "in",
    "inch": "in",
    "inches": "in",
    "ft": "ft",
    "foot": "ft",
    "feet": "ft",
}
KNOWN_UNITS = sorted(_LENGTH_ALIASES)


def canonical_unit(symbol: str) -> str | None:
    """Normalize a length-unit symbol to its canonical form (case-insensitive).

    Returns None for blank/unknown symbols (never raises). Mirrors the
    oep-geo LengthUnit::parse acceptance set.
    """
    key = (symbol or "").strip().lower()
    return _LENGTH_ALIASES.get(key)


# --------------------------------------------------------------------------
# Rule catalog
# --------------------------------------------------------------------------
# Rule dict: id, name, category, sets (rule sets that apply it), severity
# (default), requirement (normative/basis text). The check functions live in
# the RULES map below; each returns {"status", "evidence", "detail"}.
RULE_CATALOG = [
    # ---- STRUCTURAL: oep-pmi schema invariants (all sets, error) ----------
    {"id": "s01-pmi-format", "name": "PMI document format tag",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "oep-pmi header format must be exactly 'oep-pmi' (bad-format)."},
    {"id": "s02-pmi-version", "name": "PMI document schema version",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "oep-pmi schema version must be 1 (unsupported-version)."},
    {"id": "s03-unique-ids", "name": "Unique ids across all PMI classes",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "One PMI document = one id space; ids unique across "
                    "datums, frames, tolerances and notes (duplicate-id)."},
    {"id": "s04-unique-labels", "name": "Unique non-blank datum labels",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "Datum labels are single letters, unique and non-blank "
                    "(empty-label, duplicate-label)."},
    {"id": "s05-refs-unique", "name": "No duplicate id in any reference list",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "A feature/body reference list holds each id at most once "
                    "(duplicate-ref)."},
    {"id": "s06-datum-frame-resolution", "name": "Frame datum refs resolve",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "Every frame datum_id resolves to a defined datum, with "
                    "precedence 1..=3 and no duplicate precedence in a frame "
                    "(unresolved-datum, invalid-precedence, "
                    "duplicate-precedence)."},
    {"id": "s07-tolerance-frame-resolution", "name": "Tolerance frame refs resolve",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "Every tolerance frame_id resolves to a defined frame "
                    "(unresolved-frame)."},
    {"id": "s08-tolerance-values", "name": "Finite non-negative tolerance values",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "Tolerance values and second values are finite and >= 0 "
                    "(non-finite-value)."},
    {"id": "s09-note-position", "name": "Well-formed note anchor positions",
     "category": "structural", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "A note position is exactly three finite numbers "
                    "(invalid-position)."},
    # ---- Y14.41: MBD expectations ----------------------------------------
    {"id": "y01-pmi-present", "name": "Model carries PMI (missing PMI check)",
     "category": "y4141", "sets": ["y4141", "prime"], "severity": "error",
     "requirement": "ASME Y14.41 MBD delivers product and manufacturing "
                    "information on the model; a model with no annotations is "
                    "missing PMI when the source confirms a complete PMI pass."},
    {"id": "y02-datum-anchored", "name": "Datum features identify real geometry",
     "category": "y4141", "sets": ["y4141", "prime"], "severity": "warning",
     "requirement": "Each datum designates at least one feature or body id "
                    "(a floating datum letter is not a datum feature)."},
    {"id": "y03-tolerance-anchored", "name": "Tolerances attach to toleranced features",
     "category": "y4141", "sets": ["y4141", "prime"], "severity": "warning",
     "requirement": "Each geometric tolerance attaches to at least one feature "
                    "or body id; a floating feature control frame is not a "
                    "valid annotation."},
    {"id": "y04-datum-used", "name": "No dangling datums",
     "category": "y4141", "sets": ["y4141", "prime"], "severity": "warning",
     "requirement": "Every defined datum participates in at least one datum "
                    "reference frame; a defined-but-unreferenced datum is "
                    "dangling and drives nothing."},
    {"id": "y05-frame-used", "name": "No orphan datum reference frames",
     "category": "y4141", "sets": ["y4141", "prime"], "severity": "warning",
     "requirement": "Every datum reference frame is referenced by at least "
                    "one tolerance; an unreferenced frame is dead PMI."},
    # ---- UNITS ------------------------------------------------------------
    {"id": "u01-units-declared", "name": "Document units statement present",
     "category": "units", "sets": ["y4141", "prime"], "severity": "warning",
     "requirement": "ASME Y14.41 requires a model units statement; the units "
                    "symbol must be a recognized engineering length unit."},
    {"id": "u02-units-consistent", "name": "PMI and model units agree",
     "category": "units", "sets": ["y4141", "prime"], "severity": "warning",
     "requirement": "When both PMI and model units are supplied they must "
                    "denote the same physical unit (unit errors catch e.g. a "
                    "mm model annotated in inches)."},
    # ---- CROSS-MODEL ------------------------------------------------------
    {"id": "c01-model-ref-resolution", "name": "PMI refs resolve in the model",
     "category": "cross-model", "sets": ["y4141", "prime"], "severity": "warning",
     "requirement": "Feature/body ids referenced by datums, tolerances and "
                    "notes resolve against the supplied model inventory "
                    "(dangling references into a model that does not contain "
                    "them)."},
]

RULES = {}


def _rule(rule_id):
    def deco(fn):
        RULES[rule_id] = fn
        return fn
    return deco


# --------------------------------------------------------------------------
# Rule sets (severity defaults + per-set overrides; prime is the stricter
# defense-prime requirements bundle).
# --------------------------------------------------------------------------
RULE_SETS = {
    "y4141": {
        "name": "ASME Y14.41 MBD rule set",
        "description": "Model-based definition expectations drawn from "
                       "ASME Y14.41-2019: PMI on the model, valid datum "
                       "features, anchored annotations, units statement.",
        "basis": "ASME Y14.41 Digital Product Definition Data Practices "
                 "(clean-room: public standard references only, per P0-M5-T01).",
        "rules": [r["id"] for r in RULE_CATALOG],
        "overrides": {},
    },
    "prime": {
        "name": "Defense-prime MBD requirements bundle",
        "description": "y4141 plus the acceptance posture a defense prime "
                       "applies to released MBD: floating annotations and "
                       "unit mismatches are release-blocking errors, model "
                       "references must resolve.",
        "basis": "Common prime supplier requirements on digital product "
                 "definition (no single prime standard is embedded; the "
                 "bundle is our documented interpretation for demo/QA).",
        "rules": [r["id"] for r in RULE_CATALOG],
        # Prime upgrades the Y14.41 warnings that a prime would gate on.
        "overrides": {
            "y02-datum-anchored": "error",
            "y03-tolerance-anchored": "error",
            "y04-datum-used": "error",
            "y05-frame-used": "error",
            "u01-units-declared": "error",
            "u02-units-consistent": "error",
            "c01-model-ref-resolution": "error",
        },
    },
}


def severity_for(rule_id: str, rule_set: str) -> str:
    """Effective severity of rule_id under rule_set (default or override)."""
    for r in RULE_CATALOG:
        if r["id"] == rule_id:
            return RULE_SETS[rule_set]["overrides"].get(rule_id, r["severity"])
    raise KeyError(f"unknown rule {rule_id}")


# --------------------------------------------------------------------------
# Input bundle helpers
# --------------------------------------------------------------------------
def isfinite(x):
    return isinstance(x, (int, float)) and not isinstance(x, bool) and math.isfinite(x)


def _ref_ids_of(obj, key):
    value = obj.get(key) or []
    return [i for i in value if isinstance(i, str) and i.strip()]


def _find_duplicates(ids):
    seen, dupes = set(), []
    for i in ids:
        if i in seen and i not in dupes:
            dupes.append(i)
        seen.add(i)
    return dupes


def _count_annotations(pmi_doc):
    return (len(pmi_doc.get("datums") or []) + len(pmi_doc.get("tolerances") or [])
            + len(pmi_doc.get("notes") or []))


def check_input(inp: dict, rule_set: str) -> dict:
    """Run every rule of `rule_set` against input bundle `inp`.

    Returns an InputReport dict: {"input", "id", "rule_set", "findings",
    "status", "summary"}. Exactly one finding per rule per input, deterministic
    (catalog order).
    """
    findings = []
    for rid in RULE_SETS[rule_set]["rules"]:
        fn = RULES[rid]
        try:
            res = fn(inp)
        except MbdInputError as exc:  # e.g. structurally unreadable doc
            res = {"status": "unverified", "evidence": f"could not evaluate: {exc}",
                   "detail": "rule evaluation raised a typed input error"}
        findings.append({
            "rule": rid,
            "status": res["status"],
            "severity": severity_for(rid, rule_set),
            "evidence": res.get("evidence", ""),
            "detail": res.get("detail"),
        })
    summary = _rollup(findings)
    return {
        "input": inp.get("source", {}),
        "id": inp.get("id") or inp.get("source", {}).get("file", "inline"),
        "rule_set": rule_set,
        "findings": findings,
        "summary": summary,
        "status": "checked",
    }


def _rollup(findings):
    counts = {s: 0 for s in STATUSES}
    for f in findings:
        counts[f["status"]] += 1
    counts["total"] = sum(counts.values())
    return counts


def _pmi(input_):
    doc = input_.get("pmi")
    if not isinstance(doc, dict):
        raise MbdInputError("no PMI document supplied in the input bundle")
    return doc


# ==========================================================================
# STRUCTURAL RULES (s01..s09) - parity with crates/oep-pmi/src/validate.rs
# ==========================================================================
@_rule("s01-pmi-format")
def _s01(inp):
    doc = _pmi(inp)
    if doc.get("format") != "oep-pmi":
        return {"status": "fail",
                "evidence": f"expected format 'oep-pmi', got {doc.get('format')!r}"}
    return {"status": "pass", "evidence": "format tag is 'oep-pmi'"}


@_rule("s02-pmi-version")
def _s02(inp):
    doc = _pmi(inp)
    if doc.get("version") != 1:
        return {"status": "fail",
                "evidence": f"unsupported schema version {doc.get('version')!r} (expected 1)"}
    return {"status": "pass", "evidence": "schema version is 1"}


@_rule("s03-unique-ids")
def _s03(inp):
    doc = _pmi(inp)
    ids = []
    for cls in ("datums", "frames", "tolerances", "notes"):
        ids.extend(o.get("id") for o in doc.get(cls) or [])
    dupes = _find_duplicates([i for i in ids if isinstance(i, str)])
    if dupes:
        return {"status": "fail",
                "evidence": f"duplicate id(s) across PMI classes: {', '.join(sorted(set(dupes)))}"}
    return {"status": "pass", "evidence": "all ids unique across the PMI document"}


@_rule("s04-unique-labels")
def _s04(inp):
    doc = _pmi(inp)
    # blank labels
    blank = [d.get("id") for d in (doc.get("datums") or [])
             if not str(d.get("label", "")).strip()]
    seen, dupes = {}, []
    for d in doc.get("datums") or []:
        label = str(d.get("label", "")).strip()
        if not label:
            continue
        if label in seen and label not in [x[0] for x in dupes]:
            dupes.append((label, seen[label], d.get("id")))
        seen.setdefault(label, d.get("id"))
    if blank:
        return {"status": "fail",
                "evidence": f"datum(s) with blank label: {', '.join(map(str, blank))}"}
    if dupes:
        msg = "; ".join(f"label {l!r} used by {a} and {b}" for l, a, b in dupes)
        return {"status": "fail", "evidence": msg}
    return {"status": "pass", "evidence": "datum labels unique and non-blank"}


@_rule("s05-refs-unique")
def _s05(inp):
    doc = _pmi(inp)
    # Parity with crates/oep-pmi/src/validate.rs duplicate-ref: duplicates
    # are checked WITHIN each single reference list (one datum's feature list,
    # one tolerance's body list, ...), never across objects - the same feature
    # may legally be referenced by many tolerances/notes/datums.
    dupes, offenders = [], []
    for group, key, label in (
        (doc.get("datums") or [], "feature_ids", "datum"),
        (doc.get("datums") or [], "body_ids", "datum"),
        (doc.get("tolerances") or [], "feature_ids", "tolerance"),
        (doc.get("tolerances") or [], "body_ids", "tolerance"),
        (doc.get("notes") or [], "feature_ids", "note"),
        (doc.get("notes") or [], "body_ids", "note"),
    ):
        for obj in group:
            ids = _ref_ids_of(obj, key)
            for d in _find_duplicates(ids):
                if d not in dupes:
                    dupes.append(d)
                offenders.append(f"{label} {obj.get('id')} list {key}: {d}")
    if dupes:
        return {"status": "fail",
                "evidence": "duplicate id(s) within a single reference list: "
                            + "; ".join(offenders)}
    return {"status": "pass", "evidence": "no duplicate ids in any single reference list"}


@_rule("s06-datum-frame-resolution")
def _s06(inp):
    doc = _pmi(inp)
    datum_ids = {d.get("id") for d in doc.get("datums") or []}
    problems = []
    for f in doc.get("frames") or []:
        seen_prec = set()
        for ref in f.get("refs") or []:
            d = ref.get("datum_id")
            if d not in datum_ids:
                problems.append(f"frame {f.get('id')} references unknown datum {d!r}")
            prec = ref.get("precedence")
            if not isinstance(prec, int) or prec < 1 or prec > 3:
                problems.append(f"frame {f.get('id')} ref {d!r} precedence {prec!r} outside 1..=3")
            elif prec in seen_prec:
                problems.append(f"frame {f.get('id')} has duplicate precedence {prec}")
            seen_prec.add(prec)
    if problems:
        return {"status": "fail", "evidence": "; ".join(dict.fromkeys(problems))}
    return {"status": "pass", "evidence": "all frame datum refs resolve with valid precedence"}


@_rule("s07-tolerance-frame-resolution")
def _s07(inp):
    doc = _pmi(inp)
    frame_ids = {f.get("id") for f in doc.get("frames") or []}
    bad = []
    for t in doc.get("tolerances") or []:
        fid = t.get("frame_id")
        if fid is not None and fid not in frame_ids:
            bad.append(f"tolerance {t.get('id')} references unknown frame {fid!r}")
    if bad:
        return {"status": "fail", "evidence": "; ".join(bad)}
    return {"status": "pass", "evidence": "all tolerance frame refs resolve"}


@_rule("s08-tolerance-values")
def _s08(inp):
    doc = _pmi(inp)
    bad = []
    for t in doc.get("tolerances") or []:
        for key, label in (("value", "value"), ("second_value", "second value")):
            v = t.get(key)
            if v is None:
                continue
            if not isfinite(v) or v < 0:
                bad.append(f"tolerance {t.get('id')} has non-finite or negative {label} {v!r}")
    if bad:
        return {"status": "fail", "evidence": "; ".join(bad)}
    return {"status": "pass", "evidence": "tolerance values finite and non-negative"}


@_rule("s09-note-position")
def _s09(inp):
    doc = _pmi(inp)
    bad = []
    for n in doc.get("notes") or []:
        pos = n.get("position")
        if pos is None:
            continue
        if not isinstance(pos, list) or len(pos) != 3 or not all(isfinite(x) for x in pos):
            bad.append(f"note {n.get('id')} position {pos!r} is not exactly 3 finite numbers")
    if bad:
        return {"status": "fail", "evidence": "; ".join(bad)}
    return {"status": "pass", "evidence": "note positions well-formed"}


# ==========================================================================
# Y14.41 RULES (y01..y05)
# ==========================================================================
@_rule("y01-pmi-present")
def _y01(inp):
    doc = inp.get("pmi")
    extraction = inp.get("pmi_extracted")
    if not isinstance(doc, dict):
        if extraction is True:
            return {"status": "fail",
                    "evidence": "source declares a complete PMI pass but no PMI "
                                "document was supplied: PMI is missing"}
        return {"status": "unverified",
                "evidence": "no PMI document supplied; whether the design truly "
                            "lacks PMI is unverified",
                "detail": "nothing to inspect"}
    count = _count_annotations(doc)
    if count == 0:
        if extraction is True:
            return {"status": "fail",
                    "evidence": "model carries no PMI (0 datums/tolerances/notes) "
                                "after a source-confirmed complete PMI pass: "
                                "missing PMI"}
        return {"status": "unverified",
                "evidence": "model carries no PMI in the supplied data, but the "
                            "source did not confirm a complete PMI pass; cannot "
                            "distinguish 'no PMI on part' from 'PMI not "
                            "extractable' - UNVERIFIED, not a finding of missing PMI"}
    return {"status": "pass",
            "evidence": f"model carries PMI ({count} datum/tolerance/note annotation(s))"}


@_rule("y02-datum-anchored")
def _y02(inp):
    doc = _pmi(inp)
    floating = []
    for d in doc.get("datums") or []:
        if not _ref_ids_of(d, "feature_ids") and not _ref_ids_of(d, "body_ids"):
            floating.append(d.get("id"))
    if floating:
        return {"status": "fail",
                "evidence": f"datum feature(s) with no designated feature/body: "
                            f"{', '.join(map(str, floating))}"}
    return {"status": "pass", "evidence": "every datum designates a feature or body"}


@_rule("y03-tolerance-anchored")
def _y03(inp):
    doc = _pmi(inp)
    floating = []
    for t in doc.get("tolerances") or []:
        if not _ref_ids_of(t, "feature_ids") and not _ref_ids_of(t, "body_ids"):
            floating.append(t.get("id"))
    if floating:
        return {"status": "fail",
                "evidence": f"tolerance(s) with no toleranced feature/body: "
                            f"{', '.join(map(str, floating))}"}
    return {"status": "pass", "evidence": "every tolerance attaches to a feature or body"}


@_rule("y04-datum-used")
def _y04(inp):
    doc = _pmi(inp)
    used = set()
    for f in doc.get("frames") or []:
        for ref in f.get("refs") or []:
            used.add(ref.get("datum_id"))
    dangling = [d.get("id") for d in (doc.get("datums") or []) if d.get("id") not in used]
    if dangling:
        return {"status": "fail",
                "evidence": f"dangling datum(s) defined but referenced by no frame: "
                            f"{', '.join(map(str, dangling))}"}
    return {"status": "pass", "evidence": "every datum participates in a datum reference frame"}


@_rule("y05-frame-used")
def _y05(inp):
    doc = _pmi(inp)
    used = {t.get("frame_id") for t in doc.get("tolerances") or []}
    orphan = [f.get("id") for f in (doc.get("frames") or []) if f.get("id") not in used]
    if orphan:
        return {"status": "fail",
                "evidence": f"orphan frame(s) defined but referenced by no tolerance: "
                            f"{', '.join(map(str, orphan))}"}
    return {"status": "pass", "evidence": "every datum reference frame is used by a tolerance"}


# ==========================================================================
# UNITS RULES (u01..u02)
# ==========================================================================
@_rule("u01-units-declared")
def _u01(inp):
    doc = _pmi(inp)
    units = str(doc.get("units", "")).strip()
    if not units:
        return {"status": "fail",
                "evidence": "no document units statement (units string is blank); "
                            "ASME Y14.41 requires a model units statement"}
    canon = canonical_unit(units)
    if canon is None:
        return {"status": "warn",
                "evidence": f"units symbol {units!r} is not a recognized length "
                            f"unit (known: {', '.join(KNOWN_UNITS)})"}
    return {"status": "pass", "evidence": f"document units {units!r} recognized ({canon} unit)"}


@_rule("u02-units-consistent")
def _u02(inp):
    doc = _pmi(inp)
    model_facts = inp.get("model")
    pmi_units = str(doc.get("units", "")).strip()
    if isinstance(model_facts, dict) and str(model_facts.get("units", "")).strip():
        model_units = str(model_facts["units"]).strip()
        p_canon, m_canon = canonical_unit(pmi_units), canonical_unit(model_units)
        if p_canon is None or m_canon is None:
            return {"status": "unverified",
                    "evidence": f"cannot compare units {pmi_units!r} (PMI) vs "
                                f"{model_units!r} (model): one side is not a "
                                "recognized length unit"}
        if p_canon != m_canon:
            return {"status": "fail",
                    "evidence": f"PMI units {pmi_units!r} ({p_canon}) disagree with "
                                f"model units {model_units!r} ({m_canon})"}
        return {"status": "pass",
                "evidence": f"PMI units {pmi_units!r} and model units {model_units!r} agree"}
    return {"status": "unverified",
            "evidence": "no model units supplied to compare against; unit "
                        "consistency is UNVERIFIED, not assumed"}


# ==========================================================================
# CROSS-MODEL RULE (c01)
# ==========================================================================
@_rule("c01-model-ref-resolution")
def _c01(inp):
    doc = _pmi(inp)
    model_facts = inp.get("model")
    if not isinstance(model_facts, dict):
        return {"status": "unverified",
                "evidence": "no model facts supplied; resolution of PMI refs "
                            "into the model is UNVERIFIED, not assumed"}
    features = set(model_facts.get("feature_ids") or [])
    bodies = set(model_facts.get("body_ids") or [])
    unresolved, where = [], []
    for group, label in ((doc.get("datums") or [], "datum"),
                         (doc.get("tolerances") or [], "tolerance"),
                         (doc.get("notes") or [], "note")):
        for obj in group:
            for item in _ref_ids_of(obj, "feature_ids"):
                if item not in features:
                    unresolved.append(item)
            for item in _ref_ids_of(obj, "body_ids"):
                if item not in bodies:
                    unresolved.append(item)
    if unresolved:
        uniq = sorted(set(unresolved))
        return {"status": "fail",
                "evidence": f"PMI references resolve to nothing in the supplied model: "
                            f"{', '.join(uniq)}"}
    return {"status": "pass",
            "evidence": f"all PMI refs resolve in model inventory "
                        f"({len(features)} features, {len(bodies)} bodies)"}


# ==========================================================================
# Service: loaders, runner, report
# ==========================================================================
def load_input(path: str) -> dict:
    """Load an input bundle from a JSON path.

    Accepts either a full bundle ({"schema_version", "source", "pmi", ...}) or
    a bare oep-pmi document; missing/malformed files raise MbdInputError.
    """
    try:
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
    except FileNotFoundError as exc:
        raise MbdInputError(f"input file not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise MbdInputError(f"input file is not valid JSON: {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise MbdInputError(f"input file {path} does not contain a JSON object")
    if data.get("format") == "oep-pmi":
        return {"schema_version": INPUT_SCHEMA_VERSION,
                "source": {"origin": "oep-pmi-file", "file": os.path.basename(path)},
                "pmi": data}
    if "pmi" in data:
        return data
    raise MbdInputError(f"input file {path} is neither an oep-pmi document nor a bundle")


def _fixture_bundle(entry: dict, base: str) -> dict:
    pmi_path = os.path.join(base, entry.get("file", ""))
    if not os.path.exists(pmi_path):
        raise MbdInputError(f"fixture pmi file not found: {entry.get('file')}")
    with open(pmi_path, "r", encoding="utf-8") as fh:
        try:
            pmi_doc = json.load(fh)
        except json.JSONDecodeError as exc:
            raise MbdInputError(f"fixture {entry.get('id')} pmi file not valid JSON: {exc}") from exc
    if not isinstance(pmi_doc, dict):
        raise MbdInputError(f"fixture {entry.get('id')} pmi is not a JSON object")
    inp = {
        "schema_version": INPUT_SCHEMA_VERSION,
        "id": entry.get("id"),
        "source": {
            "origin": entry.get("source", "unknown"),
            "file": entry.get("file"),
            "provenance": entry.get("provenance"),
        },
        "pmi": pmi_doc,
        "pmi_extracted": entry.get("pmi_extracted"),
    }
    model_file = entry.get("model_file")
    if model_file:
        model_path = os.path.join(base, model_file)
        if not os.path.exists(model_path):
            raise MbdInputError(f"fixture {entry.get('id')} model file not found: {model_file}")
        with open(model_path, "r", encoding="utf-8") as fh:
            try:
                facts = json.load(fh)
            except json.JSONDecodeError as exc:
                raise MbdInputError(
                    f"fixture {entry.get('id')} model file not valid JSON: {exc}") from exc
        if not isinstance(facts, dict) or facts.get("format") != MODEL_FACTS_FORMAT:
            raise MbdInputError(
                f"fixture {entry.get('id')} model file is not {MODEL_FACTS_FORMAT}")
        inp["model"] = {"units": facts.get("units", ""),
                        "feature_ids": list(facts.get("feature_ids") or []),
                        "body_ids": list(facts.get("body_ids") or [])}
        inp.setdefault("model_present", True)
    return inp


def run_manifest(manifest_path: str, only=None, rule_sets=None) -> list:
    """Check every fixture in a manifest under each rule set.

    Returns one InputReport per (fixture, rule set). A fixture that cannot be
    loaded yields an error row for each rule set (never a crash). A missing or
    malformed manifest yields an empty list.
    """
    rule_sets = list(rule_sets or ["y4141", "prime"])
    try:
        with open(manifest_path, "r", encoding="utf-8") as fh:
            manifest = json.load(fh)
    except (OSError, json.JSONDecodeError):
        return []
    base = os.path.dirname(os.path.abspath(manifest_path))
    reports = []
    for entry in manifest.get("fixtures", []):
        if only is not None and entry.get("id") not in only:
            continue
        try:
            bundle = _fixture_bundle(entry, base)
        except MbdInputError as exc:
            for sid in rule_sets:
                reports.append({
                    "input": {"file": entry.get("file")}, "id": entry.get("id"),
                    "rule_set": sid, "findings": [], "status": "error",
                    "error": str(exc),
                    "summary": {k: 0 for k in STATUSES} | {"total": 0},
                })
            continue
        for sid in rule_sets:
            reports.append(check_input(bundle, sid))
    return reports


def build_report(reports: list, meta: dict | None = None) -> dict:
    """Assemble a report dict from InputReports.

    Honest by construction: rollup totals are computed from the granular
    findings (never hand-typed); UNVERIFIED counts separately from PASS.
    """
    meta = dict(meta or {})
    ordered = sorted(reports, key=lambda r: (r.get("id", ""), r.get("rule_set", "")))
    inputs = []
    totals = {s: 0 for s in STATUSES}
    findings_total = 0
    for r in ordered:
        row = {
            "id": r.get("id"),
            "rule_set": r.get("rule_set"),
            "source": r.get("input", {}),
            "status": r.get("status"),
        }
        if r.get("error"):
            row["error"] = r["error"]
        s = r.get("summary", {})
        row["summary"] = s
        row["findings"] = r.get("findings", [])
        inputs.append(row)
        for st in STATUSES:
            totals[st] += s.get(st, 0)
        findings_total += s.get("total", 0)
    report = {
        "report_format": REPORT_FORMAT,
        "report_version": REPORT_VERSION,
        "generated": meta.get("generated"),
        "tool": dict(TOOL),
        "task": meta.get("task"),
        "rule_sets": meta.get("rule_sets", sorted({i["rule_set"] for i in inputs})),
        "honesty": {
            "scope": [
                "This report is NOT a certificate of conformance to ASME Y14.41 or "
                "to any single defense-prime requirement for a manufactured part.",
                "It certifies only what the supplied PMI/model data demonstrates. "
                "Gaps in the supplied data are reported UNVERIFIED, never silently "
                "as PASS.",
                "UNVERIFIED findings are NOT PASS findings; they must be resolved "
                "by the requester (more data or a confirmed extraction status) "
                "before this report can support acceptance.",
            ],
            "limits": [
                "Model geometry legality, feature size, and manufacturing "
                "feasibility are NOT checked by this pass.",
                "PMI semantics are judged against the supplied oep-pmi data; "
                "an import gap on the source side is surfaced as UNVERIFIED, "
                "never as a fabricated PASS or FAIL.",
            ],
        },
        "summary": {
            "inputs": len(inputs),
            "findings": findings_total,
            **totals,
        },
        "inputs": inputs,
    }
    return report


def render_markdown(report: dict) -> str:
    """Render the report as human-readable Markdown (report output surface).

    The H1 title tracks the task that produced the report (honest labelling of
    the artifact); with no task present it renders a neutral format title.
    """
    ttl = report.get("task")
    lines = [f"# {ttl} MBD Validator Report" if ttl else "# MBD Validator Report", ""]
    lines.append(f"- report_format: {report['report_format']} v{report['report_version']}")
    if report.get("generated"):
        lines.append(f"- generated: {report['generated']}")
    if report.get("task"):
        lines.append(f"- task: {report['task']}")
    s = report["summary"]
    lines.append(f"- inputs: {s['inputs']}  findings: {s['findings']}  "
                 f"PASS {s['pass']} | FAIL {s['fail']} | WARN {s['warn']} | "
                 f"UNVERIFIED {s['unverified']}")
    lines += ["", "## Honesty scope", ""]
    lines += [f"- {line}" for line in report["honesty"]["scope"]]
    lines += ["", "## Status legend", ""]
    lines += ["- **PASS** - the supplied data demonstrated the requirement satisfied.",
              "- **FAIL** - the supplied data demonstrated a violation.",
              "- **WARN** - the requirement is not met exactly but needs a ruling "
              "(e.g. unrecognized unit symbol).",
              "- **UNVERIFIED** - the rule could not run on the supplied data; "
              "this is not a PASS.", ""]
    for row in report.get("inputs", []):
        lines += [f"## {row['id']}  ({row['rule_set']})", ""]
        if row.get("status") == "error":
            lines += [f"- error: {row['error']}", ""]
            continue
        lines += ["| rule | status | severity | evidence |",
                  "|---|---|---|---|"]
        for f in row["findings"]:
            lines += [f"| {f['rule']} | {f['status'].upper()} | {f['severity']} | "
                      f"{f['evidence']} |"]
        lines += [""]
    return "\n".join(lines)


def write_report(report: dict, outdir: str) -> tuple:
    """Write the report as JSON + Markdown; returns (json_path, md_path).

    Injects the real generation timestamp at write time (build_report stays
    deterministic for tests).
    """
    os.makedirs(outdir, exist_ok=True)
    from datetime import datetime, timezone
    stamped = dict(report)
    stamped["generated"] = datetime.now(timezone.utc).isoformat()
    json_path = os.path.join(outdir, "mbd-validate-report.json")
    md_path = os.path.join(outdir, "mbd-validate-report.md")
    with open(json_path, "w", encoding="utf-8") as fh:
        json.dump(stamped, fh, indent=2, sort_keys=False)
        fh.write("\n")
    with open(md_path, "w", encoding="utf-8") as fh:
        fh.write(render_markdown(stamped))
    return json_path, md_path


# --------------------------------------------------------------------------
# Thin runner CLI (product CLI + exit codes = P4-M5-T02; this stays demoable).
# --------------------------------------------------------------------------
def main(argv=None) -> int:
    ap = argparse.ArgumentParser(prog="mbd_validate",
                                 description="P4-M5-T01 MBD validator service "
                                             "(rule engine + corpus fixture check)")
    ap.add_argument("--manifest", required=True, help="path to fixture manifest JSON")
    ap.add_argument("--out", default=".", help="output directory for the report")
    args = ap.parse_args(argv)
    reports = run_manifest(args.manifest)
    if not reports:
        print(f"no fixtures checked (manifest: {args.manifest})", file=sys.stderr)
        return 1
    report = build_report(reports, {"task": "P4-M5-T01",
                                    "rule_sets": sorted({r["rule_set"] for r in reports})})
    json_path, md_path = write_report(report, args.out)
    s = report["summary"]
    print(f"mbd-validate: {s['inputs']} input rows | PASS {s['pass']} FAIL {s['fail']} "
          f"WARN {s['warn']} UNVERIFIED {s['unverified']}")
    print(f"report: {json_path}")
    print(f"report: {md_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
