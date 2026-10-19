#!/usr/bin/env python3
"""Keelworks export-control classification gate (P11-T05).

The release-pipeline checkbox: every release <V> must carry a committed
classification file workstreams/P11/export-control/<V>.classification.toml asserting
UNCLASSIFIED status, data locality, and the absence of ITAR/ECCN-restricted
content. Absence of the file FAILS the release (exit non-zero) -- that is the
"export-control checkbox" wired into the pipeline, building on the P0-M2-T04
memo (workstreams/P0-M2/export-control-itar-eccn.md).

Pure stdlib (tomllib + argparse), no runtime deps, hermetic: it reads only
the repo path it is given and never writes unless --emit is used.

Subcommands
-----------
  verify --release <V> [--repo DIR]
      Exit 0 iff workstreams/P11/export-control/<V>.classification.toml exists AND
      passes schema + content validation (status UNCLASSIFIED, no negative
      assertions tripped, data_locality present). Exit 1 with a clear
      message otherwise. A MISSING file exits 1 with "release blocked".

  selfcheck [--repo DIR]
      CI-safe: parse the template and every committed
      *.<V>.classification.toml under workstreams/P11/export-control/. Requires no
      particular release. Exit 1 on any malformed / non-conforming file.

  emit --release <V> --train <T> [--date YYYY-MM-DD] [--repo DIR]
      Copy the template into place at workstreams/P11/export-control/<V>.classification
      .toml with release/train/date filled. A convenience for the release
      engineer; verify is the gate.
"""

from __future__ import annotations

import argparse
import os
import shutil
import sys
import tomllib
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
DEFAULT_REL_DIR = Path("workstreams/P11/export-control")
TEMPLATE_NAME = "classification.template.toml"

# Hard gate rules (mirror workstreams/P11/export-control.md section 2):
#  - status must be exactly UNCLASSIFIED
#  - [classification.itar].subject and [classification.ear].subject must be
#    false (not subject to ITAR / not subject to the EAR)
#  - every [restricted_content] assertion must be false, and data_locality
#    must be non-empty (data-locality assertion required)
#  - release field must match the requested version
REQUIRED_KEYS_TOP = ("release", "train", "classification_date")
REQUIRED_KEYS_CLASSIFICATION = ("status", "summary")
REQUIRED_KEYS_ITAR = ("subject",)
REQUIRED_KEYS_EAR = ("subject", "eccn", "enc_note", "firearm_cam_backstop")
REQUIRED_KEYS_RESTRICTED = (
    "contains_itar_technical_data",
    "contains_eccn_controlled_content",
    "contains_customer_data",
    "data_locality",
)
REQUIRED_KEYS_GOVERNANCE = ("basis", "prepared_by")


def _rel_dir(repo: Path) -> Path:
    return Path(repo) / DEFAULT_REL_DIR


def _missing(what: str, msg: str) -> int:
    print(f"FAIL  {what}: {msg}")
    return 1


