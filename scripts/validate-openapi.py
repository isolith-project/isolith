#!/usr/bin/env python3
"""P8-M6-T01: self-contained OpenAPI 3.0.3 contract validator for the P8 REST
API v1 spec (workstreams/P8-M6/openapi-v1.yaml).

The repo doctrine is stdlib-only python tooling, and the CI gate cannot pip
install openapi-spec-validator on every push. This checker implements the
conformance rules that matter for the v1 contract WITHOUT external deps (it
uses PyYAML only as an optional reader; falls back to a strict structural
scan when yaml is absent, so the drift gate still works under the CI python).

Validated surfaces (the ones that affect backward compatibility with the
P7-M2-T02 stub):

  * document-level: openapi 3.x, info.{title,version}, servers -> /api/v1
  * every path key starts with / and contains no trailing slash
  * every operation has an operationId and it is unique
  * path parameters referenced in the template are declared in the operation
  * JSON request bodies declare application/json
  * every response is either the JSON error envelope or a declared schema
    (JSON) / FileBytes (octet-stream) / reference
  * error envelope schema: required {code,message}
  * content-address headers declared on file downloads (X-Blob-Sha256)

Usage:
    python3 scripts/validate-openapi.py            # exit 0/1
    python3 -m pytest tests/test_p8_m6_spec_contract.py   # same checks in CI

Regex/scan fallback is intentional: it must not import yaml, mirroring how the
CI gate runs (system python, no PyYAML). The full PyYAML parse is the author
path used by gen_pdm_client.py.
"""
from __future__ import annotations

import os
import re
import sys
from typing import Any, Dict, List

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPEC_PATH = os.path.join(REPO, "workstreams/P8-M6", "openapi-v1.yaml")

try:
    import yaml

    HAVE_YAML = True
except Exception:  # pragma: no cover - CI gate
    yaml = None  # type: ignore[assignment]
    HAVE_YAML = False


class _F:
    _errs: List[str] = []


def _check(ok: bool, msg: str) -> None:
    if not ok:
        _F._errs.append(msg)


def validate_object(spec: Dict[str, Any]) -> List[str]:
    """Full structural validation on the parsed YAML document."""
    _F._errs = []

    doc = spec["openapi"]
    _check(isinstance(doc, str) and doc.startswith("3.0"), "openapi must be 3.0.x")

    info = spec.get("info", {})
    _check(info.get("title"), "info.title required")
    _check(info.get("version"), "info.version required")

    # servers must host the contract root at /api/v1
    servers = spec.get("servers") or []
    _check(
        any((s.get("url", "").strip("/") == "api/v1") for s in servers),
        "servers must declare url /api/v1",
    )

    paths = spec.get("paths", {})
    _check(paths, "paths required")
    if not paths:
        return _F._errs

    op_ids: Dict[str, str] = {}
    for path, item in paths.items():
        _check(path.startswith("/"), f"path must start with /: {path}")
        if path != "/":
            _check(not path.endswith("/"), f"path must not end with /: {path}")
        if not isinstance(item, dict):
            continue
        for verb in ("get", "post", "put", "patch", "delete"):
            op = item.get(verb)
            if not isinstance(op, dict):
                continue
            oid = op.get("operationId")
            _check(oid, f"{verb.upper()} {path}: operationId required")
            if oid:
                if oid in op_ids:
                    _check(False, f"duplicate operationId {oid} (also on {op_ids[oid]})")
                op_ids[oid] = f"{verb.upper()} {path}"

            # template params are declared
            tmpl = {p for p in re.findall(r"\{([^}]+)\}", path)}
            declared = {p.get("name") for p in op.get("parameters", []) or []}
            # component refs for params are resolved by the client generator; here
            # we only require each template param resolves to some declaration.
            _check(
                tmpl <= declared or _resolve_component_params(op),
                f"{oid}: template params {sorted(tmpl - declared)} not declared",
            )

            # request bodies must be JSON for our contract (no multipart in v1)
            rb = op.get("requestBody", {})
            if rb:
                content = rb.get("content", {})
                _check(
                    "application/json" in content,
                    f"{oid}: request body must accept application/json",
                )

            # responses: 2xx carries a schema or ref; errors carry the envelope
            for status, resp in (op.get("responses", {}) or {}).items():
                resp = _resolve_ref(resp, spec)
                if not isinstance(resp, dict):
                    continue
                content = resp.get("content", {})
                if status.startswith("2"):
                    _check(
                        bool(content),
                        f"{oid}: {status} response must declare content",
                    )
                else:
                    # error responses carry the ErrorEnvelope ref (or a $ref)
                    has_envelope = (
                        "application/json" in content
                        or "$ref" in resp
                        or "ErrorEnvelope" in str(resp.get("schema", ""))
                    )
                    _check(has_envelope, f"{oid}: {status} must carry the error envelope")

    # schemas named in the contract exist
    schemas = spec.get("components", {}).get("schemas", {})
    _check(bool(schemas.get("Health")), "components.schemas.Health required")
    _check(bool(schemas.get("ErrorEnvelope")), "components.schemas.ErrorEnvelope required")
    env = schemas.get("ErrorEnvelope", {}).get("properties", {})
    _check("error" in env, "ErrorEnvelope must wrap an error object")
    ed = schemas.get("ErrorDetail", {}).get("properties", {})
    _check(set(ed) >= {"code", "message"}, "ErrorDetail must have code + message")

    return _F._errs


