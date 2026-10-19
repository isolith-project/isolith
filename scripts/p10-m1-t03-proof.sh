#!/usr/bin/env bash
# P10-M1-T03 live proof: build oep-vault-migrate and run the migration proof
# runner on a representative V5-era folder tree. Writes
# workstreams/P10-M1/proof/migration-proof.json and exits 0 iff every leg PASSes.
set -eu
cd "$(dirname "$0")/.."

echo "== building oep-vault-migrate =="
cargo build -p oep-pdm --bin oep-vault-migrate

echo "== running migration proof =="
python3 python/oep_tooling/vault_migration_proof.py
