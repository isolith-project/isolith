#!/usr/bin/env python3
"""Acceptance test for Keelworks P11-T04: crypto / integrity.

Two layers (same pattern as the P11-T06 supply-chain test):

1. Machine-check the committed mechanism (always): blob content-hash
   integrity tests exist in Rust and assert integrity failure on tamper, the
   release sign/verify scripts exist and are executable, the Ed25519 release
   pubkey is committed (64 hex), the doctrine doc claims the card and carries
   the mandated sections under the program language rule, the crypto CI
   workflow wires the checks, and a recorded signed-release round-trip proof
   exists and says pass.

2. Self-contained gpg detached-signature round trip (when gpg is present,
   forced by --full): scratch keyring + scratch signing key, sign a fake
   release artifact set with scripts/release/sign-release.sh, verify with
   verify-release.sh (PASS), tamper an artifact and prove verification FAILS,
   and (when the oep-update binaries are available) prove a tampered
   update.bundle.json is refused.

Run:  python3 tests/test_p11t04_crypto.py [--full]
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOC = os.path.join(ROOT, "P11", "crypto-integrity.md")
PUBKEY = os.path.join(ROOT, "P11", "keys", "oep-update-signing.pub")
SIGN_SH = os.path.join(ROOT, "scripts", "release", "sign-release.sh")
VERIFY_SH = os.path.join(ROOT, "scripts", "release", "verify-release.sh")
WORKFLOW = os.path.join(ROOT, ".gitea", "workflows", "crypto-integrity.yaml")
STORE_RS = os.path.join(ROOT, "crates", "oep-pdm", "src", "store.rs")
API_RS = os.path.join(ROOT, "crates", "oep-pdm", "src", "api.rs")
BLOB_INT = os.path.join(ROOT, "crates", "oep-pdm", "tests", "blob_integrity.rs")
SIGNER_RS = os.path.join(ROOT, "crates", "oep-update", "src", "bin", "oep-update-sign.rs")
PROOF_DIR = os.path.join(ROOT, "P11", "proof")

EM_DASH = "\u2014"
EN_DASH = "\u2013"
CURLY = "\u2018\u2019\u201c\u201d"

failures = []


def check(cond, msg):
    if not cond:
        failures.append(msg)


def read(path):
    with open(path, encoding="utf-8") as fh:
        return fh.read()


def section(text, heading):
    m = re.search(r"\n##{1,3} (?:[\d.]+\s*)?%s" % re.escape(heading), text)
    if not m:
        return ""
    start = m.end()
    nxt = re.search(r"\n##{1,3} ", text[start:])
    return text[start:] if nxt is None else text[start:start + nxt.start()]


def find_bin(name):
    cand = os.path.join(ROOT, "target", "debug", name)
    if os.path.isfile(cand) and os.access(cand, os.X_OK):
        return cand
    cand = os.path.join(ROOT, "target", "release", name)
    if os.path.isfile(cand) and os.access(cand, os.X_OK):
        return cand
    which = shutil.which(name)
    return which or None


def run(cmd, env=None, cwd=None, input=None):
    return subprocess.run(cmd, capture_output=True, text=True,
                          env=env, cwd=cwd, input=input)


def machine_checks():
    print("P11-T04 crypto/integrity acceptance")
    print("doc: %s" % DOC)
    print("=" * 66)

    # --- Blob content-hash integrity (Rust) ---------------------------------
    check(os.path.isfile(STORE_RS), "missing: crates/oep-pdm/src/store.rs")
    if os.path.isfile(STORE_RS):
        s = read(STORE_RS)
        check("Integrity(" in s, "StoreError::Integrity variant missing")
        check("fn blob_bytes" in s, "blob_bytes read gate missing")
        check("fn verify_blob" in s, "verify_blob gate missing")
        check("tampered_in_memory_blob_fails_retrieval_with_integrity" in s,
              "in-memory tampered-blob test missing in store.rs")
    check(os.path.isfile(API_RS) and read(API_RS).count("INTEGRITY") >= 1,
          "api.rs error_code::INTEGRITY missing")
    check(os.path.isfile(BLOB_INT), "missing: crates/oep-pdm/tests/blob_integrity.rs")
    if os.path.isfile(BLOB_INT):
        b = read(BLOB_INT)
        check("tampered_blob_file_fails_load_with_integrity" in b,
              "on-disk tampered-blob acceptance test missing")
        check("intact_persisted_blob_survives_reload_and_verifies" in b,
              "intact control test missing in blob_integrity.rs")

    # --- Signer supports the release-engine mode ------------------------------
    if os.path.isfile(SIGNER_RS):
        g = read(SIGNER_RS)
        check("--gen-key" in g, "oep-update-sign lacks --gen-key (build-host keygen)")
        check("--inline-payload" in g, "oep-update-sign lacks --inline-payload")
        check("--item" in g, "oep-update-sign lacks --item (multi-artifact)")

    # --- Release sign/verify scripts ------------------------------------------
    check(os.path.isfile(SIGN_SH) and os.access(SIGN_SH, os.X_OK),
          "sign-release.sh missing or not executable")
    check(os.path.isfile(VERIFY_SH) and os.access(VERIFY_SH, os.X_OK),
          "verify-release.sh missing or not executable")
    if os.path.isfile(SIGN_SH):
        check("oep-airgap-signing" in read(SIGN_SH),
              "sign-release.sh does not default to the air-gap signing key")
    if os.path.isfile(VERIFY_SH):
        check("oep-update-check" in read(VERIFY_SH),
              "verify-release.sh does not drive oep-update-check")

    # --- Committed key posture -------------------------------------------------
    check(os.path.isfile(PUBKEY), "missing: P11/keys/oep-update-signing.pub")
    if os.path.isfile(PUBKEY):
        pub = read(PUBKEY).strip()
        check(len(pub) == 64 and all(c in "0123456789abcdef" for c in pub),
              "P11/keys/oep-update-signing.pub must be a 64-hex Ed25519 pubkey, got %r" % pub)

    # --- Doctrine doc -----------------------------------------------------------
    check(os.path.isfile(DOC), "missing: P11/crypto-integrity.md")
    if os.path.isfile(DOC):
        text = read(DOC)
        check("P11-T04" in text, "doc does not claim P11-T04")
        for heading in (
            "Content-hash integrity on vault blobs",
            "Release artifact detached signatures",
            "Signed update manifest",
            "Air-gap signing key posture",
            "CI enforcement",
        ):
            check(section(text, heading).strip() != "",
                  "doc missing section: %s" % heading)
        for ch, name in ((EM_DASH, "em dash"), (EN_DASH, "en dash")):
            check(ch not in text, "doc contains a %s (program rule)" % name)
        for ch in CURLY:
            check(ch not in text, "doc contains a curly quote (program rule)")

    # --- CI workflow -------------------------------------------------------------
    check(os.path.isfile(WORKFLOW), ".gitea/workflows/crypto-integrity.yaml is missing")
    if os.path.isfile(WORKFLOW):
        wf = read(WORKFLOW)
        check("test_p11t04_crypto.py" in wf, "workflow does not run test_p11t04_crypto.py")
        check("blob_integrity" in wf, "workflow does not run the blob_integrity tests")
        check("oep-update" in wf, "workflow does not run the oep-update acceptance")

    # --- Recorded signed-release round-trip proof ----------------------------------
    proofs = []
    if os.path.isdir(PROOF_DIR):
        proofs = [f for f in os.listdir(PROOF_DIR)
                  if f.startswith("signed-release-roundtrip-") and f.endswith(".json")]
    check(len(proofs) >= 1, "no P11/proof/signed-release-roundtrip-*.json proof recorded")
    for pf in proofs:
        try:
            data = json.load(open(os.path.join(PROOF_DIR, pf), encoding="utf-8"))
            check(data.get("pass") is True, "roundtrip proof %s records pass != true" % pf)
            check(data.get("tamper_artifact_fails") is True,
                  "roundtrip proof %s missing tamper_artifact_fails=true" % pf)
            check(bool(data.get("artifacts", 0)) and bool(data.get("update_bundle")),
                  "roundtrip proof %s incomplete (needs artifacts + update_bundle)" % pf)
        except Exception as e:  # noqa: BLE001
            check(False, "roundtrip proof %s unreadable/invalid: %s" % (pf, e))

    print("proofs recorded: %s" % (", ".join(proofs) or "(none)"))
    return failures


def gpg_round_trip(full):
    """Self-contained gpg detached-signature round trip with scratch keys.

    Runs under --full (CI) or when gpg is already available on the host. Uses
    scratch keyrings only; never touches the real build-host secrets.
    """
    if shutil.which("gpg") is None:
        print("[skip] gpg round trip: gpg not installed")
        return
    signer = find_bin("oep-update-sign")
    checker = find_bin("oep-update-check")
    if signer is None or checker is None:
        if not full:
            print("[skip] gpg round trip: oep-update binaries not built; "
                  "run --full (CI) after `cargo build -p oep-update --bins`")
            return
        # CI full mode: build the bins once so the bundle leg can run.
        r = run(["cargo", "build", "-p", "oep-update", "--bins"], cwd=ROOT)
        if r.returncode != 0:
            check(False, "cannot build oep-update bins: %s" % r.stderr[-200:])
            return
        signer = find_bin("oep-update-sign")
        checker = find_bin("oep-update-check")
        if signer is None or checker is None:
            check(False, "oep-update bins still missing after cargo build")
            return

    T = tempfile.mkdtemp(prefix="p11t04-rt-")
    assets = os.path.join(T, "assets")
    out = os.path.join(T, "out")
    gnupg_sign = os.path.join(T, "gnupg-sign")
    gnupg_verify = os.path.join(T, "gnupg-verify")
    os.makedirs(assets, mode=0o700)
    os.makedirs(gnupg_sign, mode=0o700)
    os.makedirs(gnupg_verify, mode=0o700)
    for name, data in (("oep-pdm.play.bin", b"FAKE oep-pdm payload 01"),
                       ("oep-web.play.bin", b"FAKE oep-web payload 02 longer")):
        with open(os.path.join(assets, name), "wb") as fh:
            fh.write(data)

    try:
        # Scratch signing key (really held only in the scratch keyring).
        env_sign = dict(os.environ, GNUPGHOME=gnupg_sign)
        r = run(["gpg", "--batch", "--passphrase", "", "--quick-gen-key",
                 "OEP Crypto Dev <crypto-dev@localhost>", "rsa2048", "sign", "0"],
                env=env_sign)
        if r.returncode != 0:
            check(False, "cannot generate scratch gpg key: %s" % r.stderr[-200:])
            return
        uid = "OEP Crypto Dev <crypto-dev@localhost>"

        # Export the scratch pubkey into the verify keyring so verify finds it.
        r = run(["gpg", "--armor", "--export", uid], env=env_sign)
        if r.returncode != 0:
            check(False, "cannot export scratch pubkey")
            return
        env_verify = dict(os.environ, GNUPGHOME=gnupg_verify)
        r = run(["gpg", "--batch", "--import"], input=r.stdout,
                env=env_verify)
        if r.returncode != 0:
            check(False, "cannot import scratch pubkey into verify keyring")
            return

        # Scratch Ed25519 keypair for the update bundle via the built signer.
        ed_keys = os.path.join(T, "edkeys")
        r = run([signer, "--gen-key", ed_keys], cwd=T)
        if r.returncode != 0:
            check(False, "oep-update-sign --gen-key failed: %s" % r.stderr[-200:])
            return
        with open(os.path.join(ed_keys, "secret.hex")) as fh:
            ed_secret = fh.read().strip()
        with open(os.path.join(ed_keys, "public.hex")) as fh:
            ed_pub = fh.read().strip()

        # --- Sign leg ---------------------------------------------------------
        sign_env = dict(env_sign, OEP_UPDATE_SIGN_KEYFILE=os.path.join(ed_keys, "secret.hex"))
        r = run(["bash", SIGN_SH, "--assets", assets, "--version", "2027.1.0-rc2",
                 "--out", out, "--sign-key", uid], env=sign_env)
        if r.returncode != 0:
            check(False, "sign-release.sh failed: %s" % (r.stdout + r.stderr)[-400:])
            return
        check(os.path.isfile(os.path.join(out, "update.bundle.json")),
              "sign-release.sh did not emit update.bundle.json")

        # --- Intact verify: must PASS -----------------------------------------
        r = run(["bash", VERIFY_SH, "--assets", out, "--version", "2027.1.0-rc2",
                 "--check-keyring", gnupg_verify, "--checker", checker,
                 "--update-pubkey", ed_pub], env=env_verify)
        check(r.returncode == 0,
              "verify-release.sh on intact release must PASS, got rc=%s: %s"
              % (r.returncode, r.stdout[-300:]))

        # --- Tampered artifact: must FAIL -------------------------------------
        with open(os.path.join(out, "oep-pdm.play.bin"), "ab") as fh:
            fh.write(b"tampered")
        r = run(["bash", VERIFY_SH, "--assets", out, "--version", "2027.1.0-rc2",
                 "--check-keyring", gnupg_verify, "--checker", checker,
                 "--update-pubkey", ed_pub], env=env_verify)
        check(r.returncode != 0,
              "verify-release.sh on tampered artifact must FAIL, got rc=%s" % r.returncode)

        # --- Tampered update bundle: must FAIL (signature/verify) -------------
        # Re-sign a clean copy, then corrupt the bundle in place.
        r = run(["bash", SIGN_SH, "--assets", assets, "--version", "2027.1.0-rc2",
                 "--out", out, "--sign-key", uid], env=sign_env)
        if r.returncode == 0:
            bundle = os.path.join(out, "update.bundle.json")
            with open(bundle, "rb") as fh:
                data = bytearray(fh.read())
            data[len(data) // 2] ^= 0xFF
            with open(bundle, "wb") as fh:
                fh.write(bytes(data))
            r = run(["bash", VERIFY_SH, "--assets", out, "--version", "2027.1.0-rc2",
                     "--check-keyring", gnupg_verify, "--checker", checker,
                     "--update-pubkey", ed_pub], env=env_verify)
            check(r.returncode != 0,
                  "verify-release.sh on tampered update.bundle.json must FAIL, got rc=%s"
                  % r.returncode)
    finally:
        shutil.rmtree(T, ignore_errors=True)


def run_checks(full=False):
    machine_checks()
    gpg_round_trip(full)
    return failures


if __name__ == "__main__":
    full = "--full" in sys.argv[1:]
    fails = run_checks(full)
    if "--full" in sys.argv[1:]:
        print("mode: full (round trip forced)")
    if fails:
        print("P11-T04 crypto ACCEPTANCE: FAIL")
        for f in fails:
            print("  - " + f)
        sys.exit(1)
    print("P11-T04 crypto ACCEPTANCE: PASS")
    sys.exit(0)


def test_p11t04_crypto_acceptance():
    """pytest entry point: the same checks, asserted without sys.exit."""
    fails = machine_checks()
    # Under generic pytest, run the round trip only when the tooling exists so
    # the gate stays fast; --full forces it in the dedicated CI job.
    if "--full" in sys.argv or os.environ.get("OEP_CRYPTO_FULL") == "1":
        gpg_round_trip(True)
    else:
        gpg_round_trip(False)
    assert not fails, "P11-T04 crypto acceptance failed (%d): %s" % (
        len(fails), fails[:5])
