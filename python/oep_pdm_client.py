"""GENERATED FILE. Do not edit by hand.
Auto-generated from P8-M6/openapi-v1.yaml by scripts/gen_pdm_client.py.
The OpenAPI spec at P8-M6/openapi-v1.yaml is THE contract for the P8
REST API v1. The client is a derived artifact: regenerate it whenever
the spec changes (python3 scripts/gen_pdm_client.py). A mismatch with
the spec is caught by tests/test_p8_m6_spec_contract.py.
"""

from dataclasses import dataclass, field
from typing import List, Optional

# The contract root served by every oep-pdm binary.
API_BASE = "/api/v1"
API_VERSION = "1.0.0"
# SHA-256 of the P8-M6/openapi-v1.yaml bytes this client was generated from.
SPEC_SHA256 = "cd26418539ddfd717ab88ae5f74955b454104246d8e1c9d4c9fad5ea1597e4fb"

@dataclass
class Health:
    status: str
    service: str
    version: str

@dataclass
class FileRef:
    id: str
    filename: str
    format: str
    content_type: str
    size: int
    blob_sha256: str
    # hex lowercase sha-256 of the file bytes.

@dataclass
class Revision:
    number: str
    state: str
    created_at: str
    created_by: str
    files: List[FileRef]
    maturity: str = None
    comment: str = None

@dataclass
class Item:
    id: str
    name: str
    kind: str
    created_at: str
    latest_revision: str
    revision_count: int
    description: str = None
    owner: str = None
    context: str = None

@dataclass
class ItemSummary:
    id: str
    name: str
    kind: str
    latest_revision: str
    revision_count: int

@dataclass
class ItemList:
    items: List[ItemSummary]
    total: int

@dataclass
class RevisionList:
    item_id: str
    latest: str
    revisions: List[Revision]


class PdmError(Exception):
    """A non-2xx response carrying the contract error envelope."""
    def __init__(self, status, code, message, method="", path=""):
        self.status = int(status)
        self.code = code
        self.message = message
        self.method = method
        self.path = path
        super().__init__(f"PDM HTTP {status} ({code}): {message}")

    @property
    def is_not_found(self):
        return self.status == 404

@dataclass
class FilePayload:
    """Downloaded file bytes plus the contract's metadata headers."""
    item_id: str = ""
    rev: str = ""
    filename: str = ""
    format: str = ""
    content_type: str = ""
    blob_sha256: str = ""
    bytes: bytes = b""

