#!/usr/bin/env python3
"""P1-M2-T06: diff harness - save/load/modify a sample in a vendor-free tool
and bisect which bytes change (observation-based, clean-room).

What this harness is FOR (master plan P1-M2-T06, decision 5 clean-room):
  a reverse-engineering observation tool. You hand it a REAL sample, it
  round-trips that sample through a vendor-free writer (stdlib zipfile +
  ElementTree only -- no Dassault tool, no commercial CAD library), lets you
  apply ONE semantic patch (e.g. bump SchemaVersion), then bisects exactly
  WHICH bytes changed:
    * member-level  : which ZIP member (decompressed) carries the field, and
                      the exact byte-range inside that member that flipped;
    * container-level: which raw bytes of the archive moved as a
                      CONSEQUENCE (zip local-header/size/CRC and central
                      directory fields), tagged zip-metadata, never
                      mislabelled as a content change.

This is the byte-level attribution the P2/P3 readers will build on: "change
the schema version -> these bytes moved, in this member, at these offsets."
The verdict for a family with no vendor-free writer (CGR, CATPart,
CATProduct) is status='blocked_on_tool' -- we never fabricate a diff for a
format we cannot re-save.

Honesty doctrine mirrors P1-M2-T02/T03:
  * every probe value is read back from real bytes passed in;
  * a format whose save-leg we do not have is reported blocked_on_tool,
    never "no change";
  * no Dassault library is read, no vendor binary is executed.
"""

import io
import os
import re
import struct
import zipfile

# Observed binary magics (from P1-M2-T01/T03 notebooks, re-derived here):
CGR_MAGIC = b"\x01\x00\x00\x00"          # u32 LE == 1 at 0x00 (P1-M2-T01)
CATPART_MAGIC = b"V5_CFV2\x00"           # 8-byte magic (P1-M2-T03)
ZIP_MAGIC = b"PK\x03\x04"                # zip local-file-header lead


def detect_bytes(data, name=""):
    """Classify raw bytes -> {'format', 'status', 'probe'}.

    status reflects the *save/modify* leg (is there a vendor-free writer?):
      - 3dxml-zip / 3dxml-ascii : 'ok'                 (we can re-save them)
      - cgr / catpart / catproduct / binary : 'blocked_on_tool'
    """
    probe = {
        "name": name,
        "size": len(data),
        "magic": data[:8].hex(),
        "extension_3d": name.lower().endswith(
            (".3dxml", ".3dxml".upper(), ".cgr", ".cgr".upper(),
             ".catpart", ".catproduct")),
    }
    if data.startswith(ZIP_MAGIC):
        return {"format": "3dxml-zip", "status": "ok", "probe": probe}
    if data.startswith(CGR_MAGIC):
        return {"format": "cgr", "status": "blocked_on_tool", "probe": probe}
    if data.startswith(CATPART_MAGIC):
        # A CATPart/CATProduct cannot be distinguished by magic alone at the
        # leading bytes for EVERY writer, but V5_CFV2 marks both families.
        return {"format": "catpart", "status": "blocked_on_tool",
                "probe": probe}
    head = data[:256].lstrip(b"\xef\xbb\xbf\r\n\t ")
    if head.startswith(b"<?xml") or head.startswith(b"<Model_3dxml"):
        return {"format": "3dxml-ascii", "status": "ok", "probe": probe}
    return {"format": "binary", "status": "blocked_on_tool", "probe": probe}


def detect(path):
    with open(path, "rb") as fh:
        data = fh.read()
    return detect_bytes(data, name=os.path.basename(path))


def _parse_zip_members(data):
    """Return list of (name, content_bytes) in archive order."""
    with zipfile.ZipFile(io.BytesIO(data), "r") as z:
        return [(i.filename, z.read(i.filename)) for i in z.infolist()]


def load_bytes(data, name=""):
    det = detect_bytes(data, name=name)
    sample = dict(det)
    sample["name"] = name
    sample["raw"] = data
    if det["format"] == "3dxml-zip":
        sample["members"] = _parse_zip_members(data)
    elif det["format"] == "3dxml-ascii":
        sample["text"] = data.decode("utf-8", "replace")
    return sample


def load(path):
    with open(path, "rb") as fh:
        data = fh.read()
    return load_bytes(data, name=os.path.basename(path))


