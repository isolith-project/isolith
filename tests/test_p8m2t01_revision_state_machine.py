"""P8-M2-T01 revision state machine: contract lockstep guard.

The wire contract (P8-M6/openapi-v1.yaml, v1) fixes the four revision states
``working / under_review / released / obsolete`` and a nullable ``maturity``
string. This test guards that the Rust state machine keeps those exact wire
names and the exact legal-hop set, so a silent rename or refactor cannot drift
the implementation from the contract without failing CI.
"""

import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[1]
STATE_FILE = ROOT / "crates" / "oep-pdm" / "src" / "state.rs"
SPEC_DOC = ROOT / "P8-M2" / "revision-state-machine.md"

EXPECTED_STATES = {"working", "under_review", "released", "obsolete"}
EXPECTED_MATURITY = {"preliminary", "detailed", "final"}
EXPECTED_LEGAL = {
    ("working", "under_review"),
    ("working", "released"),
    ("working", "obsolete"),
    ("under_review", "working"),
    ("under_review", "released"),
    ("under_review", "obsolete"),
    ("released", "obsolete"),
}


def snek(name: str) -> str:
    """CamelCase -> snake_case, e.g. UnderReview -> under_review."""
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def legal_hops_from_source(src: str):
    """Extract the (from, to) pairs named inside LEGAL_TRANSITIONS."""
    m = re.search(
        r"pub const LEGAL_TRANSITIONS[^=]*=\s*\[(.*?)\];", src, re.S
    )
    assert m, "LEGAL_TRANSITIONS table not found in state.rs"
    body = m.group(1)
    hops = set()
    for a, b in re.findall(
        r"RevisionState::(\w+)\s*,\s*RevisionState::(\w+)", body
    ):
        hops.add((snek(a), snek(b)))
    return hops


def test_revision_state_wire_names_match_contract():
    src = STATE_FILE.read_text()
    for name in EXPECTED_STATES:
        assert f'"{name}"' in src, f"missing wire state {name!r} in state.rs"


def test_maturity_wire_names_match_contract():
    src = STATE_FILE.read_text()
    for name in EXPECTED_MATURITY:
        assert f'"{name}"' in src, f"missing wire maturity {name!r}"


def test_legal_hops_match_contract_exactly():
    src = STATE_FILE.read_text()
    hops = legal_hops_from_source(src)
    assert hops == EXPECTED_LEGAL, (
        f"legal-hop table drifted from the contract: extra={hops - EXPECTED_LEGAL} "
        f"missing={EXPECTED_LEGAL - hops}"
    )


def test_spec_documents_the_same_states():
    md = SPEC_DOC.read_text() if SPEC_DOC.exists() else ""
    assert md, "missing P8-M2/revision-state-machine.md spec document"
    for name in EXPECTED_STATES:
        assert name in md, f"spec doc does not mention the {name!r} state"
    for name in EXPECTED_MATURITY:
        assert name in md, f"spec doc does not mention the {name!r} maturity"
