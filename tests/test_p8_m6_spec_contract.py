#!/usr/bin/env python3
"""P8-M6-T01 contract gate: P8-M6/openapi-v1.yaml IS the contract, and the
committed generated client (python/oep_pdm_client.py) must be the faithful
derivation of it.

Two enforcement layers:

1. DRIFT (always runs, stdlib-only, CI-safe): the generated client embeds
   `SPEC_SHA256`, the sha-256 of the exact P8-M6/openapi-v1.yaml bytes it was
   generated from. If anyone edits the spec without regenerating the client,
   this test fails: edit the contract, you regenerate the client. That is the
   OpenAPI spec as the single source of truth.

2. STRUCTURE (stdlib-only): the committed spec must still declare the v1
   surface the stub implements (base path, all paths/verbs/operationIds in the
   contract, the error envelope). A second test re-runs the full generator and
   diffs byte-for-byte when PyYAML is importable (author machine), so the
   check doesn't depend on a YAML parser being present under the CI gate's
   system python.
"""
import hashlib
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SPEC_PATH = os.path.join(ROOT, "P8-M6", "openapi-v1.yaml")
CLIENT_DIR = os.path.join(ROOT, "python")
GENERATOR = os.path.join(ROOT, "scripts", "gen_pdm_client.py")


def load_client():
    import importlib.util

    sys.path.insert(0, CLIENT_DIR)
    module_path = os.path.join(CLIENT_DIR, "oep_pdm_client.py")
    spec = importlib.util.spec_from_file_location("oep_pdm_client", module_path)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def spec_sha256() -> str:
    with open(SPEC_PATH, "rb") as fh:
        return hashlib.sha256(fh.read()).hexdigest()


def test_committed_client_is_in_sync_with_spec():
    """The contract is the spec; the client must drift with it."""
    client = load_client()
    assert client.SPEC_SHA256 == spec_sha256(), (
        "python/oep_pdm_client.py is out of sync with P8-M6/openapi-v1.yaml; "
        "run: python3 scripts/gen_pdm_client.py"
    )
    # Same for the wire identity the client hard-codes.
    assert client.API_BASE == "/api/v1"
    assert client.API_VERSION == "1.0.0"


def test_spec_declares_the_stub_contract_surface():
    """The committed spec must still carry every surface the stub serves."""
    text = open(SPEC_PATH, encoding="utf-8").read()
    assert "openapi: 3.0.3" in text
    assert "servers:" in text and "/api/v1" in text.split("servers:")[1].split("\n")[1]
    # Every path + operationId the P7-M2-T02 stub implemented remains declared.
    for op_id in [
        "health",
        "listItems",
        "createItem",
        "getItem",
        "openLatestFile",
        "listRevisions",
        "createRevision",
        "getRevision",
        "openRevisionFile",
    ]:
        assert "operationId: %s" % op_id in text, "missing operationId %s" % op_id
    for path in [
        "/health",
        "/items",
        "/items/{item_id}",
        "/items/{item_id}/file/latest",
        "/items/{item_id}/revisions",
        "/items/{item_id}/revisions/{rev}",
        "/items/{item_id}/revisions/{rev}/file",
    ]:
        assert path in text, "missing path %s" % path
    # Download semantics still declared (headers + octet-stream body).
    assert "X-Blob-Sha256" in text
    assert "application/octet-stream" in text
    # Error envelope still the one the stub speaks.
    assert "ErrorEnvelope" in text and 'code' in text and "message" in text


def test_spec_is_valid_openapi_contract():
    """Self-contained OpenAPI 3.0.3 conformance (scripts/validate-openapi.py)."""
    import importlib.util

    validator_path = os.path.join(ROOT, "scripts", "validate-openapi.py")
    spec_mod = importlib.util.spec_from_file_location("validate_openapi", validator_path)
    assert spec_mod and spec_mod.loader
    mod = importlib.util.module_from_spec(spec_mod)
    spec_mod.loader.exec_module(mod)
    errors = mod.validate()
    assert not errors, "openapi-v1.yaml invalid (%d): %s" % (len(errors), errors[:5])


def test_full_regeneration_diff():
    """Author-machine check: regenerating from the spec is byte-identical."""
    try:
        import yaml  # noqa: F401 - author-machine only
    except ImportError:
        # CI system python has no PyYAML; the sha drift test above already gates.
        import pytest
        pytest.skip("PyYAML not available; sha256 drift test is the CI gate")

    import importlib.util
    import yaml as _yaml

    spec_mod = importlib.util.spec_from_file_location("gen_pdm_client", GENERATOR)
    assert spec_mod and spec_mod.loader
    mod = importlib.util.module_from_spec(spec_mod)
    spec_mod.loader.exec_module(mod)

    with open(SPEC_PATH, encoding="utf-8") as fh:
        spec = _yaml.safe_load(fh)
    rendered = mod.generate(spec)
    with open(os.path.join(CLIENT_DIR, "oep_pdm_client.py"), encoding="utf-8") as fh:
        current = fh.read()
    assert current == rendered, (
        "full regeneration differs from committed client; "
        "run: python3 scripts/gen_pdm_client.py"
    )
