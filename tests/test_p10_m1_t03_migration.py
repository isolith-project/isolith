#!/usr/bin/env python3
"""P10-M1-T03 acceptance: the `oep-vault-migrate` CLI on a representative
V5-era folder tree, verified independently of the Rust implementation.

Pytest-collectable (runs under the precommit quality gate's `pytest`), stdlib
only. Every assertion is against REAL artifacts produced by the REAL binary:

  - dry-run (`plan`) prints the mapping and writes NOTHING;
  - real import (`import`) writes <vault>/vault.json + content-addressed blobs
    that preserve item+revision+file relationships per the P8-M1-T02 schema
    (folder tree -> nested folder Contexts, file -> item/revision/file with
    Origin provenance, one imports-ledger row);
  - every blob_sha256 is recomputed here from the source file bytes with
    hashlib and compared -- checksums verified against source;
  - re-running import is idempotent (vault.json byte-identical) and a new file
    in the tree is imported as the delta;
  - `verify` PASSes on a clean tree and exits non-zero on a tampered source
    file OR a tampered stored blob.

This is the independent companion to
crates/oep-pdm/tests/vault_migration_cli.rs (Rust, CARGO_BIN_EXE).
"""

import hashlib
import json
import os
import subprocess
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.path.join(REPO, "target", "debug", "oep-vault-migrate")
T0 = "2026-10-17T00:00:00Z"

# Representative V5-era legacy tree: fake-but-representative bytes, nested
# folders, an empty "Archive" folder that must still re-materialize as a
# folder Context (clean-room: metadata mapping only, no proprietary content).
TREE = {
    "Chassis/BajaChassis.CATProduct": b"fake catproduct: BajaChassis assembly\n",
    "Chassis/Frame/RearFrame.CATPart": b"fake catpart: rear frame geometry\n",
    "Chassis/Frame/SkidPlate.CATPart": b"fake catpart: skid plate geometry\n",
    "Docs/Driveline/DrivelineNotes.txt": b"fake txt: driveline notes\n",
    "Docs/Readme.txt": b"fake txt: build readme\n",
    "Misc/SuspensionInfo.pdf": b"%PDF-1.4 fake drawing\n",
}


def _binary():
    if not os.path.isfile(BIN):
        subprocess.run(
            ["cargo", "build", "-p", "oep-pdm", "--bin", "oep-vault-migrate"],
            cwd=REPO,
            check=True,
            capture_output=True,
        )
    return BIN


def _run(args):
    p = subprocess.run([_binary()] + args, capture_output=True, text=True)
    return p.returncode, p.stdout, p.stderr


def _make_tree(d):
    root = os.path.join(d, "Baja")
    for rel, data in TREE.items():
        full = os.path.join(root, rel)
        os.makedirs(os.path.dirname(full), exist_ok=True)
        with open(full, "wb") as fh:
            fh.write(data)
    # Empty folder with no files: still re-materializes as a folder Context.
    os.makedirs(os.path.join(root, "Archive"), exist_ok=True)
    return root


def _sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def _load_vault(vault):
    with open(os.path.join(vault, "vault.json"), "r", encoding="utf-8") as fh:
        return json.load(fh)


def test_plan_is_dry_run_and_writes_nothing():
    with tempfile.TemporaryDirectory() as d:
        root = _make_tree(d)
        vault = os.path.join(d, "vault-out")
        rc, out, _ = _run(["plan", root, "--vault", vault, "--collected-at", T0])
        assert rc == 0, out
        assert "dry-run" in out
        assert not os.path.exists(vault), "dry-run must not create the vault dir"
        # Every discovered entry appears in the printed mapping.
        for rel in TREE:
            assert rel in out


