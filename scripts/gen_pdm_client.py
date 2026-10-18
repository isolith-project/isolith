#!/usr/bin/env python3
"""P8-M6-T01 codegen: generate the OEP PDM Python client from the OpenAPI spec.

Reads `P8-M6/openapi-v1.yaml` (the P8 REST API v1 contract) and emits
`python/oep_pdm_client.py`: a stdlib-only (urllib) typed client whose models
and methods are derived field-for-field from the spec. This is what makes the
OpenAPI spec THE contract for P8-M6-T01: the client is generated from it, and
the drift test (`tests/test_p8_m6_spec_contract.py`) fails if the committed
client no longer matches the committed spec bytes.

Why a committed generator instead of a third-party one (openapi-generator /
swagger-codegen): the repo doctrine is stdlib-only Python tooling (see
pyproject.toml, `never install deps without the lockfile`), and third-party
generators emit a dependency-laden (urllib3/certifi) client. A compact
generator that reads the small, hand-curated v1 contract is deterministic,
dependency-free, and satisfies "python client generated" with the spec as the
single source of truth.

Usage:
    python3 scripts/gen_pdm_client.py              # regenerate in place
    python3 scripts/gen_pdm_client.py --check      # exit 1 if output would change

Requires: PyYAML (dev/author machine only; the generated client is pure stdlib).
"""
from __future__ import annotations

import argparse
import hashlib
import io
import os
import sys
from typing import Any, Dict, List, Optional

try:
    import yaml
except ImportError:  # pragma: no cover - author-machine only
    yaml = None

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPEC_PATH = os.path.join(REPO, "P8-M6", "openapi-v1.yaml")
OUT_PATH = os.path.join(REPO, "python", "oep_pdm_client.py")

# Python-level method name for each operationId the contract exposes.
OPERATION_METHODS: Dict[str, str] = {
    "health": "health",
    "listItems": "list_items",
    "createItem": "create_item",
    "getItem": "get_item",
    "openLatestFile": "open_latest_file",
    "listRevisions": "list_revisions",
    "createRevision": "create_revision",
    "getRevision": "get_revision",
    "openRevisionFile": "open_revision_file",
    # P8-M6-T03 additive gate surface (lock / transition / audit).
    "acquireLock": "acquire_lock",
    "releaseLock": "release_lock",
    "transitionRevision": "transition_revision",
    "listAudit": "list_audit",
    "verifyAudit": "verify_audit",
}

# Schemas whose wire shape the client models as dataclasses, in emit order.
# The remaining schemas are request payloads (CreateItemRequest,
# CreateRevisionRequest, FileUpload) passed through as plain dicts by the
# generated method bodies, mirroring the Rust client's request types.
MODEL_SCHEMAS: List[str] = [
    "Health",
    "FileRef",
    "Revision",
    "Item",
    "ItemSummary",
    "ItemList",
    "RevisionList",
]

SCHEMA_PY: Dict[str, str] = {
    "string": "str",
    "integer": "int",
    "boolean": "bool",
    "number": "float",
}

_ESCAPE = str.maketrans({"\\": "\\\\", '"': '\\"'})


def py_type(schema: Dict[str, Any]) -> str:
    """Map an OpenAPI schema fragment to a python type annotation string."""
    if "$ref" in schema:
        return schema["$ref"].rsplit("/", 1)[-1]
    if schema.get("type") == "array":
        item = schema.get("items", {})
        return "List[%s]" % py_type(item)
    t = schema.get("type", "string")
    return SCHEMA_PY.get(t, "str")


def emit_models(w: io.StringIO, schemas: Dict[str, Any]) -> None:
    for name in MODEL_SCHEMAS:
        s = schemas.get(name, {})
        required = set(s.get("required", []))
        w.write("\n@dataclass\nclass %s:\n" % name)
        props = s.get("properties", {})
        # Dataclass rule: fields without defaults must precede fields with
        # defaults. Emit required fields first (spec order), then optional.
        ordered = [p for p in props if p in required]
        ordered += [p for p in props if p not in required]
        for pname in ordered:
            ps = props[pname]
            ann = py_type(ps)
            if pname not in required and not ps.get("nullable"):
                ann = "Optional[%s]" % ann
            default = None
            if "default" in ps:
                d = ps["default"]
                default = '"%s"' % d.translate(_ESCAPE) if isinstance(d, str) else repr(d)
            elif pname not in required:
                default = "None"
            w.write("    %s: %s" % (pname, ann))
            if default is not None:
                w.write(" = %s" % default)
            w.write("\n")
            if ps.get("description"):
                w.write("    # %s\n" % ps["description"])
        if not props:
            w.write("    pass\n")


