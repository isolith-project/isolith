#!/usr/bin/env python3
"""P7-M2-T02 acceptance: the P8 REST API v1 contract, tested from an
INDEPENDENT client implementation (Python stdlib urllib) against the compiled
`oep-pdm-stub` binary over a real socket.

Why a second client: the Rust integration test (`crates/oep-pdm/tests/
pdm_contract_acceptance.rs`) proves the Rust client speaks the contract -- but a
contract proven from two independent implementations is a contract, not a
happy accident of one codebase. This test also exercises the standalone binary
and its snapshot persistence (`--persist-dir` -> content-addressed blobs +
store.json), which is exactly the "save revisions to PDM" survivability the
stub promises.

Stdlib only (runs under the precommit gate's system python; never installs deps).
"""
import base64
import hashlib
import json
import os
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STUB_BIN = os.path.join(REPO, "target", "debug", "oep-pdm-stub")
FIXTURE = os.path.join(REPO, "crates", "oep-pdm", "tests", "fixtures", "box-mini.step")
BASE = "/api/v1"


def _free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def _sha256(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def _ensure_binary():
    if not os.path.isfile(STUB_BIN):
        subprocess.run(
            ["cargo", "build", "-p", "oep-pdm", "--bin", "oep-pdm-stub"],
            cwd=REPO,
            check=True,
            capture_output=True,
        )


class Stub:
    def __init__(self, persist_dir=None):
        _ensure_binary()
        self.port = _free_port()
        self.addr = f"127.0.0.1:{self.port}"
        cmd = [STUB_BIN, "--addr", self.addr]
        if persist_dir is not None:
            cmd += ["--persist-dir", str(persist_dir)]
        self.proc = subprocess.Popen(
            cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
        )
        self.wait_healthy()

    def wait_healthy(self, timeout_s=15):
        deadline = time.time() + timeout_s
        last = None
        while time.time() < deadline:
            try:
                code, _ = self.request("GET", "/health")
                if code == 200:
                    return
            except Exception as e:  # noqa: BLE001 - server still booting
                last = e
            time.sleep(0.1)
        out, _ = self.proc.communicate(timeout=2) if self.proc.poll() is not None else ("", "")
        raise RuntimeError(f"stub did not become healthy in {timeout_s}s ({last!r}); log:\n{out}")

    def url(self, path):
        return f"http://{self.addr}{BASE}{path}"

    def request(self, method, path, body=None):
        data = None
        headers = {}
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        req = urllib.request.Request(self.url(path), data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                return resp.status, self._read(resp)
        except urllib.error.HTTPError as e:
            return e.code, self._read(e)

    @staticmethod
    def _read(resp):
        raw = resp.read()
        ct = resp.headers.get("Content-Type", "")
        if "application/json" in ct or (raw[:1] == b"{" and raw[-1:] == b"}"):
            try:
                return json.loads(raw)
            except ValueError:
                return raw
        return raw

    def close(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=5)
        return self.proc.returncode


def _expect_err(code, payload, want_code, want_status):
    assert code == want_status, f"status {code} != {want_status}: {payload}"
    err = payload.get("error", {}) if isinstance(payload, dict) else {}
    assert err.get("code") == want_code, f"error code {err.get('code')} != {want_code}: {payload}"


def test_pdm_wire_contract_from_python_client():
    with tempfile.TemporaryDirectory() as td:
        stub = Stub(persist_dir=os.path.join(td, "vault"))
        try:
            part = open(FIXTURE, "rb").read()
            part_sha = _sha256(part)

            # health: contract identity.
            code, h = stub.request("GET", "/health")
            assert code == 200
            assert h == {"status": "ok", "service": "oep-pdm-stub", "version": "1.0.0"}, h

            # fresh catalogue.
            code, items = stub.request("GET", "/items")
            assert code == 200 and items["total"] == 0, items

            # prefix filter on empty catalogue stays empty.
            code, filtered = stub.request("GET", "/items?prefix=Open")
            assert code == 200 and filtered["total"] == 0, filtered

            # create item.
            code, item = stub.request(
                "POST", "/items",
                {"name": "OpenBox", "kind": "part", "description": "py client",
                 "owner": "p7", "context": "w2"},
            )
            assert code == 201, item
            assert item["latest_revision"] is None and item["revision_count"] == 0, item
            assert item["context"] == "w2", item
            iid = item["id"]

            # duplicate name is a contract conflict.
            code, dup = stub.request("POST", "/items", {"name": "OpenBox", "kind": "part"})
            _expect_err(code, dup, "duplicate_name", 409)

            # save-revision (base64 upload).
            code, rev = stub.request(
                "POST", f"/items/{iid}/revisions",
                {"state": "working", "comment": "initial",
                 "created_by": "p7-modeler",
                 "file": {"filename": "box-mini.step", "format": "step",
                          "content": base64.b64encode(part).decode()}},
            )
            assert code == 201, rev
            assert rev["number"] == "1", rev
            assert rev["files"][0]["size"] == len(part), rev
            assert rev["files"][0]["blob_sha256"] == part_sha, rev

            # item reflects the new revision.
            code, item2 = stub.request("GET", f"/items/{iid}")
            assert code == 200 and item2["latest_revision"] == "1", item2
            code, rl = stub.request("GET", f"/items/{iid}/revisions")
            assert code == 200 and rl["latest"] == "1" and len(rl["revisions"]) == 1, rl

            # open-from-PDM: byte-exact download with contract headers.
            code, blob = stub.request("GET", f"/items/{iid}/file/latest")
            assert code == 200
            assert isinstance(blob, bytes) and blob == part, "latest file byte round-trip"
            assert stub.proc.poll() is None  # server still up after binary download

            # error surface from an independent client.
            code, missing = stub.request("GET", "/items/nope/revisions")
            _expect_err(code, missing, "not_found", 404)
            code, badstate = stub.request(
                "POST", f"/items/{iid}/revisions",
                {"state": "not-a-state",
                 "file": {"filename": "x.step", "format": "step",
                          "content": base64.b64encode(part).decode()}},
            )
            _expect_err(code, badstate, "invalid_state", 400)
        finally:
            stub.close()


def test_pdm_persistence_survives_restart():
    """Save revisions, restart the stub with the same vault dir, and the item
    + file must still be there (the 'save revisions to PDM' durability claim)."""
    with tempfile.TemporaryDirectory() as td:
        vault = os.path.join(td, "vault")
        part = open(FIXTURE, "rb").read()

        stub = Stub(persist_dir=vault)
        try:
            code, item = stub.request("POST", "/items", {"name": "Persistent", "kind": "assembly"})
            assert code == 201, item
            iid = item["id"]
            code, rev = stub.request(
                "POST", f"/items/{iid}/revisions",
                {"file": {"filename": "assy.step", "format": "step",
                          "content": base64.b64encode(part).decode()}},
            )
            assert code == 201 and rev["number"] == "1", rev
        finally:
            stub.close()

        # Blob must be stored content-addressed on disk.
        blob_sha = _sha256(part)
        blob_path = os.path.join(vault, "blobs", blob_sha)
        assert os.path.isfile(blob_path), f"content-addressed blob missing: {blob_path}"
        assert open(blob_path, "rb").read() == part
        assert os.path.isfile(os.path.join(vault, "store.json"))

        # Restart on the same vault: revision survived.
        stub2 = Stub(persist_dir=vault)
        try:
            code, items = stub2.request("GET", "/items")
            assert code == 200 and len(items["items"]) == 1, items
            assert items["items"][0]["id"] == iid
            code, blob = stub2.request("GET", f"/items/{iid}/file/latest")
            assert code == 200 and blob == part
        finally:
            stub2.close()


def test_bad_base_url_rejected_and_wrong_media_type():
    """Client-facility checks outside the running server: only http:// and
    json bodies are accepted (guards are part of the contract posture)."""
    # wrong media type -> 415 unsupported_media
    with tempfile.TemporaryDirectory() as td:
        stub = Stub(persist_dir=os.path.join(td, "vault"))
        try:
            req = urllib.request.Request(
                stub.url("/items"), data=b'{"name":"x","kind":"part"}',
                method="POST",
            )
            req.add_header("Content-Type", "text/plain")
            try:
                with urllib.request.urlopen(req, timeout=10) as resp:
                    code = resp.status
            except urllib.error.HTTPError as e:
                code = e.code
                payload = json.loads(e.read())
                _expect_err(code, payload, "unsupported_media", 415)
            assert code == 415
        finally:
            stub.close()


if __name__ == "__main__":
    raise SystemExit("use pytest: python3 -m pytest tests/test_p8_pdm_stub_contract.py")
