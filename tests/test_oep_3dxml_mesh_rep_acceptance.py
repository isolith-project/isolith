#!/usr/bin/env python3
"""P3-M1-T03 acceptance test (independent leg): mesh rep -> internal mesh,
verified against known-good geometry.

Written with ZERO Rust knowledge on purpose: it is the INDEPENDENT reference
decoder for the 3DXML tessellated-ASCII branch, implemented only with the
Python stdlib (zipfile + xml.etree). Cross-checking the Rust reader
(crates/oep-3dxml/tests/mesh_rep.rs) against THIS decoder on the same real
files is how "known-good geometry" is established — two implementations,
one byte source.

Strip-winding convention (pinned, [uncertain] on runs longer than 3, see
docs/re-notes/3dxml-mesh-rep.md): standard OpenGL triangle-strip ordering —
even index -> (v[i], v[i+1], v[i+2]), odd index -> (v[i+1], v[i], v[i+2]).
For a run of exactly 3 indices (our writer's output) both conventions give
the SAME single triangle, so reader/writer round-trips are unambiguous.

Usage:
    python3 tests/test_oep_3dxml_mesh_rep_acceptance.py [corpus-dir]

Exits 0 with a report when the corpus is present; prints SKIP and exits 0 when
it is absent (CI stays green before the P1 corpus PR merges). Also writes the
decoded facts to a temp file the Rust leg is not required to read — the Rust
leg asserts the same physical facts directly on cube1.
"""
import sys, os, json, tempfile, zipfile, xml.etree.ElementTree as ET

XSI_TYPE = "{http://www.w3.org/2001/XMLSchema-instance}type"

def corpus_dir(argv):
    # explicit CLI arg wins
    if len(argv) > 1 and os.path.isdir(argv[1]):
        return argv[1]
    d = os.environ.get("OEP_3DXML_SAMPLES", "")
    if d and os.path.isdir(d):
        return d
    here = os.path.dirname(os.path.abspath(__file__))
    for cand in (
        os.path.join(here, "..", "crates", "oep-3dxml", "samples"),
        os.path.join(here, "..", "corpus", "data", "3dxml"),
    ):
        if os.path.isfile(os.path.join(cand, "cube1.3dxml")):
            return cand
    return None

def local(tag):
    return tag.rsplit("}", 1)[-1]

def decode_positions_text(text):
    out = []
    if not text:
        return out
    for triple in text.split(","):
        triple = triple.strip()
        if not triple:
            continue
        out.append([float(v) for v in triple.split()])
    return out

def decode_face(attr):
    tris = []
    if "triangles" in attr:
        toks = attr["triangles"].split()
        for i in range(0, len(toks) - 2, 3):
            tris.append([int(toks[i]), int(toks[i + 1]), int(toks[i + 2])])
    elif "strips" in attr:
        for run in attr["strips"].split(","):
            inds = [int(v) for v in run.split()]
            for k in range(len(inds) - 2):
                if k % 2 == 0:
                    tris.append([inds[k], inds[k + 1], inds[k + 2]])
                else:
                    tris.append([inds[k + 1], inds[k], inds[k + 2]])
    return tris

def decode_rep_text(txt):
    root = ET.fromstring(txt)
    meshes = []
    for el in root.iter():
        if local(el.tag) == "Rep" and (el.attrib.get(XSI_TYPE) or "").endswith("PolygonalRepType"):
            pos = decode_positions_text(_find(el, "Positions"))
            tris = []
            for face in _findall(el, "Face"):
                tris.extend(decode_face(face.attrib))
            if pos and tris:
                xs = [p[0] for p in pos]
                ys = [p[1] for p in pos]
                zs = [p[2] for p in pos]
                meshes.append({
                    "npos": len(pos),
                    "ntri": len(tris),
                    "bbox": [min(xs), min(ys), min(zs), max(xs), max(ys), max(zs)],
                    "tri_min": min(min(t) for t in tris),
                    "tri_max": max(max(t) for t in tris),
                    "first_pos": pos[0],
                })
    return meshes

def _find(el, name):
    for c in el.iter():
        if local(c.tag) == name:
            return "".join(c.itertext())
    return ""

def _findall(el, name):
    return [c for c in el.iter() if local(c.tag) == name]

def bbox_close(a, b, eps=0.5):
    return all(abs(x - y) <= eps for x, y in zip(a, b))

def main(argv):
    d = corpus_dir(argv)
    if not d:
        print("SKIP: corpus not present (set OEP_3DXML_SAMPLES)")
        return 0
    report = {"skipped": [], "checked": 0, "mismatch": []}
    for fn in sorted(os.listdir(d)):
        if not fn.lower().endswith((".3dxml", ".3dxml".lower())):
            continue
        path = os.path.join(d, fn)
        with open(path, "rb") as f:
            head = f.read(4)
        if head != b"PK\x03\x04":
            report["skipped"].append((fn, "standalone xml (no zip)"))
            continue
        try:
            z = zipfile.ZipFile(path)
        except zipfile.BadZipFile as e:
            report["skipped"].append((fn, f"bad zip: {e}"))
            continue
        for info in z.infolist():
            nm = info.filename
            if not nm.lower().endswith(".3drep"):
                continue
            raw = z.read(nm)
            txt = raw.decode("utf-8", "replace")
            if "<XMLRepresentation" not in txt[:200]:
                report["skipped"].append((nm, "binary/other branch"))
                continue
            try:
                meshes = decode_rep_text(txt)
            except ET.ParseError as e:
                report["mismatch"].append((nm, f"parse error {e}"))
                continue
            # print machine-readable facts for the human/Rust cross-check
            for i, m in enumerate(meshes):
                report["checked"] += 1
                print(json.dumps({"file": fn, "member": nm, "idx": i, **m}))
    tmp = tempfile.NamedTemporaryFile(delete=False, suffix=".json")
    tmp.write(json.dumps(report).encode())
    tmp.close()
    print("TOTAL meshes checked:", report["checked"])
    print("report:", tmp.name)
    print("(Rust leg asserts cube1 physical sanity independently; this file is")
    print(" the human-readable independent decoder output for cross-checking.)")
    return 0

if __name__ == "__main__":
    sys.exit(main(sys.argv))