class PdmClient:
    """Generated typed client for the P8 REST API v1 contract."""

    def __init__(self, base_url="http://127.0.0.1:8787/api/v1", timeout=20.0):
        # Accept either the host root or the anchored /api/v1 contract root.
        stripped = base_url.rstrip("/")
        if stripped.endswith("/api/v1"):
            self._root = stripped[: -len("/api/v1")]
        else:
            self._root = stripped
        if not self._root.startswith("http://"):
            raise ValueError("only http:// base URLs are supported")
        self._prefix = "/api/v1"
        self._timeout = timeout

    def _url(self, path):
        return self._root + self._prefix + path

    def _request(self, method, path, body=None):
        import json as _json
        import urllib.error as _uerror
        import urllib.request as _ureq
        data = None
        headers = {"Accept": "application/json, application/octet-stream"}
        if body is not None:
            data = _json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        req = _ureq.Request(self._url(path), data=data, headers=headers, method=method)
        try:
            with _ureq.urlopen(req, timeout=self._timeout) as resp:
                headers = {k.lower(): v for k, v in resp.headers.items()}
                return resp.status, headers, resp.read()
        except _uerror.HTTPError as e:
            raw = e.read()
            try:
                env = _json.loads(raw or b"")
                code = env.get("error", {}).get("code", "unknown")
                message = env.get("error", {}).get("message", "")
            except Exception:  # noqa: BLE001 - non-JSON error body
                code, message = "unknown", raw.decode(errors="replace")[:200]
            raise PdmError(e.code, code, message, method, path)

    def _json(self, method, path, body=None):
        import json as _json
        status, _headers, raw = self._request(method, path, body)
        return status, _json.loads(raw) if raw else None

    def health(self):
        # Service liveness + contract identity.
        import urllib.parse as _up
        path = "/health"
        return self._json("GET", path)

    def list_items(self, prefix=None, limit=None, offset=None):
        # List items (browser catalogue). Newest first.
        import urllib.parse as _up
        path = "/items"
        qs = []
        if prefix is not None: qs.append(("prefix", str(prefix)))
        if limit is not None: qs.append(("limit", str(limit)))
        if offset is not None: qs.append(("offset", str(offset)))
        if qs: path = path + "?" + _up.urlencode(qs)
        return self._json("GET", path)

    def create_item(self, body):
        # Create a vault item.
        import urllib.parse as _up
        path = "/items"
        return self._json("POST", path, body)

    def get_item(self, item_id):
        # Get one item with its current latest revision number.
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id))
        return self._json("GET", path)

    def open_latest_file(self, item_id):
        # Open-from-PDM: download the latest revision's file bytes in one call. The browser uses this after the user picks an item.
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/file/latest"
        status, headers, raw = self._request("GET", path)
        return FilePayload(
            item_id=headers.get("x-oep-item", ""),
            rev=headers.get("x-oep-rev", ""),
            filename=headers.get("x-oep-filename", ""),
            format=headers.get("x-oep-format", ""),
            content_type=headers.get("content-type", "application/octet-stream"),
            blob_sha256=headers.get("x-blob-sha256", ""),
            bytes=raw,
        )

    def list_revisions(self, item_id):
        # List all revisions of an item, newest first.
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/revisions"
        return self._json("GET", path)

    def create_revision(self, item_id, body):
        # Save-revision-to-PDM: create the next revision of an item and upload its file (content-addressed). The stub assigns the next integer revision number; the real service applies the P8-M2-T01 state machine.
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/revisions"
        return self._json("POST", path, body)

    def get_revision(self, item_id, rev):
        # Get revision metadata (no file bytes).
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/revisions" + "/" + _up.quote(str(rev))
        return self._json("GET", path)

    def open_revision_file(self, item_id, rev):
        # Download a specific revision's file bytes.
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/revisions" + "/" + _up.quote(str(rev)) + "/file"
        status, headers, raw = self._request("GET", path)
        return FilePayload(
            item_id=headers.get("x-oep-item", ""),
            rev=headers.get("x-oep-rev", ""),
            filename=headers.get("x-oep-filename", ""),
            format=headers.get("x-oep-format", ""),
            content_type=headers.get("content-type", "application/octet-stream"),
            blob_sha256=headers.get("x-blob-sha256", ""),
            bytes=raw,
        )

    def acquire_lock(self, item_id, body):
        # Acquire the working-set lock on an item (P8-M1-T03).
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/lock"
        return self._json("POST", path, body)

    def release_lock(self, item_id, body):
        # Release the working-set lock by its token.
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/lock"
        return self._json("DELETE", path, body)

    def transition_revision(self, item_id, rev, body):
        # Apply a lock-gated lifecycle transition (P8-M2-T01 state machine). Releasing requires the live lock token.
        import urllib.parse as _up
        path = "" + "/items" + "/" + _up.quote(str(item_id)) + "/revisions" + "/" + _up.quote(str(rev)) + "/transition"
        return self._json("POST", path, body)

    def list_audit(self):
        # View the append-only, tamper-evident audit log (P8-M3-T03).
        import urllib.parse as _up
        path = "/audit"
        return self._json("GET", path)

    def verify_audit(self):
        # Re-derive the tamper-evident audit chain from disk and confirm it is intact (head checkpoint matches the log).
        import urllib.parse as _up
        path = "/audit/verify"
        return self._json("GET", path)

    def save_revision(self, item_id, filename, format, content, *,
                        state="working", comment=None, created_by=None,
                        content_type="application/octet-stream"):
        """Save-revision-to-PDM: upload raw bytes as the next revision."""
        import base64 as _b64
        body = {
            "state": state,
            "comment": comment,
            "created_by": created_by,
            "file": {
                "filename": filename,
                "format": format,
                "content_type": content_type,
                "content": _b64.b64encode(content).decode(),
            },
        }
        return self.create_revision(item_id, body)

    def verify_blob(self, payload):
        """True if the payload bytes hash to their content address."""
        import hashlib as _h
        return _h.sha256(payload.bytes).hexdigest() == payload.blob_sha256