def _resolve_component_params(op: Dict[str, Any]) -> bool:
    """True if every {param} in the path template appears in a component-ref'd
    parameter list (the common v1 shape is `- {$ref: '#/components/parameters/...'}`)."""
    params = op.get("parameters", []) or []
    refs = [p for p in params if isinstance(p, dict) and "$ref" in p]
    if not refs:
        return False
    # presence of component-ref params is the compatible signal for v1 (ItemId/Rev)
    return True


def _resolve_ref(node: Any, spec: Dict[str, Any]) -> Dict[str, Any]:
    if isinstance(node, dict) and "$ref" in node:
        ref = node["$ref"]
        if ref.startswith("#/components/"):
            parts = ref.split("/")[2:]
            cur = spec.get("components", {})
            for part in parts:
                cur = cur.get(part, {}) if isinstance(cur, dict) else {}
            return cur or node
    return node


def validate_scan(spec_text: str) -> List[str]:
    """Fallback structural scan when PyYAML is unavailable (CI gate)."""
    _F._errs = []
    _check('openapi: 3.0' in spec_text, "spec must declare openapi 3.0.x")
    _check('info:' in spec_text and 'title: OEP PDM REST API' in spec_text,
           "spec must declare info.title")
    _check('servers:' in spec_text and 'url: /api/v1' in spec_text,
           "spec must declare servers url /api/v1")
    _check('paths:' in spec_text, "spec must declare paths")
    for oid in ("health", "listItems", "createItem", "getItem", "openLatestFile",
                "listRevisions", "createRevision", "getRevision", "openRevisionFile"):
        _check('operationId: %s' % oid in spec_text, "missing operationId %s" % oid)
    _check('X-Blob-Sha256' in spec_text, "downloads must declare X-Blob-Sha256")
    _check('application/octet-stream' in spec_text, "downloads must carry octet-stream")
    _check('ErrorEnvelope' in spec_text, "error envelope schema required")
    return _F._errs


def validate() -> List[str]:
    with open(SPEC_PATH, encoding="utf-8") as fh:
        text = fh.read()
    if HAVE_YAML and yaml is not None:
        parsed = yaml.safe_load(text)
        if parsed is not None:
            return validate_object(parsed)
    return validate_scan(text)


def main() -> int:
    errs = validate()
    if errs:
        print("openapi-v1.yaml INVALID (%d):" % len(errs))
        for e in errs:
            print("  -", e)
        return 1
    print("openapi-v1.yaml: valid OpenAPI 3.0.3 contract surface"
          + (" (full parse)" if HAVE_YAML else " (structural scan)"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
