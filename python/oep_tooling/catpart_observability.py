"""P5-M1-T02 observability scanner for native CATPart entity classes.

Pure-stdlib, read-only, clean-room. Encodes the byte probes used by the
P5-M1-T02 feasibility report (docs/re-notes/p5-m1-t02-native-catpart-
observability.md) so every confidence rating is re-derivable from real corpus
bytes on a normal run.

Entity classes scanned (per unique CATPart file):
  * parameter graph / feature tree   -> readable length-prefixed feature-instance
                                        records + feature/attribute token census
  * B-Rep exact geometry             -> nested V5_CFV2 geometry blob (second
                                        magic .. first CB__END), its size fraction,
                                        IEEE-754 coordinate-density inside it,
                                        analytic-kind token locations
  * tessellation                     -> mesh-vocabulary strings, triangle-index
                                        buffer signature, CAT3DBagRep presence

API: fingerprint(path) -> dict of byte-observable facts
     scan_corpus(manifest) -> (probes, ok_ids, miss_ids) like catpart_drift

Run the acceptance gate:  python3 tests/test_p5m1t02_observability_acceptance.py
"""

import os
import re
import struct

MAGIC_V5 = b"V5_CFV2\x00"
CB_END = b"CB__END"
PRINTABLE = re.compile(rb"[\x20-\x7e]{4,}")


def token_offsets(data, minlen=4):
    return [(m.start(), m.group().decode("latin1"))
            for m in PRINTABLE.finditer(data)]


def token_set(data):
    return {s for _, s in token_offsets(data)}


# --- entity-class vocabularies (substring match, same lists as the report) ---

FEATURE_TOKENS = ["Pad", "Pocket", "Hole", "Sketch", "Revolution", "Shaft",
                  "Groove", "Slot", "Fillet", "Chamfer", "Thread", "Thickness",
                  "Shell", "Draft", "Mirror", "Rib", "Loft", "CircularPattern",
                  "RectangularPattern", "_FtrList", "_Parameters", "Parameter",
                  "Formula", "Relation", "Constraint", "MechanicalPart"]

BREP_CONTAINER_TOKENS = ["CGMGeom", "CATSeeBodyCont", "CATBRepModeContainer",
                         "CATMFBRP"]

ANALYTIC_TOKENS = ["Plane", "Line", "Circle", "Cylinder", "Cone", "Sphere",
                   "Torus", "Conic", "Parabola", "Hyperbola", "Ellipse", "Point"]

FREEFORM_TOKENS = ["Nurbs", "NURBS", "Gregory", "Bezier", "Spline"]

MESH_TOKENS = ["Mesh", "Tessell", "Facet", "Tessellation", "VertexBuffer",
               "Polygon", "CAT3DFacetRep", "CATMesh"]

# "Triang"/"Triangle" is deliberately NOT in MESH_TOKENS: in this corpus every
# occurrence is a drafting-symbol name (drafting:ISO_3D.DressUp.Symbols.Triangle
# and friends), not geometry. The acceptance test asserts that separately so the
# negative finding stays honest rather than hidden.


def _count_substrings(data, tokens):
    return sum(1 for t in tokens if t.encode("latin1") in data)


def _count_string_hits(data, tokens):
    hits = set()
    for _, s in token_offsets(data):
        for t in tokens:
            if t in s:
                hits.add(t)
    return len(hits)


def _plausible_f32(data, off):
    if off + 4 > len(data):
        return None
    v = struct.unpack_from("<f", data, off)[0]
    try:
        if v != v:  # nan
            return None
    except Exception:
        return None
    if abs(v) < 1e4 and v != 0.0:
        return v
    return None


def _plausible_f64(data, off):
    if off + 8 > len(data):
        return None
    v = struct.unpack_from("<d", data, off)[0]
    if v != v:  # nan
        return None
    if not (abs(v) < 1e6):
        return None
    return v


def nested_blob(data):
    """Return (blob_start, blob_end) for the nested V5_CFV2 geometry container.

    Defined as the region between the second V5_CFV2 magic and the first
    CB__END strictly after it. (None, None) when no such nesting exists.
    """
    magics = [m.start() for m in re.finditer(re.escape(MAGIC_V5), data)]
    if len(magics) < 2:
        return None, None
    b = magics[1]
    ends = [m.start() for m in re.finditer(re.escape(CB_END), data)]
    after = [e for e in ends if e > b]
    if not after:
        return None, None
    return b, after[0]


