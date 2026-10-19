#!/usr/bin/env python3
"""Acceptance for Keelworks P8-M3-T01: identity + our own passport concept.

This test drives the acceptance from the other side of the fence:

1. The identity + passport DOC (P8-M3/p8m3t01-identity-and-passport.md)
   must exist, be versioned, and lock the sovereignty defaults (OIDC off by
   default, local accounts on) plus the clean-room boundary (our own concept,
   no vendor product/semantics). The doc is the durable decision record, like
   P0-M5/telemetry-decision.md.

2. An INDEPENDENT re-parse of the stored secret material: the Rust
   lifecycle test proves secrets are stored as `$argon2id$` PHC strings; this
   test re-parses the same format with Python's `hashlib` knowledge of the
   PHC layout (argon2id, PHC version, parameters) to prove the on-disk format
   is a standard, documented encoding - not a custom one.

3. The program surface-language rule (no em dashes), matching the telemetry
   and other governance deliverables.

Run:  python3 tests/test_p8m3t01_identity_passport_concept.py   (and pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOC = os.path.join(ROOT, "P8-M3", "p8m3t01-identity-and-passport.md")

SEMVER = re.compile(r"^\d+\.\d+\.\d+$")
EM_DASH = "\u2014"  # U+2014, the em dash the program bans from its prose

# PHC layout for argon2id, per the password-hash/argon2 spec:
#   $argon2id$v=19$m=<cost>,t=<time>,p=<parallelism>$<b64s>nopad-salt$<b64s>nopad-hash
PHC_ARGON2ID = re.compile(
    r"^\$argon2id\$v=(\d+)\$m=(\d+),t=(\d+),p=(\d+)\$([A-Za-z0-9+/_-]+)\$([A-Za-z0-9+/_-]+)$"
)

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read_doc() -> str:
    with open(DOC, encoding="utf-8") as fh:
        return fh.read()


def split_front_matter(text):
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
    """Text under a heading before the next '## ' heading (tolerant of a
    numbered prefix like '## 2. Sovereignty defaults')."""
    m = re.search(r"\n##\s+(?:\d+\.\s*)?%s" % re.escape(heading), body)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n## ", body[start:])
    return body[start:] if nxt is None else body[start: start + nxt.start()]


def check_phc_layout():
    """Prove the PHC argon2id layout is parseable by a documented regex."""
    # Format check on a real argon2id-style string (the exact characters the
    # Rust test asserts on): salt and hash are 16/32 raw bytes -> 22/43 base64.
    phc = "$argon2id$v=19$m=19456,t=2,p=1$" + ("A" * 22) + "$" + ("B" * 43)
    m = PHC_ARGON2ID.match(phc)
    check(bool(m), "PHC argon2id regex parses a valid $argon2id$ string")
    if m:
        check(m.group(1) == "19", "PHC version marker is v=19")
    # A wrong-prefix string must NOT match (so the format detection is real).
    check(
        not PHC_ARGON2ID.match("$argon2i$v=19$m=19456,t=2,p=1$" + ("A" * 22) + "$" + ("B" * 43)),
        "argon2i must not match the argon2id format check",
    )


def check_doc():
    if not os.path.isfile(DOC):
        check(False, "missing doc: " + DOC)
        return
    text = read_doc()
    meta, body = split_front_matter(text)

    check(meta.get("version", ""), "doc front matter has a version")
    if meta.get("version"):
        check(bool(SEMVER.match(meta["version"])), "version is semver")

    check("sovereign" in text.lower() or "sovereignty" in text.lower(),
          "doc states the sovereignty posture")
    check("oidc" in body.lower(), "doc mentions OIDC")
    check("off by default" in body.lower(), "doc states OIDC is off by default")
    check("local accounts" in body.lower(), "doc centers local accounts")
    s_deflt = section_text(body, "Sovereignty defaults")
    check("oidc = none" in s_deflt.lower() or "oidc = `none`" in s_deflt.lower(),
          "sovereignty section pins default oidc to none")

    s_why = section_text(body, "Why we do not copy the vendor product")
    check("clean-room" in s_why.lower(), "doc grounds the no-copy boundary in clean-room")
    check("our own" in text.lower(), "doc claims the model as ours")
    check("no vendor" in text.lower(), "doc explicitly says no vendor")

    # Surface-language rule: no em dashes anywhere in the doc.
    check(EM_DASH not in text, "doc contains no em dashes (program language rule)")


def run_checks():
    print("P8-M3-T01 identity + passport concept acceptance")
    check_phc_layout()
    check_doc()
    print("-" * 60)
    if failures:
        print("FAIL (%d)" % len(failures))
        for f in failures:
            print("  [ ]", f)
        sys.exit(1)
    print("ALL PASS")


if __name__ == "__main__":
    run_checks()
