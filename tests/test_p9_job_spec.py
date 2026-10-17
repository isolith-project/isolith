#!/usr/bin/env python3
"""P9-M1-T01 acceptance test: job/task spec schema committed + validated.

Pure stdlib. Runs under `pytest -q` and directly. Checks four things:

1. The committed JSON Schema (docs/P9-M1/job-spec-v1.schema.json) uses ONLY
   the documented self-contained subset (type/properties/required/items/enum/
   const/pattern/oneOf/minimum/maximum/minLength/...) that this test can
   validate with no external deps -- an unknown keyword fails the run.
2. Both committed examples validate against the schema with a self-contained
   validator (a job that validates in Python must also deserialize+validate
   in the crate's integration test -- both peer at the same bytes).
3. Structure: a job must carry format=oep-job, version=1, executor, resources,
   and at most these top-level keys.
4. DRIFT guard: the Schema's closed-vocabulary enums and the 64-hex blob
   length come from the SAME constants the Rust crate declares in
   crates/oep-job/src/spec.rs -- schema and code cannot diverge.
"""
import json
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))

SCHEMA_PATH = os.path.join(REPO, "docs", "P9-M1", "job-spec-v1.schema.json")
EXAMPLES = [
    os.path.join(REPO, "docs", "P9-M1", "examples", "render-job.json"),
    os.path.join(REPO, "docs", "P9-M1", "examples", "convert-job.json"),
]
SPEC_RS = os.path.join(REPO, "crates", "oep-job", "src", "spec.rs")

ALLOWED_KEYWORDS = {
    "$schema", "$id", "title", "description", "$defs", "$ref",
    "type", "properties", "required", "items", "enum", "const",
    "pattern", "minimum", "maximum", "minLength", "maxLength",
    "additionalProperties", "default", "oneOf",
}


