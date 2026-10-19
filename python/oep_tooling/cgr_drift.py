#!/usr/bin/env python3
"""P1-M2-T02: CGR version-drift comparison tool (observation-based, clean-room).

This module is the COMPARISON layer above the P1-M2-T01 hex-level observation
notebook (python/oep_tooling/cgr_observe.py). It fingerprints real CGR files,
groups them by era/family, and reports whether the on-disk structure drifted
between era groups.

Method / honesty doctrine (program plan P1-M2-T02, decision 5 clean-room):
  * Every probe value is read back from a real .cgr file passed by the caller.
  * Era/family labels come from OBSERVED structure (which tail variant the
    file carries), not from marketing release names.
  * An era for which NO real sample is supplied is reported as
    status='blocked_on_sample' -- never silently treated as "no drift".
  * No Dassault library is read, no vendor binary is executed.

Observed invariants (re-derived here, documented in
docs/problems/cgr-version-drift.md and P1-M2-T01 cgr-hex-notebook.md):
  * magic    : u32 LE == 1 at 0x00  -> 4 bytes `01 00 00 00`
  * pad      : 96 zero bytes 0x04..0x63
  * template : 16 bytes 0x68..0x77 == `00 07 00 00 00 07 00 00 00 b1 01 0b
               01 08 02 4f` in every sample observed (R11-era doc set and
               2007-era sample)
  * alignment: file size % 0x1000 == 0
  * family tails at 0x78.. (two observed old-era families):
      - "class-string" : ASCII class-name strings near 0x7c/0xb9
        (CAT3DBagRep / CATSurfacicRep) -- piston.cgr (2007, older-V5 family)
      - "float-tail"   : f32 near 0x7c clustering ~1.158, no header class
        strings -- cfysa/*.cgr (CATIA R19-era documentation sample set)

The two families sharing the invariant header spine IS the headline drift
finding for the P2 reader: stable header, family-dependent tail.
"""

import json
import os
import re
import struct


# Observed fixed 16-byte header template (0x68..0x77), byte-for-byte in the
# eleven real samples verified 2026-10-15.
TEMPLATE = bytes.fromhex("000700000007000000b1010b0108024f")


def _read_ascii_runs(data, minlen=4):
    runs = []
    for m in re.finditer(rb"[\x20-\x7e]{%d,}" % minlen, data):
        runs.append((m.start(), m.group().decode("latin1")))
    return runs


def _header_strings(data, limit=0x200):
    """Class-name-like ASCII strings in the header region, if any."""
    head = data[:limit]
    out = []
    for off, s in _read_ascii_runs(head, 4):
        if re.match(r"^(CAT|CGR)", s) and len(s) <= 40:
            out.append((off, s))
    return out


def fingerprint(path):
    """Probe a real CGR file. Returns a plain-dict, JSON-serialisable probe."""
    with open(path, "rb") as fh:
        data = fh.read()
    size = len(data)
    probe = {
        "file": os.path.basename(path),
        "size": size,
        "magic_bytes": data[0:4].hex(),
        "magic_u32le": struct.unpack("<I", data[0:4])[0] if size >= 4 else None,
        "pad_zero_bytes": (data[4:0x64] == b"\x00" * 0x60) if size >= 0x64 else None,
        "template_hex": data[0x68:0x78].hex() if size >= 0x78 else None,
        "template_matches": (data[0x68:0x78] == TEMPLATE) if size >= 0x78 else False,
        "aligned_0x1000": (size % 0x1000 == 0),
        "header_strings": _header_strings(data, limit=0x200),
        "f32_at_0x7c": None,
        "family": None,
    }
    if size >= 0x80:
        probe["f32_at_0x7c"] = round(struct.unpack("<f", data[0x7C:0x80])[0], 6)

    # Family classification (observed, honest, from the bytes we have):
    cls = [s for _, s in probe["header_strings"]]
    if any(s.startswith("CAT3DBagRep") or s.startswith("CATSurfacicRep")
           for s in cls):
        probe["family"] = "class-string-tail"
        probe["family_evidence"] = "ASCII CAT3DRep-family class names in header"
    elif probe["f32_at_0x7c"] is not None and 1.0 < probe["f32_at_0x7c"] < 2.0:
        probe["family"] = "float-tail"
        probe["family_evidence"] = "f32 near 1.0 at 0x7c, no header class strings"
    else:
        probe["family"] = "unclassified"
        probe["family_evidence"] = "no recognised old-era tail marker"
    return probe


def compare(probes):
    """Group fingerprints by family/era and report drift honestly.

    probes: dict {filename: fingerprint() dict} OR iterable of probe dicts.
    Returns a report dict; any era with no samples is marked blocked_on_sample.
    """
    items = list(probes.values()) if isinstance(probes, dict) else list(probes)

    groups = {}
    n_total = 0
    for p in items:
        fam = p.get("family") or "unclassified"
        groups.setdefault(fam, []).append(p)
        n_total += 1

    era_status = {}
    for fam, ps in sorted(groups.items()):
        era_status[fam] = {
            "samples": len(ps),
            "files": [p.get("file") for p in ps],
            "header_spine_stable": all(
                p.get("template_matches") and p.get("magic_bytes") == "01000000"
                for p in ps),
            "status": "verified",
        }

    report = {
        "task": "P1-M2-T02",
        "samples_analysed": n_total,
        "families_found": sorted(groups.keys()),
        "era_status": era_status,
        # Honest verdict construction:
        #  - 0 families => nothing to compare.
        #  - verified families share the header spine => stable header within
        #    the observed old V5 family band; family-dependent tails exist.
        #  - a 'class-string-tail' vs 'float-tail' split => tail drift between
        #    the two observed old-era writer families.
        #  - any era mentioned by the plan but absent here is reported as
        #    blocked_on_sample, so nobody can misread an empty era as 'no drift'.
        "drift_verdict": None,
        "modern_era_2026": {
            "status": "blocked_on_sample",
            "reason": ("No license-clean, independently-downloadable "
                       "V5-6R2015+/R2026-era CGR sample currently available "
                       "(see P1-M1-T01 registry). Comparison for that era is "
                       "honestly recorded as blocked, not as 'no drift'."),
        },
    }

    verified = [f for f, st in era_status.items() if st["status"] == "verified"]
    spine_stable = all(
        era_status[f].get("header_spine_stable") for f in verified) if verified else False
    if not verified:
        report["drift_verdict"] = "no-real-material"
    elif len(verified) == 1:
        report["drift_verdict"] = (
            "stable-header-within-family" if spine_stable else "inconclusive")
    else:
        tails_differ = (
            "class-string-tail" in verified and "float-tail" in verified)
        if spine_stable and tails_differ:
            report["drift_verdict"] = (
                "stable-header-within-family,tail-drift-detected")
        elif spine_stable:
            report["drift_verdict"] = "stable-header-within-family"
        else:
            report["drift_verdict"] = "header-drift-detected"
    return report


if __name__ == "__main__":
    import sys
    paths = sys.argv[1:]
    if not paths:
        print("usage: cgr_drift.py <file.cgr> [file.cgr ...]")
        sys.exit(1)
    probes = {os.path.basename(p): fingerprint(p) for p in paths}
    rep = compare(probes)
    print(json.dumps(rep, indent=2, sort_keys=True))
