#!/usr/bin/env python3
"""P10-M1-T03 live proof runner: `oep-vault-migrate` end-to-end on a
representative V5-era folder tree, capturing every acceptance leg into
`P10-M1/proof/migration-proof.json`.

Runs the REAL compiled binary against real bytes in a temporary tree and
re-verifies the vault output independently (stdlib hashlib re-computes every
source blob hash; json re-parses the vault document). Every leg is recorded as
{leg, expected, actual, pass}; the summary records machine-readable facts
(counts, hashes, ledger rows, vault byte-stability over an idempotent re-run).

Exit 0 iff every leg passes; also writes the JSON proof before exiting.
Usage: python3 python/oep_tooling/vault_migration_proof.py
"""
import hashlib
import json
import os
import subprocess
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BIN = os.path.join(REPO, "target", "debug", "oep-vault-migrate")
PROOF = os.path.join(REPO, "P10-M1", "proof", "migration-proof.json")
T0 = "2026-10-17T00:00:00Z"

TREE = {
    "Chassis/BajaChassis.CATProduct": b"fake catproduct: BajaChassis assembly\n",
    "Chassis/Frame/RearFrame.CATPart": b"fake catpart: rear frame geometry\n",
    "Chassis/Frame/SkidPlate.CATPart": b"fake catpart: skid plate geometry\n",
    "Docs/Driveline/DrivelineNotes.txt": b"fake txt: driveline notes\n",
    "Docs/Readme.txt": b"fake txt: build readme\n",
    "Misc/SuspensionInfo.pdf": b"%PDF-1.4 fake drawing\n",
}


class Proof:
    def __init__(self):
        self.legs = []
        self.facts = {}

    def leg(self, name, expected, actual, ok):
        self.legs.append({"leg": name, "expected": expected, "actual": str(actual), "pass": bool(ok)})
        mark = "PASS" if ok else "FAIL"
        print("[%s] %s: %s" % (mark, name, actual))
        return ok

    def fact(self, key, value):
        self.facts[key] = value


def run(args):
    return subprocess.run([BIN] + args, capture_output=True, text=True)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def load_vault(vault):
    with open(os.path.join(vault, "vault.json"), "r", encoding="utf-8") as fh:
        return json.load(fh)


