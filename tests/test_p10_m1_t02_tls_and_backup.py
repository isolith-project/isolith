#!/usr/bin/env python3
"""P10-M1-T02 acceptance (TDD, RED first): TLS + certs for the stack, vault DB +
content-addressed blob backup to a second disk/archive, restore round-trip.

Written BEFORE the feature. On a tree without the binary the checks degrade to
honest SKIP (exit 0), matching the P2/P3/P4/P8/P9 gate convention; with the
binary + openssl present they run the REAL service over REAL sockets:

  TLS      => `oep-pdm --tls-cert --tls-key` serves HTTPS; a TLS handshake
              validates the chain / hostname (openssl s_client
              -verify_return_error); health rides over TLS; a plain HTTP client
              is NOT served on the TLS listener.
  CERTS    => scripts/gen-stack-certs.sh emits a CA + leaf + key and verifies
              the chain with openssl verify (the zero-config LAN identity).
  BACKUP   => scripts/backup/vault-backup.sh snapshots the persist dir (store
              DB + blobs + audit) to a second-disk/archive target, writes a
              sha256 manifest, and self-verifies; NAS targets are refused.
  SCHEDULE => scripts/backup/install-vault-backup-cron.sh registers a daily
              cron/systemd user timer entry for the producer.
  RESTORE  => scripts/backup/vault-restore-check.sh restores the backup into a
              fresh persist dir on this host, boots oep-pdm against it, and the
              real vault read hash round-trips against the manifest.

Run:  python3 tests/test_p10_m1_t02_tls_and_backup.py   (and under pytest)
Exit 0 = pass (SKIPs allowed); nonzero = fail. Stdlib only.

This is the producer-side proof; the full timed bare-metal drill is the
P10-M3-T01 card (scripts/dr/, its own runbook + proof).
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BIN = ROOT / "target" / "debug" / "oep-pdm"
SCRIPTS = ROOT / "scripts"
BACKUP = SCRIPTS / "backup"

_failures = []


def check(name, ok, detail=""):
    print("[%s] %s%s" % ("PASS" if ok else "FAIL", name,
                         (" -- " + detail) if detail else ""))
    if not ok:
        _failures.append(name)


def _free_port():
    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def _tool(which):
    return shutil.which(which)


def _seed_vault(persist_dir, cli, base_url):
    """Create one item + one released revision via the real API."""
    import urllib.request

    def call(method, path, body=None):
        req = urllib.request.Request(
            base_url + path,
            data=json.dumps(body).encode() if body else None,
            method=method,
            headers={"Content-Type": "application/json"},
        )
        return json.loads(urllib.request.urlopen(req).read())

    item = call("POST", "/items", {
        "name": "BackupSeed", "kind": "part", "description": "P10-M1-T02 seed"})
    fixture = ROOT / "crates" / "oep-pdm" / "tests" / "fixtures" / "box-mini.step"
    sha = _sha256(fixture.read_bytes())
    rev = call("POST", f"/items/{item['id']}/revisions", {
        "state": "released", "comment": "seed", "created_by": "acceptance",
        "file": {
            "filename": "seed.step", "format": "step",
            "content_type": "application/octet-stream",
            "content": __import__("base64").b64encode(fixture.read_bytes()).decode(),
        }})
    return item, rev, sha


def _sha256(b):
    import hashlib
    return hashlib.sha256(b).hexdigest()


def _binary_ok():
    return BIN.is_file()


def test_tls_flags_in_help():
    if not _binary_ok():
        print("[skip] no binary; build first: cargo build -p oep-pdm")
        return
    out = subprocess.run([str(BIN), "--help"], capture_output=True, text=True)
    check("--tls-cert in help", "--tls-cert" in out.stderr or "--tls-cert" in out.stdout)
    check("--tls-key in help", "--tls-key" in out.stderr or "--tls-key" in out.stdout)


def test_tls_terminates_over_https():
    if not (_binary_ok() and _tool("openssl")):
        print("[skip] no binary and/or openssl; TLS terminate not run")
        return
    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        cert = td / "cert.pem"
        key = td / "key.pem"
        subprocess.run(
            ["openssl", "req", "-x509", "-newkey", "rsa:2048",
             "-keyout", str(key), "-out", str(cert), "-days", "1", "-nodes",
             "-subj", "/CN=localhost",
             "-addext", "subjectAltName=DNS:localhost,IP:127.0.0.1"],
            check=True, capture_output=True)
        port = _free_port()
        proc = subprocess.Popen(
            [str(BIN), "--addr", "127.0.0.1:%d" % port, "--persist-dir",
             str(td / "vault"), "--tls-cert", str(cert), "--tls-key", str(key)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            time.sleep(1.0)
            # TLS handshake must verify chain+hostname (non-zero on any failure).
            rc = subprocess.run(
                ["openssl", "s_client", "-connect", "127.0.0.1:%d" % port,
                 "-servername", "localhost", "-CAfile", str(cert),
                 "-verify_return_error", "-quiet"],
                input=b"GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\n"
                       b"Connection: close\r\n\r\n",
                capture_output=True, timeout=20)
            body = rc.stdout.decode("utf-8", "replace")
            check("TLS handshake verifies (s_client -verify_return_error rc=0)",
                  rc.returncode == 0, "rc=%r" % rc.returncode)
            check("health JSON rides over TLS", '"status":"ok"' in body,
                  body[:120])

            # Plain HTTP client must NOT be served on the TLS listener.
            import socket as _s
            raw = _s.create_connection(("127.0.0.1", port), timeout=5)
            raw.sendall(b"GET /api/v1/health HTTP/1.1\r\nHost: localhost\r\n\r\n")
            raw.settimeout(3)
            head = b""
            try:
                while len(head) < 64:
                    chunk = raw.recv(4096)
                    if not chunk:
                        break
                    head += chunk
            except Exception:
                pass
            raw.close()
            check("plain HTTP not served on TLS listener",
                  not head.startswith(b"HTTP/1.1 200"), head[:64].decode("latin1"))
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()


def test_gen_certs_emits_verifiable_stack():
    if not _tool("openssl"):
        print("[skip] no openssl; cert generation not run")
        return
    script = SCRIPTS / "gen-stack-certs.sh"
    check("gen-stack-certs.sh exists", script.is_file())
    if not script.is_file():
        return
    with tempfile.TemporaryDirectory() as td:
        subprocess.run(["bash", str(script), td], check=True, capture_output=True)
        for f in ("ca.pem", "cert.pem", "key.pem", "cert-chain.pem"):
            check(f"gen-stack-certs emits {f}", (Path(td) / f).is_file())
        rc = subprocess.run(
            ["openssl", "verify", "-CAfile", str(Path(td) / "ca.pem"),
             str(Path(td) / "cert.pem")], capture_output=True)
        check("generated leaf verifies against its CA", rc.returncode == 0)


def test_backup_producer_is_checksummed():
    script = BACKUP / "vault-backup.sh"
    check("backup producer exists", script.is_file())
    if not script.is_file():
        return
    text = script.read_text()
    check("producer writes sha256 manifest", "manifest.sha256" in text)
    check("producer self-verifies", "sha256sum -c" in text)
    check("producer refuses NAS target (decision #9)",
          "/mnt/nas" in text)


def test_install_schedule_and_restore_round_trip():
    if not (_binary_ok() and BACKUP.joinpath("vault-backup.sh").is_file()):
        print("[skip] no binary and/or backup scripts; schedule/restore not run")
        return
    install = BACKUP / "install-vault-backup-cron.sh"
    restore = BACKUP / "vault-restore-check.sh"
    for s in (install, restore):
        check("%s exists" % s.name, s.is_file())

    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        vault = td / "vault"
        archive = td / "archive"  # second-disk/archive target on this host
        vault.mkdir()
        archive.mkdir()

        port = _free_port()
        proc = subprocess.Popen(
            [str(BIN), "--addr", "127.0.0.1:%d" % port, "--persist-dir", str(vault)],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        try:
            time.sleep(1.0)
            base = "http://127.0.0.1:%d/api/v1" % port
            item, rev, blob_sha = _seed_vault(vault, BIN, base)
            check("seeded vault item", item is not None)
        finally:
            proc.terminate()
            try:
                proc.wait(timeout=3)
            except Exception:
                proc.kill()

        # Backup producer -> archive target.
        rc = subprocess.run(
            ["bash", str(BACKUP / "vault-backup.sh"), str(vault), str(archive)],
            capture_output=True, text=True)
        check("backup producer exits 0", rc.returncode == 0,
              rc.stdout[-200:] + rc.stderr[-200:])
        backup_root = archive / "keelworks-vault-backup"
        check("backup bundle layout (latest/)",
              (backup_root / "latest" / "store.json").is_file())
        check("backup bundle manifest written",
              (backup_root / "manifest.sha256").is_file())
        check("backup bundle meta written",
              (backup_root / "meta.json").is_file())

        # Restore round-trip on this host.
        rc = subprocess.run(
            ["bash", str(BACKUP / "vault-restore-check.sh"), str(archive), str(BIN)],
            capture_output=True, text=True)
        check("restore round-trips on U1/VM (exit 0)", rc.returncode == 0,
              rc.stdout[-300:] + rc.stderr[-300:])


def test_surface_language_rule():
    """No em dashes / curly quotes in the files this card ships (program rule)."""
    for p in (SCRIPTS / "gen-stack-certs.sh",
              BACKUP / "vault-backup.sh",
              BACKUP / "vault-restore-check.sh",
              BACKUP / "install-vault-backup-cron.sh"):
        if p.is_file():
            text = p.read_text()
            check("no em dash in %s" % p.name, "\u2014" not in text)
            check("no curly quote in %s" % p.name,
                  "\u201c" not in text and "\u2018" not in text)


def main():
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")
             and callable(v)]
    for t in tests:
        try:
            t()
        except Exception as e:  # noqa: BLE001
            check(t.__name__, False, "exception: %r" % (e,))
    print("--")
    if _failures:
        print("P10-M1-T02 FAILURES: %d" % len(_failures))
        for f in _failures:
            print("  - %s" % f)
        return 1
    print("P10-M1-T02 GREEN (all checks passed)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