def test_import_preserves_schema_and_checksums_against_source():
    with tempfile.TemporaryDirectory() as d:
        root = _make_tree(d)
        vault = os.path.join(d, "vault-out")
        rc, _out, err = _run(["import", root, "--vault", vault, "--collected-at", T0])
        assert rc == 0, err

        doc = _load_vault(vault)
        assert doc["format"] == "oep-vault" and doc["version"] == 1

        # Folder tree re-materializes (root + 6 folders incl. empty Archive).
        by_id = {c["id"]: c for c in doc["contexts"]}
        assert len(doc["contexts"]) == 7, [c["name"] for c in doc["contexts"]]
        names = {c["name"]: c for c in doc["contexts"]}
        assert names["Baja"]["kind"] == "product"
        assert by_id[names["Driveline"]["parent_id"]]["name"] == "Docs", \
            "deep nesting preserved: Driveline under Docs"
        assert by_id[names["Frame"]["parent_id"]]["name"] == "Chassis"
        assert "Archive" in names, "empty folder still re-materializes as a Context"
        assert names["Archive"]["kind"] == "folder"

        # One item/revision/file per entry, with Origin provenance.
        assert len(doc["items"]) == len(TREE)
        assert len(doc["revisions"]) == len(TREE)
        assert len(doc["files"]) == len(TREE)
        for item in doc["items"]:
            origin = item["origin"]
            assert origin["source"] == "v5-folder-tree"
            assert origin["importer"] == "oep-vault-migrate/0.1"
            assert len(origin["sha256"]) == 64

        # Checksums verified against source: recompute every blob hash here.
        files = {f["filename"]: f for f in doc["files"]}
        for rel, data in TREE.items():
            leaf = rel.rsplit("/", 1)[1]
            f = files[leaf]
            assert f["blob_sha256"] == _sha256_file(os.path.join(root, rel))
            assert f["size"] == len(data)
            # Blob file on disk byte-matches source.
            with open(os.path.join(vault, "blobs", f["blob_sha256"]), "rb") as fh:
                assert fh.read() == data, "vault blob byte-matches source"

        # One imports-ledger row with every idempotency key.
        assert len(doc["imports"]) == 1
        assert len(doc["imports"][0]["keys"]) == len(TREE)


def test_reimport_is_idempotent_and_new_file_imports_as_delta():
    with tempfile.TemporaryDirectory() as d:
        root = _make_tree(d)
        vault = os.path.join(d, "vault-out")
        rc, _o, err = _run(["import", root, "--vault", vault, "--collected-at", T0])
        assert rc == 0, err
        before = open(os.path.join(vault, "vault.json"), "rb").read()

        rc, out, _ = _run(["import", root, "--vault", vault, "--collected-at", T0])
        assert rc == 0
        assert "nothing to import" in out
        after = open(os.path.join(vault, "vault.json"), "rb").read()
        assert before == after, "no-op re-run leaves vault.json byte-identical"

        # New file in the tree -> delta imported exactly once.
        new_rel = "Chassis/Frame/NewBracket.CATPart"
        with open(os.path.join(root, new_rel), "wb") as fh:
            fh.write(b"a brand new legacy part\n")
        rc, _o, err = _run(["import", root, "--vault", vault, "--collected-at", T0])
        assert rc == 0, err
        doc = _load_vault(vault)
        assert len(doc["items"]) == len(TREE) + 1
        assert len(doc["imports"]) == 2
        assert len(doc["imports"][0]["keys"]) == len(TREE)  # untouched first run
        assert len(doc["imports"][1]["keys"]) == 1

        rc, out, _ = _run(["import", root, "--vault", vault, "--collected-at", T0])
        assert rc == 0 and "nothing to import" in out


def test_verify_passes_and_detects_tamper_both_directions():
    with tempfile.TemporaryDirectory() as d:
        root = _make_tree(d)
        vault = os.path.join(d, "vault-out")
        rc, _o, err = _run(["import", root, "--vault", vault, "--collected-at", T0])
        assert rc == 0, err

        rc, out, _ = _run(["verify", root, "--vault", vault])
        assert rc == 0, out
        assert "verify PASS" in out

        # Tamper a SOURCE file -> verify fails.
        with open(os.path.join(root, "Docs/Readme.txt"), "ab") as fh:
            fh.write(b"TAMP")
        rc, _o, err = _run(["verify", root, "--vault", vault])
        assert rc != 0 and "verify failed" in err

        # Restore source; tamper the STORED blob -> verify fails (store scan).
        with open(os.path.join(root, "Docs/Readme.txt"), "wb") as fh:
            fh.write(TREE["Docs/Readme.txt"])
        doc = _load_vault(vault)
        readme = [f for f in doc["files"] if f["filename"] == "Readme.txt"][0]
        with open(os.path.join(vault, "blobs", readme["blob_sha256"]), "wb") as fh:
            fh.write(b"EVIL BYTES SWAPPED IN")
        rc, _o, err = _run(["verify", root, "--vault", vault])
        assert rc != 0 and "corrupt" in err


def test_cli_rejects_bad_usage_and_reports_version():
    rc, _o, err = _run(["import"])  # missing --vault and <root>
    assert rc != 0
    assert "missing" in err or "--vault" in err
    rc, out, _ = _run(["--version"])
    assert rc == 0 and out.startswith("oep-vault-migrate")