def main():
    proof = Proof()
    with tempfile.TemporaryDirectory(prefix="oep-migrate-proof-") as d:
        root = os.path.join(d, "Baja")
        for rel, data in TREE.items():
            full = os.path.join(root, rel)
            os.makedirs(os.path.dirname(full), exist_ok=True)
            with open(full, "wb") as fh:
                fh.write(data)
        os.makedirs(os.path.join(root, "Archive"), exist_ok=True)  # empty folder
        vault = os.path.join(d, "vault-out")

        # -- dry-run -----------------------------------------------------
        p = run(["plan", root, "--vault", vault, "--collected-at", T0])
        proof.leg("plan exit 0", 0, p.returncode, p.returncode == 0)
        proof.leg(
            "plan announces dry-run", "dry-run in stdout", "dry-run" in p.stdout,
            "dry-run" in p.stdout,
        )
        proof.leg(
            "dry-run writes nothing",
            "vault dir absent",
            os.path.exists(vault),
            not os.path.exists(vault),
        )

        # -- import --------------------------------------------------------
        p = run(["import", root, "--vault", vault, "--collected-at", T0])
        proof.leg("import exit 0", 0, p.returncode, p.returncode == 0)
        doc = load_vault(vault)
        proof.fact("contexts", len(doc["contexts"]))
        proof.fact("items", len(doc["items"]))
        proof.fact("revisions", len(doc["revisions"]))
        proof.fact("files", len(doc["files"]))
        proof.fact("import_rows", len(doc["imports"]))
        proof.leg(
            "one item per legacy file", len(TREE), len(doc["items"]),
            len(doc["items"]) == len(TREE),
        )
        proof.leg("header oep-vault v1", (doc["format"], doc["version"]),
                  ("oep-vault", 1), doc["format"] == "oep-vault" and doc["version"] == 1)

        by_id = {c["id"]: c for c in doc["contexts"]}
        names = {c["name"]: c for c in doc["contexts"]}
        driveline_parent = by_id[names["Driveline"]["parent_id"]]["name"]
        frame_parent = by_id[names["Frame"]["parent_id"]]["name"]
        proof.leg("deep nesting Driveline->Docs", "Docs", driveline_parent,
                  driveline_parent == "Docs")
        proof.leg("nested Frame->Chassis", "Chassis", frame_parent,
                  frame_parent == "Chassis")
        proof.leg("empty folder re-materializes", "folder", names["Archive"]["kind"],
                  names["Archive"]["kind"] == "folder")

        # Checksums verified against source (independent recompute).
        by_filename = {f["filename"]: f for f in doc["files"]}
        checks = []
        for rel, data in TREE.items():
            leaf = rel.rsplit("/", 1)[1]
            got = by_filename[leaf]["blob_sha256"]
            want = sha256_file(os.path.join(root, rel))
            checks.append((leaf, got == want))
        proof.leg(
            "every blob_sha256 == source sha-256",
            "True x%d" % len(TREE),
            checks,
            all(ok for _, ok in checks),
        )
        proof.fact("blob_addresses_match_source", all(ok for _, ok in checks))
        blob_bytes_ok = all(
            open(os.path.join(vault, "blobs", by_filename[rel.rsplit("/",1)[1]]["blob_sha256"]), "rb").read()
            == data
            for rel, data in TREE.items()
        )
        proof.leg("vault blob bytes == source bytes", True, blob_bytes_ok, blob_bytes_ok)

        # -- verify (clean) ------------------------------------------------
        p = run(["verify", root, "--vault", vault])
        proof.leg("verify exit 0 (clean)", 0, p.returncode, p.returncode == 0)
        n_pass = p.stdout.count("PASS")
        proof.fact("verify_pass_rows", n_pass)

        # -- idempotent re-run ---------------------------------------------
        before = open(os.path.join(vault, "vault.json"), "rb").read()
        p = run(["import", root, "--vault", vault, "--collected-at", T0])
        after = open(os.path.join(vault, "vault.json"), "rb").read()
        proof.leg("re-run is a no-op", "nothing to import", p.stdout.strip(),
                  "nothing to import" in p.stdout)
        proof.leg("re-run vault.json byte-identical", True, before == after,
                  before == after)

        # -- delta ---------------------------------------------------------
        with open(os.path.join(root, "Chassis/Frame/NewBracket.CATPart"), "wb") as fh:
            fh.write(b"a brand new legacy part\n")
        p = run(["import", root, "--vault", vault, "--collected-at", T0])
        doc2 = load_vault(vault)
        proof.leg("delta import exit 0", 0, p.returncode, p.returncode == 0)
        proof.leg(
            "delta imported once (7 items)", 7, len(doc2["items"]),
            len(doc2["items"]) == 7,
        )
        proof.leg(
            "one ledger row per run", 2, len(doc2["imports"]),
            len(doc2["imports"]) == 2,
        )
        proof.fact("import_rows_after_delta", len(doc2["imports"]))

        # -- tamper detection ------------------------------------------------
        with open(os.path.join(root, "Docs/Readme.txt"), "ab") as fh:
            fh.write(b"TAMP")
        p = run(["verify", root, "--vault", vault])
        proof.leg(
            "verify fails on tampered source", "non-zero", p.returncode,
            p.returncode != 0,
        )
        with open(os.path.join(root, "Docs/Readme.txt"), "wb") as fh:
            fh.write(TREE["Docs/Readme.txt"])
        readme = [f for f in doc2["files"] if f["filename"] == "Readme.txt"][0]
        with open(os.path.join(vault, "blobs", readme["blob_sha256"]), "wb") as fh:
            fh.write(b"EVIL BYTES SWAPPED IN")
        p = run(["verify", root, "--vault", vault])
        proof.leg(
            "verify fails on tampered stored blob", "non-zero", p.returncode,
            p.returncode != 0,
        )

    # -- emit proof json ---------------------------------------------------
    all_ok = all(leg["pass"] for leg in proof.legs)
    import datetime

    # timezone-aware UTC timestamp (utcnow() is deprecated in modern CPython).
    run_at = datetime.datetime.now(datetime.timezone.utc).isoformat().replace("+00:00", "Z")
    record = {
        "card": "P10-M1-T03",
        "tool": "oep-vault-migrate",
        "run_at": run_at,
        "tree": "representative V5-era folder tree (fake bytes: CATPart/CATProduct/txt/pdf + empty Archive folder)",
        "result": "PASS" if all_ok else "FAIL",
        "facts": proof.facts,
        "legs": proof.legs,
    }
    os.makedirs(os.path.dirname(PROOF), exist_ok=True)
    with open(PROOF, "w", encoding="utf-8") as fh:
        json.dump(record, fh, indent=2)
        fh.write("\n")
    print()
    print("RESULT %s (%d %s)" % (
        ("PASS" if all_ok else "FAIL"),
        len(proof.legs),
        "legs",
    ))
    print("proof written:", PROOF)
    return 0 if all_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