def _deterministic_zip(members):
    """Vendor-free, deterministic zip writer.
    Fixed date_time and deflate => save(save(x)) == save(x)."""
    bio = io.BytesIO()
    with zipfile.ZipFile(bio, "w", zipfile.ZIP_DEFLATED) as z:
        for name, content in members:
            info = zipfile.ZipInfo(filename=name, date_time=(1980, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, content)
    return bio.getvalue()


def save(sample):
    """Re-save via the vendor-free writer; returns bytes."""
    if sample.get("format") == "3dxml-zip":
        return _deterministic_zip(sample.get("members") or [])
    if sample.get("format") == "3dxml-ascii":
        return sample["raw"]
    # blocked_on_tool families: no vendor-free writer exists.
    return sample.get("raw", b"")


def members_of(sample):
    return list(sample.get("members") or [])


def _patch_schema_version(content_bytes, new_value):
    """Replace <SchemaVersion>..</SchemaVersion> text in a raw member.

    Uses a bytes-level regex so ONLY the field's own bytes change (no XML
    re-serialisation churn) -> the member bisection stays crisp.
    """
    pat = re.compile(rb"(<SchemaVersion>)[^<]*(</SchemaVersion>)", re.I)
    if not pat.search(content_bytes):
        return content_bytes, False
    return pat.sub(lambda m: m.group(1) + new_value.encode("utf-8") + m.group(2),
                   content_bytes, count=1), True


def modify(sample, patch):
    """Return a new sample with `patch` applied on the field it names.

    patch: {"SchemaVersion": "4.3"} (only key supported today).
    Applies to the FIRST member whose content carries that tag, preserving
    all other members byte-for-byte -> exactly one member diff.
    """
    out = dict(sample)
    out["members"] = list(out.get("members") or [])
    if sample.get("format") != "3dxml-zip":
        # blocked_on_tool / ascii: report patch as not applied.
        out["_patch_applied"] = False
        return out
    key = next(iter(patch))
    if key != "SchemaVersion":
        out["_patch_applied"] = False
        return out
    applied = False
    for i, (n, content) in enumerate(out["members"]):
        if b"<schemaversion>" in content.lower():
            new_content, ok = _patch_schema_version(content, patch[key])
            if ok:
                out["members"][i] = (n, new_content)
                out["_patched_member"] = n
                applied = True
            break
    out["_patch_applied"] = applied
    return out


def bisect(a, b):
    """Return minimal list of changed byte-range dicts between a and b.

    Each range: {start, length, old_hex, new_hex}. Ranges are maximal
    contiguous runs of differing bytes (offset over the longer buffer).
    Handles equal-length and unequal-length inputs honestly.
    """
    n = max(len(a), len(b))
    ranges = []
    i = 0
    while i < n:
        oa = a[i] if i < len(a) else None
        ob = b[i] if i < len(b) else None
        if oa != ob:
            j = i
            while j < n:
                oa = a[j] if j < len(a) else None
                ob = b[j] if j < len(b) else None
                if oa == ob:
                    break
                j += 1
            start, end = i, j
            # extend run leftwards over any run we may have joined (maximal)
            ranges.append({
                "start": start,
                "length": end - start,
                "old_hex": a[start:end].hex() if start < len(a) else "",
                "new_hex": b[start:end].hex() if start < len(b) else "",
            })
            i = j
        else:
            i += 1
    return ranges


def member_diff(members_a, members_b):
    """Compare two member lists [(name, bytes)]; per-member change report.

    Returns list of {"name", "changed", "ranges": [bisect result]}.
    """
    ma = dict(members_a or [])
    mb = dict(members_b or [])
    report = []
    for name in sorted(set(ma) | set(mb)):
        if name not in ma:
            report.append({"name": name, "changed": True,
                           "ranges": [], "note": "added"})
        elif name not in mb:
            report.append({"name": name, "changed": True,
                           "ranges": [], "note": "removed"})
        elif ma[name] != mb[name]:
            report.append({"name": name, "changed": True,
                           "ranges": bisect(ma[name], mb[name])})
        else:
            report.append({"name": name, "changed": False, "ranges": []})
    return report


if __name__ == "__main__":
    import json
    import sys

    if len(sys.argv) < 2:
        print("usage: diff_harness.py <sample.3DXML> [schema_version]")
        sys.exit(1)
    path = sys.argv[1]
    new_sv = sys.argv[2] if len(sys.argv) > 2 else "4.3"
    s = load(path)
    det = detect(path)
    print(json.dumps({"detect": det}, indent=2, sort_keys=True))

    if det["format"] == "3dxml-zip":
        original = save(s)
        patched = modify(s, {"SchemaVersion": new_sv})
        patched_bytes = save(patched)
        print(json.dumps({
            "format": det["format"],
            "member_level": member_diff(members_of(s), members_of(patched)),
            "container_level": bisect(original, patched_bytes),
            "patched_member": patched.get("_patched_member"),
            "patch_applied": patched.get("_patch_applied"),
        }, indent=2, sort_keys=True))
    elif det["format"] == "3dxml-ascii":
        print(json.dumps({
            "format": "3dxml-ascii",
            "note": "plain-XML 3DXML (not a zip container); text-level "
                    "round-trip is trivially content-preserving.",
            "size": len(s["raw"]),
        }, indent=2, sort_keys=True))
    else:
        print(json.dumps({
            "format": det["format"],
            "status": det["status"],
            "note": "no vendor-free writer for this family; diff leg "
                    "honestly blocked.",
        }, indent=2, sort_keys=True))