def _find_operation(paths, op_id):
    for path_tmpl, item in paths.items():
        if not isinstance(item, dict):
            continue
        for verb, op in item.items():
            if isinstance(op, dict) and op.get("operationId") == op_id:
                return verb, path_tmpl, op
    return None


def _resolve_ref(node, spec_components):
    """Resolve a $ref dict (components.schemas/parameters/responses) to its target."""
    if not isinstance(node, dict) or "$ref" not in node:
        return node
    ref = node["$ref"]
    if not ref.startswith("#/components/"):
        return node
    parts = ref.split("/")[2:]
    cur = spec_components
    for part in parts:
        if isinstance(cur, dict):
            cur = cur.get(part, {})
    return cur


def _params(op: Dict[str, Any], spec_components) -> List[Dict[str, Any]]:
    """Parameter descriptors (name + in), resolving component parameter refs."""
    out = []
    for p in op.get("parameters", []) or []:
        p = _resolve_ref(p, spec_components)
        if isinstance(p, dict) and p.get("name") and p.get("in") in ("path", "query"):
            out.append(p)
    return out


def _path_source(template: str, path_params: List[str]) -> str:
    """Emit source that builds the request path from the template."""
    # Constant segment(s) with no path params => plain literal.
    if not path_params:
        return 'path = "%s"' % template

    # Hand-assemble: constant segments stay literal, {param} -> quoted arg.
    src = 'path = ""'
    acc_const = ""
    for part in template.split("/"):
        if not part:
            continue
        if part.startswith("{") and part.endswith("}"):
            p = part[1:-1]
            if acc_const:
                src += ' + "/%s"' % acc_const
                acc_const = ""
            src += ' + "/" + _up.quote(str(%s))' % p
        else:
            acc_const = acc_const + "/" + part if acc_const else part
    if acc_const:
        src += ' + "/%s"' % acc_const
    return src


def _payload_required(op, has_path_params) -> bool:
    return op.get("requestBody", {}).get("required", False)


