#!/usr/bin/env python3
"""Acceptance test for Isolith P8-M1-T01: oep-pdm web-stack decision.

Written FIRST (TDD), mirroring the renderer-backend ADR test convention. The
decision record at workstreams/pdm/service/web-stack-adr.md must make this test pass. It
verifies the real material, not mocks:
  decision      => Rust axum as the oep-pdm service web stack (default per
                   card P8-M1-T01; FastAPI weighed and rejected for core with
                   reasons, kept as tooling language)
  license       => AGPL-3.0 for the PDM server (program decision #3); axum
                   (MIT OR Apache-2.0) assessed as license-clean under it
  contract      => the workstreams/pdm/api/openapi-v1.yaml contract (inherited from
                   P7-M2-T02) is what the skeleton must satisfy; the stub
                   becomes the contract test, served by the real service
  real material => the committed axum service skeleton (service.rs + the
                   oep-pdm binary), the pinned axum dependency in the crate
                   manifest, and the recorded health proof
                   (workstreams/pdm/service/proof/health.json) captured from a live run
  guardrails    => version pin, escape hatch, contract test gate, library
                   surface-language rule (no em dashes, no curly quotes)

Run:  python3 tests/test_p8_m1_web_stack_adr.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import json
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ADR = os.path.join(ROOT, "workstreams/pdm/service", "web-stack-adr.md")
SPEC = os.path.join(ROOT, "workstreams/pdm/api", "openapi-v1.yaml")
SERVICE_SRC = os.path.join(ROOT, "crates", "oep-pdm", "src", "service.rs")
BIN_SRC = os.path.join(ROOT, "crates", "oep-pdm", "src", "bin", "oep-pdm.rs")
CARGO = os.path.join(ROOT, "crates", "oep-pdm", "Cargo.toml")
PROOF = os.path.join(ROOT, "workstreams/pdm/service", "proof", "health.json")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_adr() -> str:
    with open(ADR, encoding="utf-8") as fh:
        return fh.read()


def split_front_matter(text):
    """Return (meta_dict, body) for simple top-level YAML front matter."""
    if not text.startswith("---"):
        return {}, text
    _, fm, body = text.split("---", 2)
    meta = {}
    for line in fm.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or ":" not in stripped:
            continue
        key, _, val = stripped.partition(":")
        meta[key.strip()] = val.strip()
    return meta, body


def section_text(body, heading):
    """Text under a '## [N.] Heading', before the next heading of equal/higher
    level. Tolerates the numbered-heading style used by earlier records."""
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##? ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def run_checks():
    print("P8-M1-T01 web-stack decision acceptance")
    print("adr: %s" % ADR)
    print("=" * 66)

    # 1. The ADR exists.
    check(os.path.isfile(ADR), "missing: workstreams/pdm/service/web-stack-adr.md")
    if not os.path.isfile(ADR):
        return failures

    text = read_adr()
    meta, body = split_front_matter(text)
    low = text.lower()

    # 2. Front matter is present and versioned (living-document discipline).
    check("version" in meta, "front matter must carry a version")
    if "version" in meta:
        check(SEMVER.match(meta["version"]),
              "version must be semver X.Y.Z, got %r" % meta.get("version"))
    check("status" in meta and meta["status"].strip(),
          "front matter must carry a status")
    check("date" in meta and meta["date"].strip(),
          "front matter must carry a date")
    check("owner" in meta and meta["owner"].strip(),
          "front matter must carry an owner")
    check("decision" in meta and meta["decision"].strip(),
          "front matter must carry a decision key")
    check("applies_to" in meta and meta["applies_to"].strip(),
          "front matter must carry applies_to")

    # 3. The decision names Rust axum as the adopted web stack.
    check("axum" in low, "must name axum explicitly (the adopted stack)")
    check("rust" in low, "must be scoped to the Rust core lane")
    check("web stack" in low or "framework" in low or "server" in low,
          "must frame it as a web-stack/HTTP framework decision")

    # 4. FastAPI was weighed and rejected for the server core, with reasons.
    opt = section_text(body, "Options considered")
    oml = opt.lower()
    check(bool(oml), "must have an 'Options considered' section")
    if oml:
        check("fastapi" in oml,
              "options must weigh Python FastAPI explicitly")
        check("reject" in oml or "defer" in oml or "not adopt" in oml,
              "alternatives must be rejected or deferred with a reason")
        check("tooling" in oml or "client" in oml,
              "must note Python stays allowed for tooling/contract clients")

    # 5. The why: Rust-first doctrine, shared serde contract types, self-hosted.
    why = section_text(body, "Why this decision")
    wl = why.lower()
    check(bool(why), "must have a 'Why this decision' section")
    if wl:
        check("rust-first" in wl or "rust first" in wl or "invariant" in wl,
              "why-section must tie the choice to the Rust-first decision")
        check("api.rs" in wl or "serde" in wl or "contract types" in wl
              or "store" in wl,
              "why-section must note the existing Rust contract types are reused")
        check("license" in wl or "agpl" in wl,
              "why-section must address the AGPL-3.0 license posture")

    # 6. AGPL-3.0 license posture for the PDM server (program decision #3).
    lic = section_text(body, "AGPL-3.0 license posture") or section_text(body, "License")
    ll = lic.lower()
    check(bool(lic), "must have a license posture section")
    if ll:
        check("agpl" in ll, "license section must name AGPL-3.0")
        check("server" in ll or "pdm" in ll or "apache" in ll,
              "license section must tie to the hybrid server/PDM posture")
        check("axum" in ll or "mit" in ll or "permissive" in ll,
              "license section must assess axum's license under AGPL")

    # 7. Contract: the P8-M6 openapi v1 spec is the surface to satisfy and the
    #    stub becomes the contract test, not a throwaway.
    cq = section_text(body, "Contract and queue")
    cl = cq.lower()
    check(bool(cq), "must have a 'Contract and queue' section")
    if cl:
        check("openapi-v1" in cl or "openapi" in cl or "contract" in cl,
              "contract section must name the OpenAPI v1 contract")
        check("test" in cl, "contract section must keep the stub as a test")
        check("pdm_contract_acceptance" in cl or "test_p8_pdm_stub_contract" in cl
              or "tests/" in cl,
              "contract section must name the retained contract suites")
        check("no rewrite" in cl or "unchanged" in cl or "same base url" in cl,
              "contract section must state the client stays unchanged")

    # 8. Guardrails: version pin, escape hatch, CI test gate.
    grd = section_text(body, "Guardrails")
    gl = grd.lower()
    check(bool(gl), "must have a 'Guardrails' section")
    if gl:
        check("pin" in gl or "version" in gl,
              "guardrails must pin the axum/tokio version")
        check("escape" in gl or "swap" in gl or "replace" in gl
              or "actix" in gl or "hyper" in gl,
              "guardrails must keep an escape hatch")
        check("test" in gl or "ci" in gl or "acceptance" in gl,
              "guardrails must require a contract test gate")

    # 9. Real material: the committed service skeleton uses axum and has a
    #    single binary, the manifest pins axum, and the health proof exists.
    svc_ok = os.path.isfile(SERVICE_SRC)
    check(svc_ok, "missing service skeleton: crates/oep-pdm/src/service.rs")
    if svc_ok:
        svc = open(SERVICE_SRC, encoding="utf-8").read()
        slow = svc.lower()
        check("axum" in slow, "service skeleton must use axum")
        check("router" in slow, "service skeleton must build an axum Router")
        check("health" in slow, "service skeleton must serve the health route")
        check("items" in slow, "service skeleton must serve the items routes")
        check("revision" in slow, "service skeleton must serve the revision routes")
        check("store" in slow, "service skeleton must call into the vault store")
    check(os.path.isfile(BIN_SRC),
          "missing binary entrypoint: crates/oep-pdm/src/bin/oep-pdm.rs")
    check(os.path.isfile(CARGO), "missing crate manifest")
    if os.path.isfile(CARGO):
        cargo = open(CARGO, encoding="utf-8").read()
        check("axum" in cargo and "=" in cargo and '"' in cargo,
              "crate manifest must declare a pinned axum dependency")
        check("tokio" in cargo, "crate manifest must declare tokio")
    check(os.path.isfile(SPEC),
          "missing contract spec: workstreams/pdm/api/openapi-v1.yaml")
    if os.path.isfile(SPEC):
        spec = open(SPEC, encoding="utf-8").read()
        check("openapi: 3" in spec, "contract spec must be OpenAPI 3.x")
        check("paths:" in spec and "/health" in spec,
              "contract spec must define the health path")

    # 10. Health proof: real-material record of a live GET /api/v1/health.
    proof_ok = os.path.isfile(PROOF)
    check(proof_ok, "missing health proof: workstreams/pdm/service/proof/health.json")
    if proof_ok:
        try:
            with open(PROOF, encoding="utf-8") as fh:
                health = json.load(fh)
        except ValueError as e:
            check(False, "health proof is not valid JSON: %s" % e)
        else:
            check(health.get("status") == "ok",
                  "health proof must report status ok, got %r" % health)
            check(health.get("service") == "oep-pdm",
                  "health proof must report the real service name oep-pdm, "
                  "got %r" % health.get("service"))
            check(bool(health.get("version")),
                  "health proof must report a contract version")

    # 11. Cross-references: card + lane anchors.
    check("P8-M1-T01" in text, "must cite its own card P8-M1-T01")
    check("workstreams/pdm/api" in text, "must tie to the REST API v1 milestone P8-M6")
    check("workstreams/modeler/interchange" in text or "P7-M2-T02" in text,
          "must reference the inherited stub card P7-M2-T02")
    check("oep-pdm" in low, "must name the service crate oep-pdm")

    # 12. Surface language rule: no em dashes, no curly quotes (program
    #     discipline, see the P0-M2 decisions).
    ndashes = text.count(EM_DASH)
    check(ndashes == 0,
          "document contains %d em dash(es); ban applies to P8 prose too"
          % ndashes)
    ncurly = (text.count("\u201c") + text.count("\u201d")
              + text.count("\u2018") + text.count("\u2019"))
    check(ncurly == 0,
          "document contains %d curly quote(s); plain ASCII quotes only"
          % ncurly)

    return failures


def main():
    failures = run_checks()
    if failures:
        print("P8-M1 WEB-STACK ADR ACCEPTANCE FAILED (%d)" % len(failures))
        for f in failures:
            print("  -", f)
        sys.exit(1)
    print("P8-M1 WEB-STACK ADR ACCEPTANCE PASSED: "
          "Rust axum for oep-pdm, FastAPI weighed and rejected for core, "
          "AGPL-3.0 posture recorded, contract + guardrails + health proof "
          "verified on real material.")
    return 0


def test_p8_m1_web_stack_adr_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    failures = run_checks()
    assert not failures, "P8-M1 web-stack ADR acceptance failed (%d): %s" % (
        len(failures), failures[:5])


if __name__ == "__main__":
    sys.exit(main())