def max_f32_run(data, lo, hi):
    """Longest run of contiguous plausible nonzero f32 in [lo, hi)."""
    best = 0
    cur = 0
    i = lo
    hi = min(hi, len(data))
    while i + 4 <= hi:
        v = _plausible_f32(data, i)
        if v is not None:
            cur += 1
            if cur > best:
                best = cur
            i += 4
        else:
            cur = 0
            i += 4
    return best


def index_buffer_scan(data, lo, hi):
    """Count u32 runs (>= 9) of small values in [1, 4096) -- the loose
    signature of an index buffer. Zero on these corpus parts."""
    runs = 0
    i = lo
    hi = min(hi, len(data))
    while i + 4 <= hi:
        v = struct.unpack_from("<I", data, i)[0]
        if 0 < v < 4096:
            j = i
            n = 0
            while j + 4 <= hi:
                vv = struct.unpack_from("<I", data, j)[0]
                if not (0 < vv < 4096):
                    break
                n += 1
                j += 4
            if n >= 9:
                runs += 1
            i = j
        else:
            i += 4
    return runs


def fingerprint(path):
    with open(path, "rb") as fh:
        data = fh.read()
    is_part = b"MechanicalPart" in data and b"CATMechProdCont" not in data
    b, e = nested_blob(data)
    blob_frac = None
    if b is not None and e is not None:
        blob_len = e - b
        blob_frac = round(blob_len / len(data) * 100.0, 1)

    # analytic tokens located inside vs outside the blob; drafting-Triangle list
    tok = token_offsets(data)
    ana_inside = 0
    ana_total = 0
    triangle_contexts = []
    for off, s in tok:
        if "Triangle" in s or "Triang" in s:
            triangle_contexts.append(s)
        hit = any(a in s for a in ANALYTIC_TOKENS)
        if hit:
            ana_total += 1
            if b is not None and e is not None and b <= off < e:
                ana_inside += 1

    return {
        "id": os.path.basename(path),
        "size": len(data),
        "is_part": is_part,
        "magic_ok": data[:8] == MAGIC_V5,
        "writer_markers": bool(re.search(rb"V5R\d\d(?:SP\d+)*HF\d*", data)),
        # parameter graph
        "feature_token_hits": _count_string_hits(data, FEATURE_TOKENS),
        "feature_token_substrings": _count_substrings(data, FEATURE_TOKENS),
        # B-Rep exact geometry
        "brep_container_tokens": _count_string_hits(data, BREP_CONTAINER_TOKENS),
        "blob": (b, e) if b is not None else None,
        "blob_frac_pct": blob_frac,
        "f32_in_blob": max_f32_run(data, b, e) if b is not None else 0,
        "analytic_total": ana_total,
        "analytic_inside_blob": ana_inside,
        "freeform_tokens": _count_string_hits(data, FREEFORM_TOKENS),
        # tessellation
        "mesh_string_tokens": _count_string_hits(data, MESH_TOKENS),
        "triangle_strings": len(triangle_contexts),
        "triangle_contexts": triangle_contexts,
        "index_runs": index_buffer_scan(data, 0, len(data)),
        "has_cat3dbagrep": b"CAT3DBagRep" in data,
    }


def scan_corpus(manifest_entries, resolve):
    """Resolve every native entry to a local path via `resolve(entry)` and
    fingerprint it. Returns (probes, ok_ids, miss_ids)."""
    probes = []
    ok_ids = []
    miss_ids = []
    for idx, entry in enumerate(manifest_entries):
        path = resolve(entry)
        if not path:
            miss_ids.append(idx)
            continue
        try:
            fp = fingerprint(path)
            fp["idx"] = idx
            probes.append(fp)
            ok_ids.append(idx)
        except OSError:
            miss_ids.append(idx)
    return probes, ok_ids, miss_ids


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1:
        import json
        print(json.dumps(fingerprint(sys.argv[1]), indent=1, default=str))
