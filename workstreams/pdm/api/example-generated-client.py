#!/usr/bin/env python3
"""P8-M6-T01: browser/PDM integration demo using the GENERATED python client.

Mirror of crates/oep-pdm/examples/browser-open-save-demo.rs, driven through
python/oep_pdm_client.py (generated from workstreams/pdm/api/openapi-v1.yaml). Boots the
retained oep-pdm-stub binary on an ephemeral port and does the two verbs the
OEP workbench browser buttons will make:

  * open-from-PDM          open_latest_file (and open_revision_file)
  * save-revisions-to-PDM  save_revision (bytes in, next revision out)

Run (from repo root):
    python3 workstreams/pdm/api/example-generated-client.py

Stdlib only; starts its own stub server, no network beyond 127.0.0.1.
"""
import hashlib
import os
import socket
import subprocess
import sys
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
STUB_BIN = os.path.join(ROOT, "target", "debug", "oep-pdm-stub")

sys.path.insert(0, os.path.join(ROOT, "python"))
import oep_pdm_client as _oep_client  # noqa: E402 - after sys.path
from oep_pdm_client import PdmClient  # noqa: E402 - after sys.path


def free_port() -> int:
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def main() -> int:
    if not os.path.isfile(STUB_BIN):
        subprocess.run(["cargo", "build", "-p", "oep-pdm", "--bin", "oep-pdm-stub"],
                       cwd=ROOT, check=True, capture_output=True)
    port = free_port()
    addr = f"127.0.0.1:{port}"
    proc = subprocess.Popen([STUB_BIN, "--addr", addr],
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    try:
        c = PdmClient(base_url=f"http://{addr}/api/v1")
        # Wait for liveness.
        for _ in range(100):
            try:
                status, h = c.health()
                if status == 200:
                    break
            except Exception:  # noqa: BLE001 - server still booting
                time.sleep(0.1)
        else:
            raise RuntimeError("stub did not become healthy")

        print(f"PDM contract root : /api/v1 (generated client, SPEC_SHA256={_oep_client.SPEC_SHA256[:12]}...)")
        print(f"health            : {h}")

        step = (b"ISO-10303-21; HEADER; FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }')); "
                b"ENDSEC; DATA; ENDSEC; END-ISO-10303-21;")

        # The browser "new part, save to PDM" path.
        status, item = c.create_item({
            "name": "DemoBracket", "kind": "part", "description": "browser demo",
            "owner": "p8-m6", "context": "example",
        })
        assert status == 201, item
        print(f"\ncreated item : {item['name']} (kind={item['kind']})")

        # Save revision 1 (initial geometry).
        status, r1 = c.save_revision(
            item["id"], "bracket.step", "step", step,
            state="working", comment="initial", created_by="oep-browser",
        )
        assert status == 201, r1
        print(f"saved rev {r1['number']} : {r1['files'][0]['filename']} "
              f"({r1['files'][0]['size']} bytes, sha={r1['files'][0]['blob_sha256'][:12]}...)")

        # Open-from-PDM: byte-exact download, content address verified.
        payload = c.open_latest_file(item["id"])
        assert payload.bytes == step, "byte-exact open-from-PDM"
        assert payload.blob_sha256 == hashlib.sha256(step).hexdigest()
        print(f"open-from-PDM : rev {payload.rev} '{payload.filename}' -> "
              f"{len(payload.bytes)} bytes (blob sha256 verified: {c.verify_blob(payload)})")

        # Save a second revision, open that specific one.
        step2 = step + b" DATA; #1=NAMED_UNIT(*) ; ENDSEC; END-ISO-10303-21;"
        status, r2 = c.save_revision(item["id"], "bracket.step", "step", step2,
                                     comment="lumberjacked")
        assert status == 201 and r2["number"] == "2", r2
        payload2 = c.open_revision_file(item["id"], "2")
        assert payload2.bytes == step2 and c.verify_blob(payload2)
        print(f"open rev {r2['number']} : {len(payload2.bytes)} bytes (verified)")

        status, rl = c.list_revisions(item["id"])
        print(f"revisions : {len(rl['revisions'])} (latest {rl['latest']})")

        print("\nP8-M6-T01 generated python client demo: OK")
        return 0
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)


if __name__ == "__main__":
    sys.exit(main())