def emit_client(
    w: io.StringIO,
    paths: Dict[str, Any],
    operation_methods: Dict[str, str],
    spec_components: Dict[str, Any],
) -> None:
    w.write("\n\nclass PdmError(Exception):\n")
    w.write('    """A non-2xx response carrying the contract error envelope."""\n')
    w.write("    def __init__(self, status, code, message, method=\"\", path=\"\"):\n")
    w.write("        self.status = int(status)\n")
    w.write("        self.code = code\n")
    w.write("        self.message = message\n")
    w.write("        self.method = method\n")
    w.write("        self.path = path\n")
    w.write('        super().__init__(f"PDM HTTP {status} ({code}): {message}")\n\n')
    w.write("    @property\n    def is_not_found(self):\n")
    w.write("        return self.status == 404\n")

    w.write('\n@dataclass\nclass FilePayload:\n')
    w.write('    """Downloaded file bytes plus the contract\'s metadata headers."""\n')
    for fld, doc in (
        ("item_id", "Item the file belongs to."),
        ("rev", "Revision number served from."),
        ("filename", "X-Oep-Filename header."),
        ("format", "X-Oep-Format header."),
        ("content_type", "Media type."),
        ("blob_sha256", "X-Blob-Sha256 content address."),
        ("bytes", "Raw file bytes."),
    ):
        w.write("    %s: str = \"\"\n" % fld if fld != "bytes" else "    bytes: bytes = b\"\"\n")
    w.write("\n")

    w.write("class PdmClient:\n")
    w.write('    """Generated typed client for the P8 REST API v1 contract."""\n\n')
    w.write(
        '    def __init__(self, base_url="http://127.0.0.1:8787/api/v1", timeout=20.0):\n'
        "        # Accept either the host root or the anchored /api/v1 contract root.\n"
        '        stripped = base_url.rstrip("/")\n'
        '        if stripped.endswith("/api/v1"):\n'
        '            self._root = stripped[: -len("/api/v1")]\n'
        "        else:\n"
        "            self._root = stripped\n"
        '        if not self._root.startswith("http://"):\n'
        '            raise ValueError("only http:// base URLs are supported")\n'
        "        self._prefix = \"/api/v1\"\n"
        "        self._timeout = timeout\n"
        "\n"
        "    def _url(self, path):\n"
        '        return self._root + self._prefix + path\n'
        "\n"
        "    def _request(self, method, path, body=None):\n"
        "        import json as _json\n"
        "        import urllib.error as _uerror\n"
        "        import urllib.request as _ureq\n"
        "        data = None\n"
        '        headers = {"Accept": "application/json, application/octet-stream"}\n'
        "        if body is not None:\n"
        "            data = _json.dumps(body).encode()\n"
        '            headers["Content-Type"] = "application/json"\n'
        "        req = _ureq.Request(self._url(path), data=data, headers=headers, method=method)\n"
        "        try:\n"
        "            with _ureq.urlopen(req, timeout=self._timeout) as resp:\n"
        "                headers = {k.lower(): v for k, v in resp.headers.items()}\n"
        "                return resp.status, headers, resp.read()\n"
        "        except _uerror.HTTPError as e:\n"
        "            raw = e.read()\n"
        "            try:\n"
        "                env = _json.loads(raw or b\"\")\n"
        '                code = env.get("error", {}).get("code", "unknown")\n'
        '                message = env.get("error", {}).get("message", "")\n'
        "            except Exception:  # noqa: BLE001 - non-JSON error body\n"
        '                code, message = "unknown", raw.decode(errors="replace")[:200]\n'
        "            raise PdmError(e.code, code, message, method, path)\n"
        "\n"
        "    def _json(self, method, path, body=None):\n"
        "        import json as _json\n"
        "        status, _headers, raw = self._request(method, path, body)\n"
        "        return status, _json.loads(raw) if raw else None\n"
        "\n"
    )

    emitted = []
    for op_id, py_name in operation_methods.items():
        op_info = _find_operation(paths, op_id)
        if op_info is None:
            continue
        verb, template, op = op_info
        params = _params(op, spec_components)
        path_params = [p["name"] for p in params if p["in"] == "path"]
        query_params = [p["name"] for p in params if p["in"] == "query"]
        is_file = _is_file_response(op, spec_components)
        has_body = "requestBody" in op

        w.write("    def %s(self" % py_name)
        for p in path_params:
            w.write(", %s" % p)
        for p in query_params:
            w.write(", %s=None" % p)
        if has_body and py_name in (
            "create_item",
            "create_revision",
            "acquire_lock",
            "release_lock",
            "transition_revision",
        ):
            w.write(", body")
        w.write("):\n")
        w.write('        # %s\n' % (op.get("summary") or op_id))
        w.write("        import urllib.parse as _up\n")
        # Path source; ensure `path = ...` returns the final value.
        if query_params:
            # Build base path, then append query string.
            w.write("        ")
            w.write(_path_source(template, path_params))
            w.write("\n")
            w.write("        qs = []\n")
            for qp in query_params:
                w.write('        if %s is not None: qs.append(("%s", str(%s)))\n' % (qp, qp, qp))
            w.write('        if qs: path = path + "?" + _up.urlencode(qs)\n')
        else:
            w.write("        ")
            w.write(_path_source(template, path_params))
            w.write("\n")

        if is_file:
            w.write('        status, headers, raw = self._request("GET", path)\n')
            w.write("        return FilePayload(\n")
            w.write('            item_id=headers.get("x-oep-item", ""),\n')
            w.write('            rev=headers.get("x-oep-rev", ""),\n')
            w.write('            filename=headers.get("x-oep-filename", ""),\n')
            w.write('            format=headers.get("x-oep-format", ""),\n')
            w.write('            content_type=headers.get("content-type", "application/octet-stream"),\n')
            w.write('            blob_sha256=headers.get("x-blob-sha256", ""),\n')
            w.write("            bytes=raw,\n")
            w.write("        )\n")
        else:
            verb_upper = verb.upper()
            if has_body and py_name in (
                "create_item",
                "create_revision",
                "acquire_lock",
                "release_lock",
                "transition_revision",
            ):
                w.write('        return self._json("%s", path, body)\n' % verb_upper)
            else:
                w.write('        return self._json("%s", path)\n' % verb_upper)
        w.write("\n")
        emitted.append(py_name)

    # High-level convenience on the contract surface (mirrors the Rust client).
    w.write(
        "    def save_revision(self, item_id, filename, format, content, *,\n"
        '                        state="working", comment=None, created_by=None,\n'
        '                        content_type="application/octet-stream"):\n'
        '        """Save-revision-to-PDM: upload raw bytes as the next revision."""\n'
        "        import base64 as _b64\n"
        "        body = {\n"
        '            "state": state,\n'
        '            "comment": comment,\n'
        '            "created_by": created_by,\n'
        '            "file": {\n'
        '                "filename": filename,\n'
        '                "format": format,\n'
        '                "content_type": content_type,\n'
        '                "content": _b64.b64encode(content).decode(),\n'
        "            },\n"
        "        }\n"
        "        return self.create_revision(item_id, body)\n"
        "\n"
        "    def verify_blob(self, payload):\n"
        '        """True if the payload bytes hash to their content address."""\n'
        "        import hashlib as _h\n"
        "        return _h.sha256(payload.bytes).hexdigest() == payload.blob_sha256\n"
    )


