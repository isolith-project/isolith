#!/usr/bin/env python3
"""P9-M3-T02 contract test: the committed batch convert proof is internally
consistent and replayable from the byte artifacts.

Pure stdlib + hashlib. Verifies the four committed proof files without
re-running Rust:
  1. batch-report.json  -- schema, summary counts, per-record invariants:
     every 'ok' job has vault_blob_sha256 == artifact_sha256 (hash-verified),
     every 'skip' carries a reason, nothing 'error'.
  2. vault-snapshot.json -- the vault read-back agrees with the report:
     each ok job's output item/file content address equals its artifact sha.
  3. The sample artifacts the run downloaded (stc06.glb, box-ref.oepmesh.json)
     re-hash to exactly the report's recorded artifact_sha256 values
     (determinism replay on the bytes).
  4. batch-corpus.json -- every job is a valid oep-job convert spec with a
     registered worker function.

Run: python3 tests/test_p9_m3_export_workers.py   (or pytest).
"""
import hashlib
import json
import os
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROOF = os.path.join(REPO, "docs", "compute", "workers", "proof")

failures = []


def check(name, ok, detail=""):
    marker = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (marker, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        failures.append(name)


def load(name):
    with open(os.path.join(PROOF, name), encoding="utf-8") as fh:
        return json.load(fh)


def sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def main():
    return _run()


def _run():
    report = load("batch-report.json")
    check("report.format", report.get("format") == "oep-batch-report")
    check("report.version", report.get("version") == 1)

    summary = report.get("summary", {})
    jobs = report.get("jobs", [])
    check("report has jobs", len(jobs) == summary.get("total") == 8,
          "total=%s jobs=%s" % (summary.get("total"), len(jobs)))

    # The run we committed produced 6 ok / 2 skip / 0 error.
    by_status = {}
    for j in jobs:
        by_status[j["status"]] = by_status.get(j["status"], 0) + 1
    check("summary counts match records",
          by_status.get("ok") == summary.get("ok") == 6,
          "ok=%s skip=%s error=%s" % (by_status.get("ok"), by_status.get("skip"),
                                      by_status.get("error")))
    check("no errors", summary.get("failed") == 0)

    # Hash-verified: every ok job's vault content address == artifact sha.
    ok_jobs = [j for j in jobs if j["status"] == "ok"]
    check("hash_verified equals ok", summary.get("hash_verified") == len(ok_jobs) == 6)
    for j in ok_jobs:
        check("ok job %s vault==artifact" % j["job_name"],
              j["vault_blob_sha256"] == j["artifact_sha256"] and len(j["artifact_sha256"]) == 64,
              j["artifact_sha256"][:16])
        check("ok job %s has output revision" % j["job_name"], bool(j["output_revision"]))
    for j in jobs:
        if j["status"] == "skip":
            check("skip job %s has note" % j["job_name"], bool(j["note"]))

    # Vault snapshot read-back agrees with the report outputs.
    snap = load("vault-snapshot.json")
    check("snapshot.format", snap.get("format") == "oep-vault-snapshot")
    by_name = {}
    for e in snap["entries"]:
        by_name.setdefault(e["name"], []).extend(f["blob_sha256"] for f in e["files"])
    for j in ok_jobs:
        blob = by_name.get(j["output_item"], [])
        check("snapshot has %s" % j["output_item"], j["vault_blob_sha256"] in blob,
              "report %s" % j["vault_blob_sha256"][:16])

    # Byte-level determinism replay of the two sample artifacts.
    glb = sha256(os.path.join(PROOF, "stc06.glb"))
    cgr = next(j for j in ok_jobs if j["executor_function"] == "cgr2gltf")
    check("stc06.glb re-hashes to report", glb == cgr["artifact_sha256"],
          glb[:16])
    mesh = sha256(os.path.join(PROOF, "box-ref.oepmesh.json"))
    step = next(j for j in ok_jobs if j["job_name"] == "step-box-ref")
    check("box-ref.oepmesh.json re-hashes to report", mesh == step["artifact_sha256"],
          mesh[:16])

    # Batch manifest: every job is a valid oep-job convert with a registered fn.
    batch = load("batch-corpus.json")
    check("batch.format", batch.get("format") == "oep-batch")
    registered = {"cgr2gltf", "step2mesh"}
    for job in batch.get("jobs", []):
        check("job %s valid oep-job" % job["name"], job.get("format") == "oep-job")
        ex = job.get("executor", {})
        check("job %s convert worker" % job["name"],
              ex.get("kind") == "convert" and ex.get("function") in registered,
              repr(ex))
        check("job %s has input+output" % job["name"],
              len(job.get("input", [])) == 1 and len(job.get("output", [])) == 1)

    print("=" * 60)
    if failures:
        print("FAILED %d check(s): %s" % (len(failures), ", ".join(failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())


def test_committed_batch_proof_is_hash_consistent():
    """Pytest-collected gate: the committed P9-M3-T02 proof must replay."""
    assert _run() == 0, "committed P9-M3-T02 proof failed consistency checks"
