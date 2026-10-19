#!/usr/bin/env python3
"""P1-M2-T04: 3DXML ASCII vs binary (.3DRep) hex-level observation tool.

Pure-stdlib, read-only. Classifies and reports the observed byte-level facts
of a real 3DXML file: ZIP container vs standalone ASCII model document, and
for each .3DRep member the ASCII (XMLRepresentation / OSM) vs binary
(V5_CFV3) branch. This is the "observation" half of the hex notebook: it
exists to re-derive the facts the notebook records from real bytes, so claims
never drift from material. It deliberately does NOT parse geometry -- P3 owns
that.

Usage:
    python -m oep_tooling.3dxml_observe <file.3dxml|file.3DXML|file.3DRep> [more ...]

Exit 0 on success (every file scanned and matched an observed profile),
1 if a file cannot be read or matches no observed 3DXML profile,
2 if no files given.
"""
import os
import re
import struct
import sys
import zipfile

# Observed facts (documented in docs/re-notes/3dxml-hex-notebook.md).
# If real material later contradicts these, UPDATE the notebook AND the
# acceptance test together - never silently diverge.
PK_MAGIC = b"\x50\x4b\x03\x04"               # ZIP local-file header
XML_HEAD = b"<?xm"                            # XML declaration head
V5_MAGIC = b"V5_CFV3\x00"                    # binary .3DRep magic at 0x00
V5_TOKEN = b"CATIA_V5 CB0001\x00"            # version token at 0x40
V5_TOKEN_OFF = 0x40
OSM_NS = b'xmlns="http://www.3ds.com/xsd/osm.xsd"'


def u32be(data, off):
    return struct.unpack_from(">I", data, off)[0]


def classify_rep(name, data):
    """Report facts for a .3DRep member; returns ('ascii'|'binary'|'other')."""
    n = len(data)
    print("    %-44s %8d B" % (name, n))
    if data.lstrip().startswith(b"<"):
        head = data.lstrip()[:512]
        m = re.match(rb'<\?xml[^>]*>\s*<XMLRepresentation version="([\d.]+)"',
                     head, re.S)
        if m:
            print("      branch: ASCII/XML  (XMLRepresentation %s)"
                  % m.group(1).decode())
            print("      root xsi:type=\"BagRepType\" : %s"
                  % (b'xsi:type="BagRepType"' in data))
            print("      PolygonalRep                 : %s"
                  % (b'PolygonalRepType' in data))
            print("      VertexBuffer/Positions/Faces : %s/%s/%s"
                  % (b"<VertexBuffer" in data, b"<Positions>" in data,
                     b"<Faces>" in data))
            return "ascii"
        if head.startswith(b"<Osm") or OSM_NS in data[:512]:
            print("      branch: ASCII/XML  (OSM material rep)")
            return "ascii"
        print("      branch: ASCII/XML  (unclassified XML rep)")
        return "ascii"
    # binary branch
    print("      branch: BINARY   (V5_CFV3 native-V5 container)")
    print("      magic 0x00..0x07 : %s" % data[:8].hex(" "))
    print("      u32BE 0x08       : %d" % u32be(data, 0x08))
    print("      u32BE 0x0C       : %d   (0x08+0x0C == len? %s)"
          % (u32be(data, 0x0C), u32be(data, 0x08) + u32be(data, 0x0C) == n))
    print("      bytes 0x10..0x17 : %s" % data[0x10:0x18].hex(" "))
    off = data.find(V5_TOKEN)
    print("      'CATIA_V5 CB0001'@0x%02x : %s   (4-byte prefix %s)"
          % (off, off == V5_TOKEN_OFF, data[off - 4:off].hex(" ") if off >= 4 else "n/a"))
    print("      tail terminator      : %s (expect 'CB__END\\x00' in last 8)"
          % data[-8:].hex(" "))
    print("      zlib/deflate header? : %s (expect False)"
          % (data[:2] in (b"\x78\x9c", b"\x78\x01", b"\x78\xda")))
    return "binary"


def scan_3dxml(path):
    with open(path, "rb") as fh:
        data = fh.read()
    n = len(data)
    base = os.path.basename(path)
    print("== %s  (%d bytes, 0x%x) ==" % (base, n, n))
    mismatches = []
    if zipfile.is_zipfile(path) and data[:4] == PK_MAGIC:
        print("  container : ZIP-archive 3DXML  (magic %s)" % data[:4].hex(" "))
        z = zipfile.ZipFile(path)
        for name in z.namelist():
            info = z.getinfo(name)
            if name == "Manifest.xml":
                print("    %-44s %8d B  (manifest xml)" % (name, info.file_size))
                mtxt = z.read(name).decode("utf-8", "replace")
                m = re.search(r"<Root>([^<]+)</Root>", mtxt)
                print("      root -> %s" % (m.group(1) if m else "?"))
            elif name.lower().endswith(".3drep"):
                classify_rep(name, z.read(name))
            elif name.lower().endswith(".3dxml"):
                print("    %-44s %8d B  (model/aux xml %s)"
                      % (name, info.file_size,
                         "urn:3DXML refs" if "urn:3DXML:" in
                         z.read(name).decode("utf-8", "replace") else ""))
            else:
                c = z.read(name)[:8]
                print("    %-44s %8d B  (other; head %s)"
                      % (name, info.file_size, c.hex(" ")))
    elif data.startswith(XML_HEAD):
        txt = data.decode("utf-8", "replace")
        print("  container : standalone ASCII model document (no ZIP)")
        sv = re.search(r"<SchemaVersion>([\d.]+)</SchemaVersion>", txt)
        print("  SchemaVersion : %s" % (sv.group(1) if sv else "?"))
        print("  root <Model_3dxml : %s" % ("<Model_3dxml" in txt))
        print("  urn:3DXML refs    : %d" % txt.count("urn:3DXML:"))
        fmts = set(re.findall(r'format="([^"]+)"', txt))
        print("  format attrs     : %s" % (sorted(fmts) or "none"))
        print("  inline geometry  : %s" % ("<VertexBuffer" in txt))
    else:
        # maybe a bare rep file
        if base.lower().endswith(".3drep"):
            print("  container : bare .3DRep file")
            classify_rep(base, data)
        else:
            mismatches.append("unrecognized 3DXML profile (head %s)"
                              % data[:8].hex(" "))
    return mismatches


def main(argv):
    if not argv:
        print(__doc__)
        return 2
    rc = 0
    for p in argv:
        if not os.path.isfile(p):
            print("missing file: %s" % p, file=sys.stderr)
            rc = 1
            continue
        try:
            ms = scan_3dxml(p)
        except Exception as e:
            print("  ERROR scanning %s: %r" % (p, e))
            rc = 1
            continue
        if ms:
            print("  !!! does not match observed 3DXML profile: %s"
                  % "; ".join(ms))
            rc = 1
        print()
    return rc


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