def _is_file_response(op, spec_components):
    for _status, resp in (op.get("responses", {}) or {}).items():
        resp = _resolve_ref(resp, spec_components)
        content = (resp or {}).get("content", {}) or {}
        if "application/octet-stream" in content:
            return True
    return False


def generate(spec: Dict[str, Any]) -> str:
    sha = hashlib.sha256(open(SPEC_PATH, "rb").read()).hexdigest()
    w = io.StringIO()
    w.write(
        '"""GENERATED FILE. Do not edit by hand.\n'
        "Auto-generated from P8-M6/openapi-v1.yaml by scripts/gen_pdm_client.py.\n"
        "The OpenAPI spec at P8-M6/openapi-v1.yaml is THE contract for the P8\n"
        "REST API v1. The client is a derived artifact: regenerate it whenever\n"
        "the spec changes (python3 scripts/gen_pdm_client.py). A mismatch with\n"
        "the spec is caught by tests/test_p8_m6_spec_contract.py.\n"
        '"""\n' 
    )
    w.write(
        "\nfrom dataclasses import dataclass, field\n"
        "from typing import List, Optional\n"
        "\n"
        '# The contract root served by every oep-pdm binary.\n'
        'API_BASE = "/api/v1"\n'
        'API_VERSION = "%s"\n'
        '# SHA-256 of the P8-M6/openapi-v1.yaml bytes this client was generated from.\n'
        'SPEC_SHA256 = "%s"\n' % (spec["info"]["version"], sha)
    )
    emit_models(w, spec["components"]["schemas"])
    emit_client(w, spec["paths"], OPERATION_METHODS, spec.get("components", {}))
    return w.getvalue()


def spec_sha256() -> str:
    return hashlib.sha256(open(SPEC_PATH, "rb").read()).hexdigest()


def main() -> int:
    if yaml is None:  # pragma: no cover
        print("error: PyYAML required to run the generator (author machine)")
        return 2
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true",
                        help="exit 1 if the committed client would change (drift gate)")
    args = parser.parse_args()
    with open(SPEC_PATH, "r", encoding="utf-8") as fh:
        spec = yaml.safe_load(fh)
    rendered = generate(spec)
    if args.check:
        with open(OUT_PATH, "r", encoding="utf-8") as fh:
            current = fh.read()
        if current != rendered:
            print("DRIFT: python/oep_pdm_client.py does not match P8-M6/openapi-v1.yaml; "
                  "run python3 scripts/gen_pdm_client.py")
            return 1
        print("OK: generated client matches the OpenAPI contract spec")
        return 0
    with open(OUT_PATH, "w", encoding="utf-8") as fh:
        fh.write(rendered)
    print("wrote %s (spec sha256 %s)" % (OUT_PATH, spec_sha256()))
    return 0


if __name__ == "__main__":
    sys.exit(main())