def load_json(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


class SchemaError(Exception):
    pass


class Validator:
    """Self-contained validator for the documented JSON-Schema subset."""

    def __init__(self, root):
        self.root = root

    def resolve(self, ref, where):
        if not ref.startswith("#/$defs/"):
            raise SchemaError(f"{where}: unsupported $ref {ref!r}")
        node = self.root
        for part in ref.split("/")[1:]:
            node = node[part]
        return node

    def validate(self, inst, node, where="$"):
        self._check_keywords(node, where)
        errs = []

        if "$ref" in node:
            errs += self.validate(inst, self.resolve(node["$ref"], where), where + "@" + node["$ref"])

        t = node.get("type")
        if t == "object":
            if not isinstance(inst, dict):
                return errs + [f"{where}: expected object, got {type(inst).__name__}"]
            props = node.get("properties", {})
            for req in node.get("required", []):
                if req not in inst:
                    errs.append(f"{where}: missing required property {req!r}")
            for key, val in inst.items():
                if key in props:
                    errs += self.validate(val, props[key], f"{where}.{key}")
                elif node.get("additionalProperties", True) is False:
                    errs.append(f"{where}: unexpected property {key!r}")
        elif t == "array":
            if not isinstance(inst, list):
                return errs + [f"{where}: expected array, got {type(inst).__name__}"]
            items = node.get("items")
            if isinstance(items, dict):
                for i, v in enumerate(inst):
                    errs += self.validate(v, items, f"{where}[{i}]")
        elif t == "string":
            if not isinstance(inst, str):
                errs.append(f"{where}: expected string, got {type(inst).__name__}")
            else:
                if "minLength" in node and len(inst) < node["minLength"]:
                    errs.append(f"{where}: string shorter than minLength {node['minLength']}")
                if "maxLength" in node and len(inst) > node["maxLength"]:
                    errs.append(f"{where}: string longer than maxLength {node['maxLength']}")
                if "pattern" in node and re.match(node["pattern"], inst) is None:
                    errs.append(f"{where}: string fails pattern {node['pattern']!r}")
        elif t == "integer":
            if not isinstance(inst, int) or isinstance(inst, bool):
                errs.append(f"{where}: expected integer, got {type(inst).__name__}")
            else:
                if "minimum" in node and inst < node["minimum"]:
                    errs.append(f"{where}: {inst} below minimum {node['minimum']}")
                if "maximum" in node and inst > node["maximum"]:
                    errs.append(f"{where}: {inst} above maximum {node['maximum']}")
        elif t == "boolean":
            if not isinstance(inst, bool):
                errs.append(f"{where}: expected boolean, got {type(inst).__name__}")
        elif t == "null":
            if inst is not None:
                errs.append(f"{where}: expected null")
        elif t == "number":
            if not isinstance(inst, (int, float)) or isinstance(inst, bool):
                errs.append(f"{where}: expected number, got {type(inst).__name__}")

        if "enum" in node and inst not in node["enum"]:
            errs.append(f"{where}: value {inst!r} not in enum {node['enum']}")
        if "const" in node and inst != node["const"]:
            errs.append(f"{where}: const mismatch, want {node['const']!r}")
        if "oneOf" in node:
            n = sum(1 for sub in node["oneOf"] if not self.validate(inst, sub, where))
            if n != 1:
                errs.append(f"{where}: oneOf matched {n} branches (want exactly 1)")
        return errs

    def walk_keywords(self, node, where="$"):
        """Recurse only through schema-valued keywords, so `properties` maps
        (whose keys are instance property names, not schema keywords) and leaf
        keywords like `enum`/`required` are never mistaken for keywords."""
        self._check_keywords(node, where)
        for key in ("properties", "$defs"):
            if key in node and isinstance(node[key], dict):
                for sub in node[key].values():
                    self.walk_keywords(sub, f"{where}/{key}")
        if "items" in node and isinstance(node["items"], dict):
            self.walk_keywords(node["items"], f"{where}/items")
        for key in ("oneOf", "anyOf", "allOf"):
            if key in node:
                for i, sub in enumerate(node[key]):
                    if isinstance(sub, dict):
                        self.walk_keywords(sub, f"{where}/{key}[{i}]")

    def _check_keywords(self, node, where):
        for key in node:
            if key not in ALLOWED_KEYWORDS:
                raise SchemaError(f"{where}: unsupported JSON-Schema keyword {key!r}"
                                  " (keep the schema in the documented subset)")


def rust_vocabularies():
    """Parse the closed-vocabulary + numeric constants out of the Rust source."""
    text = open(SPEC_RS, encoding="utf-8").read()
    values = dict(re.findall(r'pub const ([A-Z_]+): &str = "([^"]+)";', text))
    numbers = dict(
        (name, int(v))
        for name, v in re.findall(r"pub const ([A-Z_]+): (?:usize|u32|u64) = (\d+);", text)
    )
    arrays = dict(re.findall(r"pub const ([A-Z_]+): \[&str; \d+\] = \[([^\]]*)\];", text))
    resolved = {}
    for name, members in arrays.items():
        resolved[name] = [values[m.strip()] for m in members.split(",") if m.strip()]
    return resolved, numbers


def test_schema_stays_in_subset_and_valid_json():
    schema = load_json(SCHEMA_PATH)
    assert schema.get("$schema") == "https://json-schema.org/draft/2020-12/schema"
    v = Validator(schema)
    v.walk_keywords(schema)  # raises SchemaError on any out-of-subset keyword


def test_examples_validate_against_schema():
    schema = load_json(SCHEMA_PATH)
    v = Validator(schema)
    for path in EXAMPLES:
        doc = load_json(path)
        errors = v.validate(doc, schema)
        assert not errors, f"{os.path.basename(path)} fails schema: {errors}"


def test_job_headers_and_shapes():
    schema = load_json(SCHEMA_PATH)
    v = Validator(schema)
    for path in EXAMPLES:
        doc = load_json(path)
        assert doc["format"] == "oep-job", f"{path}: bad format header"
        assert doc["version"] == 1, f"{path}: unsupported version"
        for req in ("executor", "resources"):
            assert req in doc, f"{path}: missing {req}"
        for inp in doc.get("input", []):
            assert "name" in inp and "ref" in inp, f"{path}: input lacks name/ref"
        extra = set(doc) - {
            "format", "version", "job_id", "name", "description", "created_at",
            "input", "executor", "resources", "output",
        }
        assert not extra, f"{path}: unknown top-level keys {extra}"


def test_invalid_ref_is_rejected_by_schema():
    schema = load_json(SCHEMA_PATH)
    v = Validator(schema)
    doc = load_json(EXAMPLES[0])
    # Corrupt one input blob address and confirm the schema catches it, i.e.
    # the schema really constrains refs (not a pass-through).
    for inp in doc["input"]:
        if inp["ref"].get("kind") == "file":
            inp["ref"]["blob_sha256"] = "short"
    errors = v.validate(doc, schema)
    assert any("pattern" in e for e in errors), f"schema missed bad blob address: {errors}"


def test_drift_schema_enums_match_rust_consts():
    schema = load_json(SCHEMA_PATH)
    resolved, _ = rust_vocabularies()
    checks = {
        "EXECUTOR_KINDS": schema["$defs"]["executorRef"]["properties"]["kind"]["enum"],
        "REF_KINDS": schema["$defs"]["vaultObjectRef"]["properties"]["kind"]["enum"],
        "TARGET_KINDS": schema["$defs"]["outputTarget"]["properties"]["kind"]["enum"],
        "ITEM_KINDS": schema["$defs"]["outputTarget"]["properties"]["item_kind"]["enum"],
    }
    for rust_name, schema_enum in checks.items():
        assert rust_name in resolved, f"{rust_name} not parsed from {SPEC_RS}"
        assert schema_enum == resolved[rust_name], (
            f"drift: schema {schema_enum} != Rust {rust_name} {resolved[rust_name]}\n"
            "Keep the JSON Schema enums in lockstep with spec.rs."
        )


def test_drift_schema_blob_length_matches_rust():
    schema = load_json(SCHEMA_PATH)
    resolved, values = rust_vocabularies()
    pattern = schema["$defs"]["vaultObjectRef"]["properties"]["blob_sha256"]["pattern"]
    m = re.fullmatch(r"\^\[0-9a-f\]\{(\d+)\}\$", pattern)
    assert m, f"blob pattern not in expected shape: {pattern!r}"
    assert int(m.group(1)) == int(values["BLOB_HEX_LEN"]), (
        "drift: schema blob pattern length != BLOB_HEX_LEN in spec.rs"
    )


def main():
    failures = 0
    tests = [
        test_schema_stays_in_subset_and_valid_json,
        test_examples_validate_against_schema,
        test_job_headers_and_shapes,
        test_invalid_ref_is_rejected_by_schema,
        test_drift_schema_enums_match_rust_consts,
        test_drift_schema_blob_length_matches_rust,
    ]
    print("P9-M1-T01 job spec schema acceptance")
    print(f"schema: {SCHEMA_PATH}")
    print("=" * 60)
    for t in tests:
        try:
            t()
            print(f"  [PASS] {t.__name__}")
        except Exception as e:  # noqa: BLE001 - report and continue
            print(f"  [FAIL] {t.__name__} -- {e}")
            failures += 1
    print("=" * 60)
    if failures:
        print(f"FAILED {failures} check(s)")
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
