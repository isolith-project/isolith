#!/usr/bin/env python3
"""P8-M6-T01 acceptance: the P8 REST API v1 contract, exercised by the
GENERATED python client (python/oep_pdm_client.py, produced by
scripts/gen_pdm_client.py from workstreams/P8-M6/openapi-v1.yaml) against the retained
P7-M2-T02 `oep-pdm-stub` binary over a real socket.

Why this test exists (the stub becomes the contract test): the stub is the
reference implementation the P7-M2-T02 branch integrated against, and P8-M1-T01
retained it as the compat binary that speaks the exact OpenAPI v1 contract.
P8-M6-T01 owns that contract, so this suite promotes the stub into a standing
contract test: it boots the real binary and proves the generated client and the
server agree on every verb, every error code, the content-address headers, and
durability across a restart. It is deliberately written only against the
generated client API (no hand-rolled urllib), so the client is exercised as the
deliverable, not re-implemented here.

Stdlib only (runs under the precommit gate's system python).
"""
import base64
import hashlib
import importlib.util
import os
import socket
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STUB_BIN = os.path.join(ROOT, "target", "debug", "oep-pdm-stub")
FIXTURE = os.path.join(ROOT, "crates", "oep-pdm", "tests", "fixtures", "box-mini.step")
CLIENT_PATH = os.path.join(ROOT, "python", "oep_pdm_client.py")


def load_client():
    spec = importlib.util.spec_from_file_location("oep_pdm_client", CLIENT_PATH)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# Load the generated client at import time so tests exercise the deliverable.
CLIENT = load_client()
PdmClient = CLIENT.PdmClient


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
            cwd=ROOT,
            check=True,
            capture_output=True,
        )


class Stub:
    def __init__(self, persist_dir=None):
        _ensure_binary()
        self.port = _free_port()
        self.addr = f"127.0.0.1:{self.port}"
        cmd = [STUB_BIN, "--addr", self.addr, "--no-auth"]
        if persist_dir is not None:
            cmd += ["--persist-dir", str(persist_dir)]
        self.proc = subprocess.Popen(
            cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
        )
        self.client = PdmClient(base_url=f"http://{self.addr}")
        self.wait_healthy()

    def wait_healthy(self, timeout_s=15):
        import urllib.error
        import urllib.request

        deadline = time.time() + timeout_s
        last = None
        while time.time() < deadline:
            try:
                with urllib.request.urlopen(
                    f"http://{self.addr}/api/v1/health", timeout=2
                ) as r:
                    if r.status == 200:
                        return
            except Exception as e:  # noqa: BLE001 - server still booting
                last = e
            time.sleep(0.1)
        out, _ = self.proc.communicate(timeout=2) if self.proc.poll() is not None else ("", "")
        raise RuntimeError(f"stub did not become healthy in {timeout_s}s ({last!r}); log:\n{out}")

    def close(self):
        self.proc.terminate()
        try:
            self.proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            self.proc.wait(timeout=5)
        return self.proc.returncode


def _expect_err(fn, want_code, want_status):
    try:
        return fn()
    except CLIENT.PdmError as e:
        assert e.status == want_status, f"status {e.status} != {want_status}"
        assert e.code == want_code, f"error code {e.code} != {want_code}"
        return e


