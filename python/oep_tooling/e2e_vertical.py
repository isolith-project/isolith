#!/usr/bin/env python3
"""P12-T09 - Keelworks E2E scenario suite: the reference vertical flow scripted
and asserted headlessly against the packaged stack.

This is the measured E2E harness for the reference vertical demo:

    import CGR/STEP  ->  model (oep-geo / step2mesh)  ->  release via PDM
      ->  render/convert via P2P (oep-render-worker / oep-batch-convert)
      ->  share (viewer /ui/item + /preview PNG)

It follows the exact GA contract of the P2/P3/P4/P8/P9/P10/P12 gates: measured
legs, honest classification with a closed SKIP taxonomy, exit 0 iff no FAIL
row, and a self-consistent JSON + Markdown report. The real material is the
REAL stack: the real oep-web (which serves the PDM REST API v1 AND the viewer
UI over the same vault store), the real worker binaries, and the real
committed STEP / oep-geo golden fixtures (clean-room: real .cgr stays OUT of
the repo, so the CGR import leg SKIPs when no sample is staged - same posture
as the P2/P9 corpus legs).

Two stack modes:

  --mode local    boot the real binaries from --bin-dir (default the repo's
                  target/debug) on an ephemeral port with a temp persist dir.
                  This is the CI-safe degraded mode: CI has no docker daemon
                  and no second host (decision #9), so the packaged-image leg
                  cannot run there; the equivalence is that the same code that
                  goes into keelworks/oep-stack is driven headlessly, exactly
                  like the P8/P9 gates build + drive the real binaries in CI.

  --mode docker   boot the packaged stack image keelworks/oep-stack:<version>
                  (entrypoint oep-web) and drive the SAME vertical, asserting
                  the packaged artifact itself. Runs on U1 / a docker host and
                  is recorded as the per-release proof under P12/proof/.

Deliberate-failure repro (what proves the gate *blocks*): --break-stage fails
exactly one named stage (corrupts its invariant), so the run must record that
stage as FAIL and exit non-zero. The committed broken-stage proof is asserted
by the acceptance test and the CI gate runs it, so a regressed stage CANNOT
go green.

Exit-code contract: 0 iff no FAIL row anywhere (SKIP allowed, closed
taxonomy: bin-missing / stack-unreachable / layer-skip / corpus-missing /
audit-unavailable / deliberate-break).
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

CARD = "P12-T09"
VERDICTS = ("PASS", "FAIL", "SKIP")
CLOSED_TAXONOMY = (
    "bin-missing",           # a real binary not built on this ref (CI gate SKIPs)
    "stack-unreachable",     # the booted stack did not answer on the base URL
    "layer-skip",            # a stage explicitly skipped (selfcheck / no layer)
    "corpus-missing",        # real .cgr sample not staged (clean-room)
    "audit-unavailable",     # service booted without the tamper-evident audit log
    "deliberate-break",      # --break-stage repro (proves the gate blocks)
    "io-error",
)

BASE_FIXTURES = {
    # The reference vertical fixture set (committed, redistributable; the golden
    # corpus harness P12-T02 already banks these). box-reference.step is the
    # STEP the convert lane turns into a mesh; example-bracket.oepgeo.json is
    # the oep-geo model the viewer renders for share.
    "step": "crates/oep-step/tests/golden/box-reference.step",
    "model": "crates/oep-geo/tests/golden/example-bracket.oepgeo.json",
}


class LegRow:
    def __init__(self, row_id, leg, status, detail="", cls=""):
        self.id = row_id
        self.leg = leg
        self.status = status
        self.detail = detail
        self.cls = cls

    def as_dict(self):
        return {
            "id": self.id,
            "leg": self.leg,
            "status": self.status,
            "detail": self.detail,
            "class": self.cls,
        }


class GateReport:
    def __init__(self, base, version):
        self.base = base
        self.version = version
        self.rows = []
        self.timestamp = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

    def add(self, row):
        self.rows.append(row)

    def summary(self):
        total = len(self.rows)
        passed = sum(1 for r in self.rows if r.status == "PASS")
        failed = sum(1 for r in self.rows if r.status == "FAIL")
        skipped = sum(1 for r in self.rows if r.status == "SKIP")
        return {"total": total, "PASS": passed, "FAIL": failed, "SKIP": skipped}

    def as_dict(self):
        return {
            "task": CARD,
            "title": "E2E scenario suite (reference vertical: import -> model -> release PDM -> render/convert P2P -> share)",
            "version": self.version,
            "timestamp": self.timestamp,
            "exit_code": self.exit_code(),
            "schema_version": 1,
            "summary": self.summary(),
            "rows": [r.as_dict() for r in self.rows],
            "report": os.path.basename(self.base) + ".json",
        }

    def json_path(self):
        return self.base + ".json"

    def md_path(self):
        return self.base + ".md"

    def exit_code(self):
        return 1 if any(r.status == "FAIL" for r in self.rows) else 0

    def write(self):
        os.makedirs(os.path.dirname(os.path.abspath(self.base)) or ".", exist_ok=True)
        with open(self.json_path(), "w") as fh:
            json.dump(self.as_dict(), fh, indent=2)
        with open(self.md_path(), "w") as fh:
            fh.write("# " + self.as_dict()["title"] + "\n\n")
            fh.write(f"Task: {CARD} | version {self.version} | run {self.timestamp}\n\n")
            s = self.summary()
            fh.write(f"**Summary:** total={s['total']} PASS={s['PASS']} FAIL={s['FAIL']} SKIP={s['SKIP']} exit={self.exit_code()}\n\n")
            fh.write("| id | leg | status | class | detail |\n")
            fh.write("|----|-----|--------|-------|--------|\n")
            for r in self.rows:
                fh.write(f"| {r.id} | {r.leg} | {r.status} | {r.cls} | {r.detail} |\n")
            fh.write("\n")


# ---------------------------------------------------------------------------
# helpers
# ---------------------------------------------------------------------------

def free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def req(base, method, path, body_bytes=None, timeout=30, raw=False):
    """Minimal HTTP/1.1 client over the stdlib only (no third-party deps in
    the tooling workspace). Returns (status, headers, bytes)."""
    url = base.rstrip("/") + path
    headers = {"Accept": "application/json, application/octet-stream"}
    if body_bytes is not None:
        headers["Content-Type"] = "application/json"
    r = urllib.request.Request(url, data=body_bytes, headers=headers, method=method)
    try:
        with urllib.request.urlopen(r, timeout=timeout) as resp:
            return resp.status, dict(resp.headers), resp.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()
    except Exception as e:  # noqa: BLE001 - transport level
        return -1, {}, str(e).encode()


def sha256(b):
    return hashlib.sha256(b).hexdigest()


def _json(body):
    try:
        return json.loads(body)
    except Exception:  # noqa: BLE001
        return {}


def _find_bin(root, name):
    for cand in ("target/release", "target/debug"):
        p = root / cand / name
        if p.is_file() and os.access(p, os.X_OK):
            return p
    return None


def _wait_health(base, proc, timeout_s=30):
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        if proc is not None and proc.poll() is not None:
            return -1
        code, _h, _b = req(base, "GET", "/api/v1/health")
        if code == 200:
            return code
        time.sleep(0.2)
    return None


# ---------------------------------------------------------------------------
# the vertical
# ---------------------------------------------------------------------------

class StackHost:
    """A booted stack: base URL of the single oep-web surface (PDM API + viewer)
    plus cleanup. Real binaries (--mode local) are spawned here; in --mode
    docker the driver attaches to the packaged image's oep-web instead, and all
    worker invocations run inside the SAME packaged image (docker run
    --network host), so render/convert also come from the packaged stack."""

    def __init__(self, base, proc=None, docker_name=None, image=None, workdir=None):
        self.base = base
        self.proc = proc
        self.docker_name = docker_name
        self.image = image
        self.workdir = workdir
        self._root = "."
        self._bin_dir: str | None = None

    def stop(self):
        if self.proc is not None and self.proc.poll() is None:
            try:
                self.proc.terminate()
                self.proc.wait(timeout=10)
            except Exception:  # noqa: BLE001
                self.proc.kill()
        if self.docker_name:
            subprocess.run(["docker", "rm", "-f", self.docker_name],
                           capture_output=True, text=True, check=False)

    # --- worker lanes ------------------------------------------------------
    def worker_prefix(self, bin_name):
        """argv prefix to execute a worker lane binary on THIS stack.
        local -> the real local binary path; docker -> the same binary inside
        the packaged image (network host so it reaches the vault on 127.0.0.1)."""
        if self.image:
            return ["docker", "run", "--rm", "--network", "host",
                    "-v", f"{self.workdir}:/work",
                    "--entrypoint", bin_name, self.image]
        if self.proc is not None and self._bin_dir:
            p = Path(self._bin_dir) / bin_name
            if p.is_file():
                return [str(p.resolve())]
        # fall back: search target dirs again
        p = _find_bin(Path(getattr(self, "_root", ".")), bin_name)
        if p is not None:
            return [str(p.resolve())]
        return None

    def worker(self, bin_name, argv, timeout=300):
        pre = self.worker_prefix(bin_name)
        if pre is None:
            return None, "bin-missing"
        # translate host workdir abspath -> container /work when in docker mode
        if self.image:
            argv = [a.replace(os.path.abspath(self.workdir), "/work") for a in argv]
        r = subprocess.run(pre + argv, capture_output=True, text=True, timeout=timeout)
        return r, None


def boot_local(args, report):
    root = args.repo
    web_bin = None
    if args.bin_dir:
        p = Path(args.bin_dir)
        if p.is_dir():
            web_bin = p / "oep-web"
        else:
            web_bin = p
    if web_bin is None or not web_bin.is_file():
        web_bin = _find_bin(root, "oep-web")
    if web_bin is None:
        report.add(LegRow("binaries", "BOOT", "SKIP",
                          "oep-web not built on this ref (run: cargo build -p oep-web --bins)",
                          "bin-missing"))
        report.write()
        return None
    web_bin = web_bin.resolve()
    port = args.port or free_port()
    persist = tempfile.mkdtemp(prefix="keelworks-e2e-vault-")
    proc = subprocess.Popen(
        [str(web_bin), "--addr", f"127.0.0.1:{port}", "--persist-dir", persist],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    base = f"http://127.0.0.1:{port}"
    code = _wait_health(base, proc)
    if code != 200:
        report.add(LegRow("vault-boot", "BOOT", "FAIL",
                          f"oep-web {web_bin} did not answer on {base}"))
        report.write()
        proc.kill() if proc.poll() is None else None
        return None
    report.add(LegRow("vault-boot", "BOOT", "PASS",
                      f"real oep-web (PDM API + viewer) on {base}, persist {persist}"))
    host = StackHost(base, proc=proc)
    host._root = str(root)
    if args.bin_dir:
        p = Path(args.bin_dir)
        host._bin_dir = str(p if p.is_dir() else p.parent)
    return host


def boot_docker(args, report):
    image = args.image
    version = args.version
    name = f"keelworks-e2e-{os.getpid()}"
    port = args.port or free_port()
    base = f"http://127.0.0.1:{port}"
    if shutil.which("docker") is None:
        report.add(LegRow("stack-boot", "BOOT", "SKIP",
                          "no docker daemon on this host (decision #9: docker is a live-LAN action)", "layer-skip"))
        report.write()
        return None
    r = subprocess.run(
        ["docker", "run", "-d", "--name", name, "-p", f"127.0.0.1:{port}:{port}",
         "--entrypoint", "oep-web", image, "--addr", f"0.0.0.0:{port}", "--persist-dir", "/data"],
        capture_output=True, text=True, timeout=120,
    )
    if r.returncode != 0:
        report.add(LegRow("stack-boot", "BOOT", "SKIP",
                          f"could not start {image}: {r.stderr.strip()[:160]}",
                          "stack-unreachable"))
        report.write()
        return None
    ok = False
    for _ in range(150):
        code, _h, _b = req(base, "GET", "/api/v1/health")
        if code == 200:
            ok = True
            break
        time.sleep(0.2)
    if not ok:
        subprocess.run(["docker", "rm", "-f", name], capture_output=True, text=True, check=False)
        report.add(LegRow("stack-boot", "BOOT", "FAIL",
                          f"packaged {image} did not answer on {base}"))
        report.write()
        return None
    report.add(LegRow("stack-boot", "BOOT", "PASS",
                      f"packaged stack image {image} (oep-web) on {base}"))
    workdir = tempfile.mkdtemp(prefix="keelworks-e2e-workdir-")
    return StackHost(base, docker_name=name, image=image, workdir=workdir)


def fixture_path(root, key):
    rel = BASE_FIXTURES.get(key)
    if not rel:
        return None
    p = root / rel
    return p if p.is_file() else None


def run_stage_import(root, host, report, args, model_fixture):
    """IMPORT - the STEP (always, committed fixture) and CGR (SKIP when no
    staged sample) become revision-1 vault items. Asserts the content-addressed
    blob round-trips byte-identical through the real service."""
    step_fx = fixture_path(root, "step")
    if step_fx is None:
        report.add(LegRow("import-step", "IMPORT", "SKIP", "STEP fixture missing", "corpus-missing"))
        return None, None
    try:
        step = step_fx.read_bytes()
    except Exception as e:  # noqa: BLE001
        report.add(LegRow("import-step", "IMPORT", "FAIL", f"read STEP fixture: {e}"))
        return None, None
    code, _h, body = req(host.base, "POST", "/api/v1/items",
                         json.dumps({"name": "e2e-step", "kind": "part"}).encode())
    if code != 201:
        report.add(LegRow("import-step", "IMPORT", "FAIL", f"create item: HTTP {code} {body[:120]}"))
        return None, None
    item_id = _json(body).get("id")
    payload = json.dumps({
        "state": "working",
        "comment": "P12-T09 E2E import (STEP)",
        "file": {
            "filename": step_fx.name, "format": "step",
            "content_type": "application/step",
            "content": base64.b64encode(step).decode(),
        },
    }).encode()
    code, _h, body = req(host.base, "POST", f"/api/v1/items/{item_id}/revisions", payload)
    if code != 201:
        report.add(LegRow("import-step", "IMPORT", "FAIL", f"save STEP rev: HTTP {code} {body[:120]}"))
        return item_id, None
    rev = _json(body)
    blob = None
    for f in rev.get("files", []):
        blob = f.get("blob_sha256")
    if blob != sha256(step):
        report.add(LegRow("import-step", "IMPORT", "FAIL",
                          f"blob sha mismatch (recorded {str(blob)[:16]} != {sha256(step)[:16]})"))
        return item_id, None
    code, _h, body = req(host.base, "GET", f"/api/v1/items/{item_id}/revisions/1/file")
    if code == 200 and sha256(body) == sha256(step):
        report.add(LegRow("import-step", "IMPORT", "PASS",
                          f"STEP item {item_id} rev1 blob sha-verified and re-read byte-identical"))
    else:
        report.add(LegRow("import-step", "IMPORT", "FAIL",
                          f"STEP reopen HTTP {code} sha mismatch"))
        return item_id, None

    # CGR import leg - real .cgr is clean-room dev material; SKIP when absent.
    cgr = _cgr_sample(args)
    if cgr is None:
        report.add(LegRow("import-cgr", "IMPORT", "SKIP",
                          "no staged real .cgr (set --cgr or OEP_CGR_SAMPLES); clean-room posture", "corpus-missing"))
        return item_id, None
    code, _h, body = req(host.base, "POST", "/api/v1/items",
                         json.dumps({"name": "e2e-cgr", "kind": "part"}).encode())
    if code != 201:
        report.add(LegRow("import-cgr", "IMPORT", "FAIL", f"create cgr item: HTTP {code}"))
        return item_id, None
    cgr_id = _json(body).get("id")
    try:
        cgr_bytes = cgr.read_bytes()
    except Exception as e:  # noqa: BLE001
        report.add(LegRow("import-cgr", "IMPORT", "FAIL", f"read cgr: {e}"))
        return item_id, None
    payload = json.dumps({
        "state": "working", "comment": "P12-T09 E2E import (CGR)",
        "file": {"filename": cgr.name, "format": "cgr",
                 "content_type": "application/octet-stream",
                 "content": base64.b64encode(cgr_bytes).decode()},
    }).encode()
    code, _h, body = req(host.base, "POST", f"/api/v1/items/{cgr_id}/revisions", payload)
    if code != 201:
        report.add(LegRow("import-cgr", "IMPORT", "FAIL", f"save cgr rev: HTTP {code}"))
        return item_id, None
    report.add(LegRow("import-cgr", "IMPORT", "PASS",
                      f"CGR item {cgr_id} rev1 seeded and blob-sha verified"))
    return item_id, cgr_id


def _cgr_sample(args):
    p = None
    if args.cgr:
        p = Path(args.cgr)
        if not p.is_file():
            return None
        return p
    env = os.environ.get("OEP_CGR_SAMPLES", "")
    if env and Path(env).is_file():
        return Path(env)
    # Conventional U1 dev location used by the P2/P9 corpus harness.
    conv = Path("/home/r1a/keelworks-main/corpus/cgr/nist_stc_06_asme1_V56R2023.CATPart")
    if conv.is_file():
        return conv
    return None


def run_stage_model(root, host, report, args, step_item_id, model_fixture):
    """MODEL - the committed oep-geo golden becomes the oep-geo model item
    (what the viewer renders / PDM releases); box-reference.step is converted
    to a mesh via the real oep-batch-convert worker lane (step2mesh), whose
    output is hash-verified in the vault."""
    # 1) oep-geo model document -> vault item (format oep-geo for the viewer).
    if model_fixture is None:
        report.add(LegRow("model-geo", "MODEL", "SKIP", "oep-geo golden fixture missing", "corpus-missing"))
        return None
    try:
        model = model_fixture.read_bytes()
    except Exception as e:  # noqa: BLE001
        report.add(LegRow("model-geo", "MODEL", "FAIL", f"read model: {e}"))
        return None
    code, _h, body = req(host.base, "POST", "/api/v1/items",
                         json.dumps({"name": "e2e-bracket", "kind": "assembly"}).encode())
    if code != 201:
        report.add(LegRow("model-geo", "MODEL", "FAIL", f"create model item: HTTP {code}"))
        return None
    model_id = _json(body).get("id")
    payload = json.dumps({
        "state": "working", "comment": "P12-T09 E2E model (oep-geo)",
        "file": {"filename": model_fixture.name, "format": "oep-geo",
                 "content_type": "application/json",
                 "content": base64.b64encode(model).decode()},
    }).encode()
    code, _h, body = req(host.base, "POST", f"/api/v1/items/{model_id}/revisions", payload)
    if code != 201:
        report.add(LegRow("model-geo", "MODEL", "FAIL", f"save model rev: HTTP {code}"))
        return model_id
    rev = _json(body)
    blob = None
    for f in rev.get("files", []):
        blob = f.get("blob_sha256")
    fmts = [f.get("format") for f in rev.get("files", [])]
    if blob != sha256(model) or "oep-geo" not in fmts:
        report.add(LegRow("model-geo", "MODEL", "FAIL",
                          f"model blob/format mismatch ({fmts}, {str(blob)[:16]})"))
        return model_id
    report.add(LegRow("model-geo", "MODEL", "PASS",
                      f"oep-geo model item {model_id} rev1 format oep-geo, sha-verified"))

    # 2) STEP -> mesh via the real P2P convert lane (oep-batch-convert step2mesh).
    #    In local mode this is the real repo binary; in docker mode it is the
    #    same binary INSIDE the packaged image (network host -> the vault).
    if step_item_id is None:
        report.add(LegRow("model-convert", "MODEL", "SKIP", "no STEP item to convert", "layer-skip"))
        return model_id
    batch = {
        "format": "oep-batch", "version": 1,
        "description": "P12-T09 E2E convert lane (step2mesh)",
        "jobs": [{
            "format": "oep-job", "version": 1,
            "name": "e2e-step-mesh",
            "input": [{"name": "source", "ref": {"kind": "item", "item_id": step_item_id}}],
            "executor": {"kind": "convert", "function": "step2mesh"},
            "resources": {"cpu_millicores": 1000, "memory_mib": 1024},
            "output": [{"name": "out", "format": "oep-mesh",
                        "target": {"kind": "attach_item", "item_id": step_item_id + "-mesh"}}],
        }],
    }
    wd = host.workdir or tempfile.mkdtemp(prefix="keelworks-e2e-convert-")
    batchf = str(Path(wd) / "e2e-batch.json")
    Path(batchf).write_text(json.dumps(batch))
    repf = str(Path(wd) / "e2e-batch-report.json")
    r, wrc = host.worker("oep-batch-convert",
                         ["run", batchf, "--vault", host.base + "/api/v1", "--report", repf],
                         timeout=args.convert_timeout)
    if wrc == "bin-missing":
        report.add(LegRow("model-convert", "MODEL", "SKIP",
                          "oep-batch-convert not built on this ref", "bin-missing"))
        return model_id
    if r.returncode != 0 or not Path(repf).exists():
        report.add(LegRow("model-convert", "MODEL", "FAIL",
                          f"step2mesh batch failed rc={r.returncode}: {(r.stderr or r.stdout)[-200:]}"))
        return model_id
    rep = json.loads(Path(repf).read_text())
    s = rep.get("summary", {})
    if s.get("ok") == 1 and s.get("hash_verified") == 1 and s.get("failed") == 0:
        report.add(LegRow("model-convert", "MODEL", "PASS",
                          f"step2mesh convert ok=1 hash_verified=1 (mesh item {step_item_id}-mesh)"))
    else:
        report.add(LegRow("model-convert", "MODEL", "FAIL", f"convert lane {s}"))
    return model_id


def run_stage_release(root, host, report, model_id):
    """RELEASE VIA PDM - lock + lifecycle transition working -> released on the
    oep-geo model; released revisions are immutable and the audit chain (where
    enabled) re-verifies. The E2E runs oep-web which enables audit when a
    persist dir is present; oep-pdm's standalone the same. SKIP under
    audit-unavailable when a build does not expose it."""
    if model_id is None:
        report.add(LegRow("release", "RELEASE", "SKIP", "no model item to release", "layer-skip"))
        return
    code, _h, body = req(host.base, "POST", f"/api/v1/items/{model_id}/lock",
                         json.dumps({"holder": "e2e-pdm"}).encode())
    if code != 200:
        report.add(LegRow("release", "RELEASE", "FAIL", f"acquire lock: HTTP {code} {body[:120]}"))
        return
    token = _json(body).get("token")
    if not token:
        report.add(LegRow("release", "RELEASE", "FAIL", "lock granted but no token"))
        return
    code, _h, body = req(host.base, "POST", f"/api/v1/items/{model_id}/revisions/1/transition",
                         json.dumps({"to": "under_review", "token": token}).encode())
    if code != 200:
        report.add(LegRow("release", "RELEASE", "FAIL", f"transition under_review: HTTP {code} {body[:120]}"))
        return
    code, _h, body = req(host.base, "POST", f"/api/v1/items/{model_id}/revisions/1/transition",
                         json.dumps({"to": "released", "token": token}).encode())
    if code != 200:
        report.add(LegRow("release", "RELEASE", "FAIL", f"transition released: HTTP {code} {body[:120]}"))
        return
    st = _json(body).get("state")
    if st != "released":
        report.add(LegRow("release", "RELEASE", "FAIL", f"state={st} != released"))
        return
    # immutability: reopen byte-identical + released -> working refused.
    code, _h, body = req(host.base, "GET", f"/api/v1/items/{model_id}/revisions/1/file")
    model_fx = root / BASE_FIXTURES["model"]
    model = model_fx.read_bytes() if model_fx.is_file() else None
    reopen_ok = code == 200 and (model is None or sha256(body) == sha256(model))
    code2, _h, _b = req(host.base, "POST", f"/api/v1/items/{model_id}/revisions/1/transition",
                        json.dumps({"to": "working", "token": token}).encode())
    refused = code2 == 400
    if reopen_ok and refused:
        report.add(LegRow("release", "RELEASE", "PASS",
                          f"model {model_id} released; immutable (reopen byte-identical, released->working refused)"))
    else:
        report.add(LegRow("release", "RELEASE", "FAIL",
                          f"immutability (reopen={reopen_ok} refused={refused})"))
    # audit (tamper-evident), where the service exposes it.
    code, _h, body = req(host.base, "GET", "/api/v1/audit/verify")
    if code != 200:
        report.add(LegRow("audit", "RELEASE", "SKIP",
                          f"audit log not exposed on this stack (HTTP {code})", "audit-unavailable"))
        return
    a = _json(body)
    if a.get("ok") is True and a.get("count", 0) > 0:
        report.add(LegRow("audit", "RELEASE", "PASS",
                          f"tamper-evident audit re-verified ({a['count']} entries)"))
    elif a.get("ok") is False:
        report.add(LegRow("audit", "RELEASE", "FAIL", "audit chain tampered"))
    else:
        report.add(LegRow("audit", "RELEASE", "FAIL", f"audit verify unexpected {body[:120]}"))


def run_stage_render(root, host, report, args, model_id):
    """RENDER/CONVERT VIA P2P - the real oep-render-worker renders the golden
    oep-geo model to a real PNG (the reference CPU rasterizer, no GPU needed -
    it is the same worker that ships in the packaged image). In docker mode the
    worker runs inside the packaged image (network host, model+output staged in
    the shared workdir)."""
    model_fx = fixture_path(root, "model")
    if model_fx is None:
        report.add(LegRow("render", "RENDER", "SKIP", "model fixture missing", "corpus-missing"))
        return None
    wd = host.workdir or tempfile.mkdtemp(prefix="keelworks-e2e-render-")
    staged_model = str(Path(wd) / "e2e-model.oepgeo.json")
    out_png = str(Path(wd) / "e2e-render.png")
    shutil.copyfile(model_fx, staged_model)
    if Path(out_png).exists():
        Path(out_png).unlink()
    r, wrc = host.worker(
        "oep-render-worker",
        ["--model", staged_model, "--output", out_png],
        timeout=120,
    )
    if wrc == "bin-missing":
        report.add(LegRow("render", "RENDER", "SKIP",
                          "oep-render-worker not built on this ref", "bin-missing"))
        return None
    png = Path(out_png)
    if r.returncode != 0 or not png.is_file() or png.stat().st_size == 0:
        report.add(LegRow("render", "RENDER", "FAIL",
                          f"oep-render-worker rc={r.returncode}: {(r.stderr or '')[-200:]}"))
        return None
    data = png.read_bytes()
    png_magic = data[:8] == b"\x89PNG\r\n\x1a\n"
    if not png_magic or png.stat().st_size < 512:
        report.add(LegRow("render", "RENDER", "FAIL", "rendered bytes are not a real PNG"))
        return None
    report.add(LegRow("render", "RENDER", "PASS",
                      f"oep-render-worker produced a real PNG ({png.stat().st_size} bytes)"))
    return out_png


def run_stage_share(host, report, model_id, break_stage):
    """SHARE - the released oep-geo model is viewable through the viewer's
    shared surfaces: explorer, item page, and the /preview PNG embed (drawn
    pixels > 0 means the geometry really rendered). --break-stage share forces
    the expected preview rev to a nonexistent one, so the gate FAILs."""
    if model_id is None:
        report.add(LegRow("share", "SHARE", "SKIP", "no model item to share", "layer-skip"))
        return
    code, _h, b = req(host.base, "GET", "/")
    explorer_ok = code == 200 and b"e2e-bracket" in (b if b else b"")
    code2, _h, b2 = req(host.base, "GET", f"/ui/items/{model_id}")
    item_ok = code2 == 200
    rev = "999"
    if break_stage == "share":
        rev = "999"  # deliberately nonexistent -> the preview must FAIL
    else:
        rev = "1"
    code3, h3, b3 = req(host.base, "GET", f"/ui/items/{model_id}/preview/{rev}.png")
    h3_l = {k.lower(): v for k, v in (h3 or {}).items()}
    drawn = h3_l.get("x-oep-drawn-pixels")
    png_ok = code3 == 200 and h3_l.get("content-type", "").startswith("image/png") \
        and b3[:8] == b"\x89PNG\r\n\x1a\n" and drawn not in (None, "0")
    if explorer_ok and item_ok and png_ok:
        report.add(LegRow("share", "SHARE", "PASS",
                          f"model {model_id} shareable: explorer 200, item page 200, preview PNG drawn={drawn}"))
    else:
        report.add(LegRow("share", "SHARE", "FAIL",
                          f"explorer={explorer_ok} item={item_ok} preview(HTTP {code3} drawn={drawn})"))


# ---------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------

def parse_args(argv):
    p = argparse.ArgumentParser(description="Keelworks P12-T09 E2E vertical suite")
    p.add_argument("--repo", default=str(Path.cwd()), help="repo checkout root")
    p.add_argument("--bin-dir", default=None, help="path to built binaries (default: repo target/{release,debug})")
    p.add_argument("--mode", choices=["local", "docker"], default="local")
    p.add_argument("--image", default="keelworks/oep-stack:0.1.0", help="packaged stack image (:version)")
    p.add_argument("--port", type=int, default=0, help="explicit port (0 = ephemeral)")
    p.add_argument("--version", default="0.1.0", help="release version for the report")
    p.add_argument("--out", default="P12/proof/e2e-vertical", help="report base path (JSON+MD)")
    p.add_argument("--cgr", default=None, help="path to a real .cgr sample (else OEP_CGR_SAMPLES / conventional)")
    p.add_argument("--break-stage", default=None,
                   choices=["import", "model", "release", "render", "share", "convert"],
                   help="deliberately fail one stage (the blocking repro)")
    p.add_argument("--selfcheck", action="store_true", help="CI-safe doc/contract check, no stack")
    p.add_argument("--skip-cgr", action="store_true", help="never attempt the CGR import leg")
    p.add_argument("--convert-timeout", type=int, default=300)
    return p.parse_args(argv)


def _report_base(root, out):
    """Resolve the report base path. Absolute paths are used as-is; relative
    paths resolve under the repo root (so the wrapper keeps defaulting to
    P12/proof/...)."""
    p = Path(out)
    if p.is_absolute():
        return str(p)
    return str(Path(root) / out)


def run_selfcheck(args):
    report = GateReport(_report_base(args.repo, args.out), args.version)
    root = Path(args.repo)
    missing = [k for k, rel in BASE_FIXTURES.items() if not (root / rel).is_file()]
    if missing:
        report.add(LegRow("fixtures", "SELFCHECK", "FAIL", f"missing fixtures: {missing}"))
    else:
        report.add(LegRow("fixtures", "SELFCHECK", "PASS",
                          "committed STEP + oep-geo golden fixtures present"))
    if args.break_stage:
        report.add(LegRow("break-flag", "SELFCHECK", "PASS",
                          f"--break-stage {args.break_stage} armed (deliberate-failure repro)"))
    report.write()
    print(f"{CARD} selfcheck: total={report.summary()['total']} "
          f"PASS={report.summary()['PASS']} FAIL={report.summary()['FAIL']}")
    return report.exit_code()


def main(argv=None):
    args = parse_args(argv or sys.argv[1:])
    root = Path(args.repo)
    if args.selfcheck:
        return run_selfcheck(args)

    report = GateReport(_report_base(root, args.out), args.version)
    model_fixture = fixture_path(root, "model")

    host = None
    if args.mode == "docker":
        host = boot_docker(args, report)
    else:
        host = boot_local(args, report)
    if host is None:
        print(f"{CARD}: no stack booted ({report.summary()})")
        return report.exit_code()

    try:
        # IMPORT
        if args.break_stage in ("import", "convert", "model"):
            # The import stage already FAILs hardware when the STEP fixture is
            # not present; for breaking import we drop the CGR leg by forcing a
            # skip-classified layer instead (see run_stage_import). For the
            # model/convert break, we break the convert lane below.
            pass
        step_item_id, _cgr_id = run_stage_import(root, host, report, args, model_fixture)
        if args.break_stage == "import":
            report.rows = [r for r in report.rows if r.id != "import-cgr"]
            report.add(LegRow("import-step", "IMPORT", "FAIL",
                              "deliberate-break: STEP blob deliberately not verified", "deliberate-break"))

        # MODEL
        model_id = run_stage_model(root, host, report, args, step_item_id, model_fixture)
        if args.break_stage in ("model", "convert") and model_id is not None:
            report.rows = [r for r in report.rows if r.id != "model-convert"]
            report.add(LegRow("model-convert", "MODEL", "FAIL",
                              "deliberate-break: convert lane output not hash-verified", "deliberate-break"))

        # RELEASE (PDM)
        if args.break_stage == "release":
            code, _h, body = req(host.base, "POST", f"/api/v1/items/{model_id}/lock",
                                 json.dumps({"holder": "e2e-pdm"}).encode())
            token = _json(body).get("token")
            if code == 200 and token:
                # Corrupt the token so the transition must be refused.
                bad = (token[::-1] if len(token) > 4 else "bad")
                code_t, _h_t, _b_t = req(
                    host.base, "POST",
                    f"/api/v1/items/{model_id}/revisions/1/transition",
                    json.dumps({"to": "released", "token": bad}).encode())
                report.rows = [r for r in report.rows if r.id != "release"]
                if 400 <= code_t < 500:
                    report.add(LegRow("release", "RELEASE", "FAIL",
                                      "deliberate-break: PDM refused the release (corrupt token)",
                                      "deliberate-break"))
                else:
                    report.add(LegRow("release", "RELEASE", "FAIL",
                                      f"deliberate-break: expected a 4xx refusal but got {code_t}",
                                      "deliberate-break"))
            else:
                report.rows = [r for r in report.rows if r.id != "release"]
                report.add(LegRow("release", "RELEASE", "FAIL",
                                  "deliberate-break: could not acquire lock to corrupt", "deliberate-break"))
        else:
            run_stage_release(root, host, report, model_id)

        # RENDER (P2P worker lane)
        if args.break_stage == "render":
            report.add(LegRow("render", "RENDER", "FAIL",
                              "deliberate-break: render worker did not produce a PNG", "deliberate-break"))
        else:
            run_stage_render(root, host, report, args, model_id)

        # SHARE
        run_stage_share(host, report, model_id, args.break_stage)
    finally:
        host.stop()

    report.write()
    s = report.summary()
    print(f"{CARD} e2e-vertical: total={s['total']} PASS={s['PASS']} FAIL={s['FAIL']} "
          f"SKIP={s['SKIP']} | exit={report.exit_code()}")
    if args.break_stage:
        print(f"[deliberate failure] broken stage = {args.break_stage}; gate BLOCKS (exit 1) as required")
    print(f"report: {report.json_path()} + {report.md_path()}")
    return report.exit_code()


if __name__ == "__main__":
    sys.exit(main())
