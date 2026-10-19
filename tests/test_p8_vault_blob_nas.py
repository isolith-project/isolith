"""P8-M1-T02 acceptance (blob layer): vault blobs are content-addressed AND
verified by hash on the NAS object store, reusing the P1-M4-T01 mirror
pattern (`oep_tooling/vault_blobs.py`).

Local (no NAS) checks always run: blob_addresses() validates a vault blobs dir
-- every entry must be a 64-hex content address and its bytes must hash to the
address; a tampered blob or a foreign filename is surfaced, never mirrored.

Live-NAS checks -- mirror() every real fixture blob up content-addressed
(sha-256 object name) idempotently, then verify_mirror() GETs each blob back
and re-hashes it against its address -- are gated on probe(). When the store
is unreachable or the token is absent they classify as SKIP (exit 0), per the
tolerant-failure doctrine, but a LIVE PASS is performed on real material when
the NAS answers, exactly like P1-M4-T01's corpus mirror acceptance.

Pure stdlib. Runs under `python3 tests/test_p8_vault_blob_nas.py` and pytest.
Exit code: 0 if no FAIL; 1 if any FAIL (SKIP does not fail).
"""
import hashlib
import json
import os
import shutil
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(REPO, "python"))
FIXTURES = os.path.join(REPO, "crates", "oep-pdm", "tests", "fixtures", "legacy")

_failures = []
_skips = []


def check(name, ok, detail=""):
    print("[%s] %s -- %s" % ("PASS" if ok else "FAIL", name, detail))
    if not ok:
        _failures.append(name)


def check_skip(name, detail=""):
    print("[SKIP] %s -- %s" % (name, detail))
    _skips.append(name)


def _sha256_bytes(b):
    return hashlib.sha256(b).hexdigest()


def _build_vault_blobs_dir():
    """Copy the 6 fixture blobs into a `blobs/` dir named by sha-256 -- the
    layout the Rust `BlobStore` writes (blobs/<sha256>)."""
    tmp = tempfile.mkdtemp(prefix="oep-vault-blob-test-")
    blobs_dir = os.path.join(tmp, "blobs")
    os.makedirs(blobs_dir)
    for name in os.listdir(FIXTURES):
        data = open(os.path.join(FIXTURES, name), "rb").read()
        with open(os.path.join(blobs_dir, _sha256_bytes(data)), "wb") as fh:
            fh.write(data)
    return tmp, blobs_dir


def main():
    from oep_tooling import vault_blobs as vb

    # --- local acceptance: content-addressed blob dir validation ------------
    tmp, blobs_dir = _build_vault_blobs_dir()
    names = sorted(os.listdir(blobs_dir))
    check("fixture set has 6 content-addressed blobs", len(names) == 6,
          "found %d" % len(names))
    ok_rows = [row for row in vb.blob_addresses(blobs_dir) if row[1]]
    bad_rows = [row for row in vb.blob_addresses(blobs_dir) if not row[1]]
    check("every blob validates as a 64-hex address hashing to itself",
          len(ok_rows) == 6 and not bad_rows,
          "%d ok, %d bad" % (len(ok_rows), len(bad_rows)))

    # Tamper one blob: it must be surfaced as a bad row, never mirrored.
    victim = ok_rows[0][0]
    path = os.path.join(blobs_dir, victim)
    data = bytearray(open(path, "rb").read())
    data[0] ^= 0x01
    open(path, "wb").write(bytes(data))
    bad_after = [row for row in vb.blob_addresses(blobs_dir) if not row[1]]
    check("tampered blob is surfaced as invalid (verify-by-hash)",
          any(row[0] == victim for row in bad_after),
          "rows surfaced: %s" % [row[0] for row in bad_after])

    # Foreign non-address file in blobs/ is surfaced, never mirrored.
    with open(os.path.join(blobs_dir, "not-a-hash.bin"), "w") as fh:
        fh.write("junk")
    bad2 = [row for row in vb.blob_addresses(blobs_dir) if not row[1]]
    check("foreign blobs-dir file is surfaced",
          any(row[0] == "not-a-hash.bin" for row in bad2),
          "%d bad rows" % len(bad2))
    shutil.rmtree(tmp, ignore_errors=True)

    # --- live NAS acceptance: mirror + verify on real material --------------
    if not vb.probe():
        check_skip("live NAS mirror/verify",
                   "NAS object store unreachable (probe False) or token absent; "
                   "local content-addressing/hash checks above remain green")
        print("RESULT 0 pass, %d fail, %d skip" % (len(_failures), len(_skips)))
        return 0 if not _failures else 1

    tmp2, blobs_dir2 = _build_vault_blobs_dir()
    index_path = os.path.join(tmp2, "vault-blob-index.json")
    index, results = vb.mirror(blobs_dir2, index_path)
    uploaded = [r for r in results if r[1]]
    pending = [r for r in results if not r[1]]
    check("mirror uploads all content-addressed vault blobs",
          len(uploaded) == 6, "uploaded=%d pending=%s" % (len(uploaded), pending))

    with open(index_path) as fh:
        idx = json.load(fh)
    check("committed index records 6 uploaded blobs",
          sum(1 for e in idx["entries"] if e["status"] == "uploaded") == 6,
          "statuses=%s"
          % sorted(e["status"] for e in idx["entries"]))

    ok_count, verify_rows = vb.verify_mirror(index_path)
    check("GET-back re-hash verifies every mirrored blob (P1-M4-T01 check)",
          ok_count == 6, "%d/6 verified, bad=%s"
          % (ok_count, [r for r in verify_rows if not r[1]]))

    # Idempotent re-run: second mirror is all dedup, still 6 uploaded.
    index2, results2 = vb.mirror(blobs_dir2, index_path)
    check("re-mirror is idempotent (dedup, still uploaded)",
          len([r for r in results2 if r[1]]) == 6,
          "second run: %s" % [r[2] for r in results2])
    shutil.rmtree(tmp2, ignore_errors=True)

    print("RESULT %d pass, %d fail, %d skip" % (0, len(_failures), len(_skips)))
    return 0 if not _failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