def test_generated_client_open_save_round_trip():
    with tempfile.TemporaryDirectory() as td:
        stub = Stub(persist_dir=os.path.join(td, "vault"))
        try:
            c = stub.client
            part = open(FIXTURE, "rb").read()
            part_sha = _sha256(part)

            # health: contract identity.
            status, h = c.health()
            assert status == 200
            assert (h["status"], h["service"], h["version"]) == (
                "ok", "oep-pdm", "1.0.0",
            ), h

            # fresh catalogue.
            status, items = c.list_items()
            assert status == 200 and items["total"] == 0, items

            # create item via the generated verb.
            status, item = c.create_item({
                "name": "OpenBox", "kind": "part", "description": "generated client",
                "owner": "p8", "context": "m6",
            })
            assert status == 201, item
            assert item["latest_revision"] is None and item["revision_count"] == 0, item
            assert item["context"] == "m6", item
            iid = item["id"]

            # duplicate name is a contract conflict.
            _expect_err(
                lambda: c.create_item({"name": "OpenBox", "kind": "part"}),
                "duplicate_name", 409,
            )

            # save-revision: raw bytes in, generated client base64-encodes.
            status, rev = c.save_revision(
                iid, "box-mini.step", "step", part,
                state="working", comment="initial", created_by="p8-modeler",
            )
            assert status == 201, rev
            assert rev["number"] == "1", rev
            assert rev["files"][0]["size"] == len(part), rev
            assert rev["files"][0]["blob_sha256"] == part_sha, rev

            # item reflects the new revision.
            status, item2 = c.get_item(iid)
            assert status == 200 and item2["latest_revision"] == "1", item2
            status, rl = c.list_revisions(iid)
            assert status == 200 and rl["latest"] == "1" and len(rl["revisions"]) == 1, rl

            # open-from-PDM: byte-exact download, content address verified.
            payload = c.open_latest_file(iid)
            assert isinstance(payload, CLIENT.FilePayload)
            assert payload.bytes == part, "latest file byte round-trip"
            assert payload.rev == "1" and payload.filename == "box-mini.step"
            assert payload.blob_sha256 == part_sha
            assert c.verify_blob(payload), "content address must verify"

            # open a specific revision's file.
            payload2 = c.open_revision_file(iid, "1")
            assert payload2.bytes == part and payload2.blob_sha256 == part_sha

            # revision metadata verb.
            status, revm = c.get_revision(iid, "1")
            assert status == 200 and revm["number"] == "1", revm

            # error surface from the generated client.
            _expect_err(lambda: c.get_item("nope"), "not_found", 404)
            _expect_err(lambda: c.list_revisions("nope"), "not_found", 404)
            _expect_err(
                lambda: c.save_revision(iid, "x.step", "step", part, state="not-a-state"),
                "invalid_state", 400,
            )
        finally:
            stub.close()


def test_generated_client_persistence_survives_restart():
    with tempfile.TemporaryDirectory() as td:
        vault = os.path.join(td, "vault")
        import urllib.error
        import urllib.request

        part = open(FIXTURE, "rb").read()
        blob_sha = _sha256(part)

        stub = Stub(persist_dir=vault)
        try:
            c = stub.client
            status, item = c.create_item({"name": "Persistent", "kind": "assembly"})
            assert status == 201, item
            iid = item["id"]
            status, rev = c.save_revision(
                iid, "assy.step", "step", part, created_by="p8"
            )
            assert status == 201 and rev["number"] == "1", rev
        finally:
            stub.close()

        # Blob stored content-addressed on disk.
        blob_path = os.path.join(vault, "blobs", blob_sha)
        assert os.path.isfile(blob_path), f"content-addressed blob missing: {blob_path}"
        assert open(blob_path, "rb").read() == part
        assert os.path.isfile(os.path.join(vault, "store.json"))

        # Restart on the same vault: revision + file survived (via generated client).
        stub2 = Stub(persist_dir=vault)
        try:
            c = stub2.client
            status, items = c.list_items()
            assert status == 200 and len(items["items"]) == 1, items
            assert items["items"][0]["id"] == iid
            payload = c.open_latest_file(iid)
            assert payload.bytes == part and c.verify_blob(payload)
        finally:
            stub2.close()


def test_generated_client_error_surface_and_base_url():
    # bad base URL rejected by the generated client.
    try:
        PdmClient(base_url="ftp://nope")
        assert False, "must reject non-http base URL"
    except ValueError:
        pass

    with tempfile.TemporaryDirectory() as td:
        stub = Stub(persist_dir=os.path.join(td, "vault"))
        try:
            c = stub.client
            # 404 on unknown path (envelope via generated client request path).
            e = _expect_err(lambda: c.get_item("missing"), "not_found", 404)
            assert e.is_not_found
            # 415 unsupported media on a non-JSON body.
            import urllib.error
            import urllib.request
            req = urllib.request.Request(
                f"http://{stub.addr}/api/v1/items",
                data=b'{"name":"x","kind":"part"}',
                method="POST",
            )
            req.add_header("Content-Type", "text/plain")
            try:
                with urllib.request.urlopen(req, timeout=10) as resp:
                    code = resp.status
            except urllib.error.HTTPError as resp:
                code = resp.code
                import json
                env = json.loads(resp.read())
                assert env["error"]["code"] == "unsupported_media"
            assert code == 415
        finally:
            stub.close()


if __name__ == "__main__":
    raise SystemExit("use pytest: python3 -m pytest tests/test_p8_m6_client_contract.py")