def validate_file(path: Path, expected_release: str | None) -> int:
    """Validate one classification file. Returns 0 (good) or 1 (fail)."""
    if not path.is_file():
        if expected_release is not None:
            return _missing(
                f"classification file for {expected_release}",
                f"{path.relative_to(path.parents[3]) if path.is_absolute() else path} "
                f"does not exist -- release BLOCKED (export-control checkbox unchecked)",
            )
        return _missing("classification file", f"{path} does not exist")
    try:
        with open(path, "rb") as fh:
            data = tomllib.load(fh)
    except tomllib.TOMLDecodeError as exc:
        return _missing("TOML parse", f"{path.name}: {exc}")

    fails: list[str] = []
    for k in REQUIRED_KEYS_TOP:
        if k not in data:
            fails.append(f"missing top-level key {k!r}")
    classification = data.get("classification", {})
    for k in REQUIRED_KEYS_CLASSIFICATION:
        if k not in classification:
            fails.append(f"missing [classification].{k}")
    itar = classification.get("itar", {})
    for k in REQUIRED_KEYS_ITAR:
        if k not in itar:
            fails.append(f"missing [classification.itar].{k}")
    ear = classification.get("ear", {})
    for k in REQUIRED_KEYS_EAR:
        if k not in ear:
            fails.append(f"missing [classification.ear].{k}")
    restricted = data.get("restricted_content", {})
    for k in REQUIRED_KEYS_RESTRICTED:
        if k not in restricted:
            fails.append(f"missing [restricted_content].{k}")
    governance = data.get("governance", {})
    for k in REQUIRED_KEYS_GOVERNANCE:
        if k not in governance:
            fails.append(f"missing [governance].{k}")
    if fails:
        return _missing("schema", "; ".join(fails))

    # --- content / checkbox rules ---
    if classification["status"] != "UNCLASSIFIED":
        fails.append(
            f"classification.status={classification['status']!r} is not "
            f"UNCLASSIFIED -- release FLAGGED, must be resolved before shipping"
        )
    for k, v in itar.items():
        if v is True:
            fails.append(f"[classification.itar].{k}=true -- ITAR-subject content FLAGGED")
    for k, v in ear.items():
        if v is True:
            fails.append(f"[classification.ear].{k}=true -- EAR-subject content FLAGGED")
    for k, v in restricted.items():
        if k == "data_locality":
            if not str(v).strip():
                fails.append("[restricted_content].data_locality is empty -- data-locality assertion required")
            continue
        if v is True:
            fails.append(f"[restricted_content].{k}=true -- restricted content FLAGGED")

    if expected_release is not None:
        actual = data.get("release")
        if actual != expected_release:
            fails.append(
                f"release field {actual!r} != requested {expected_release!r} -- "
                f"wrong file for this release"
            )

    if fails:
        return _missing("content", "; ".join(fails))
    print(
        f"ok    {path.name}: UNCLASSIFIED, data-locality asserted, "
        f"no ITAR/ECCN-restricted content"
    )
    return 0


def cmd_verify(args: argparse.Namespace) -> int:
    rel = _rel_dir(Path(args.repo))
    candidate = rel / f"{args.release}.classification.toml"
    rc = validate_file(candidate, args.release)
    print(f"RESULT: {'PASS - release cleared to proceed' if rc == 0 else 'FAIL - release blocked'}")
    return rc


def cmd_selfcheck(args: argparse.Namespace) -> int:
    rel = _rel_dir(Path(args.repo))
    if not rel.is_dir():
        print(f"FAIL  {rel} does not exist")
        return 1
    rc = 0
    template = rel / TEMPLATE_NAME
    if template.is_file():
        # Template is intentionally filled with <V> placeholders; only check
        # that it is valid TOML with the required shape.
        try:
            with open(template, "rb") as fh:
                tomllib.load(fh)
            print(f"ok    {TEMPLATE_NAME}: parses as TOML (placeholders allowed)")
        except tomllib.TOMLDecodeError as exc:
            print(f"FAIL  {TEMPLATE_NAME}: {exc}")
            rc = 1
    else:
        print(f"FAIL  {TEMPLATE_NAME} missing")
        rc = 1
    for p in sorted(rel.glob("*.classification.toml")):
        r = validate_file(p, None)
        rc = rc or r
    print(f"RESULT: {'PASS' if rc == 0 else 'FAIL'}")
    return rc


def cmd_emit(args: argparse.Namespace) -> int:
    rel = _rel_dir(Path(args.repo))
    template = rel / TEMPLATE_NAME
    if not template.is_file():
        print(f"FAIL  template missing: {template}")
        return 1
    out = rel / f"{args.release}.classification.toml"
    if out.exists():
        print(f"FAIL  already exists: {out}")
        return 1
    text = template.read_text(encoding="utf-8")
    text = (
        text.replace("<V>", args.release)
        .replace("YYYY.N", args.train)
        .replace("YYYY-MM-DD", args.date)
    )
    out.write_text(text, encoding="utf-8")
    print(f"ok    emitted {out.relative_to(Path(args.repo))}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="export_control_gate", description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("verify", help="gate one release version")
    p.add_argument("--release", required=True, help="version, e.g. 2027.1.0-rc1")
    p.add_argument("--repo", default=str(REPO))
    p.set_defaults(func=cmd_verify)

    p = sub.add_parser("selfcheck", help="validate template + all committed files")
    p.add_argument("--repo", default=str(REPO))
    p.set_defaults(func=cmd_selfcheck)

    p = sub.add_parser("emit", help="emit a classification file from the template")
    p.add_argument("--release", required=True)
    p.add_argument("--train", required=True)
    p.add_argument("--date", default="")
    p.add_argument("--repo", default=str(REPO))
    p.set_defaults(func=cmd_emit)

    args = ap.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
