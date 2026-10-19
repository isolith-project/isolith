#!/usr/bin/env python3
"""P5-M3-T01: clean-room B-Rep extraction attempt for native CATParts.

Attempts to extract a B-Rep face/surface chain from a real V5_CFV2 CATPart
into the oep-geo native model (Document -> Part -> Body -> Features), on the
observable byte surface only. This is the hardest clean-room work in the
program: the exact NURBS/analytic coefficients live inside an encoded CGM/NV
sub-stream whose record boundaries are not byte-attributable from a static
sample. The tool therefore extracts every record whose presence IS attributable
at the byte level, structurally gates any numeric surface decode an analytic-
surface hypothesis could justify, and classifies extraction parity honestly
(exact / observed / [uncertain] / blocked_on_encoded_payload). It never
fabricates geometry: a record whose numbers cannot be validated to an
analytic-surface hypothesis is emitted with parity [uncertain], not with
invented coefficients.

Honesty doctrine (ADR P5-M1-T03 section 8, P4-M1 honesty budget):
  * No fabricated PASS. Every extracted record carries a byte offset (or a
    gated decode) and a parity label.
  * Tolerant never-crash: unknown/absent markers are logged and skipped, the
    document still validates.
  * The numeric payload is only emitted as an analytic surface when a strict
    structural hypothesis passes; otherwise it is reported as
    blocked_on_encoded_payload with the failed hypothesis named.
  * "Exact" is only claimed where independent byte evidence pins the record
    (magic, rep-graph markers, record-class markers with offsets).

Usage:
  python3 -m oep_tooling.catpart_brep_extract   <catpart> [--json out.json]
  python3 -m oep_tooling.catpart_brep_extract   --corpus <repo-root> [--json out.json]
Exit code 0 on success (report produced).
"""
import argparse
import json
import math
import os
import re
import struct
import sys

MAGIC_V5 = b"V5_CFV2\x00"

# B-Rep record-class markers observed in every real CATPart (the surface /
# edge / vertex / wire record classes CATIA's B-Rep payload self-labels).
BREP_RECORD_CLASSES = (
    ("surface_record", b"BrpRsur"),      # B-Rep "R-surface" face record class
    ("edge_record", b"BrpIRedge"),       # B-Rep indexed edge record class
    ("vertex_record", b"BrpVertex"),     # B-Rep vertex record class
    ("wire_record", b"BrpWire"),         # B-Rep wire record class
    ("super_brep", b"SuperBRep"),        # resolved composite B-Rep
    ("oper_breps", b"OperBReps"),        # operation B-Rep list
)

# Body/geometry container markers that anchor the B-Rep payload region.
BODY_CONTAINERS = (
    ("see_body_cont", b"CATSeeBodyCont"),
    ("brep_mode_cont", b"CATBRepModeContainer"),
    ("cgm_body", b"CATCGMBODY"),
    ("cgm_geom", b"CGMGeom"),
    ("mfbrp", b"CATMFBRP"),
)

# Analytic surface-family feature kinds CATIA self-labels in the feature tree;
# presence means the part's B-Rep references that surface family.
SURFACE_FAMILIES = (
    ("Cylinder", b"GSMCylinder"),
    ("Extrude", b"GSMExtrude"),
    ("Revol", b"GSMRevol"),
    ("Plane", b"GSMPlane"),
    ("Sphere", b"GSMSphere"),
    ("PlanarSupport", b"PlanarSupport"),
    ("Fill", b"Fill"),
)

# NV/CGM encoded geometry sub-stream marker (observed: CATIA stores the exact
# geometry payload behind this ASCII token family).
NV_MARKERS = (b"0B~}0%", b"CATIA_V5 CB0001", b"CB__END")


def find_all(data, token):
    return [m.start() for m in re.finditer(re.escape(token), data)]


def observed_log(item, offsets, note=""):
    """One byte-observed extraction record."""
    return {
        "offset": offsets[0] if offsets else None,
        "offset_hex": hex(offsets[0]) if offsets else None,
        "count": len(offsets),
        "offsets": offsets[:8],
        "note": note,
    }


def extract_brep_chain(data):
    """Extract every byte-attributable B-Rep record from one CATPart blob."""
    n = len(data)
    chain = {
        "magic_ok": data[:8] == MAGIC_V5,
        "size": n,
        "second_substream_offsets": [
            m.start() for m in re.finditer(re.escape(MAGIC_V5), data)][1:],
        "rep_graph": {},
        "body_containers": {},
        "brep_records": {},
        "surface_families": {},
        "nv_payloads": {},
    }

    # rep-graph: part -> shape-1 -> bag rep (the default representation)
    for name, tok in (("mechanical_part", b"MechanicalPart"),
                      ("prdrep", b"PRDREP"),
                      ("shape_1", b"Shape 1"),
                      ("bag_rep", b"BagRep"),
                      ("bag_reps_list", b"_BagRepsList")):
        offs = find_all(data, tok)
        if offs:
            chain["rep_graph"][name] = observed_log(name, offs)

    for name, tok in BODY_CONTAINERS:
        offs = find_all(data, tok)
        if offs:
            chain["body_containers"][name] = observed_log(name, offs)

    for name, tok in BREP_RECORD_CLASSES:
        offs = find_all(data, tok)
        if offs:
            chain["brep_records"][name] = observed_log(
                name, offs,
                "byte-observed B-Rep record-class marker in the part payload")

    for name, tok in SURFACE_FAMILIES:
        offs = find_all(data, tok)
        if offs:
            chain["surface_families"][name] = observed_log(
                name, offs,
                "byte-observed surface-family reference in the feature tree")

    for tok in NV_MARKERS:
        offs = find_all(data, tok)
        if offs:
            chain["nv_payloads"][tok.decode(errors="replace")] = observed_log(
                tok.decode(errors="replace"), offs,
                "CGM/NV encoded exact-geometry sub-stream marker")
    return chain


# ---------------------------------------------------------------------------
# Structural analytic-surface gating: only claim "exact" when a hypothesis
# passes on real bytes with no contradictions.
# ---------------------------------------------------------------------------

def gated_analytic_surfaces(data, chain):
    """Look for a self-consistent analytic cylinder record candidate.

    Hypothesis: a canonical cylinder surface record stores (axis-origin,
    axis-direction, radius, min, max) as five f64s in a small record after a
    surface-class marker. The structural gate is deliberately STRICT but a
    passed gate is a *candidate* (a byte-intersecting structure), not a
    semantically verified coefficient: we cannot prove from a static sample
    that these floats ARE the authored radius/axis, so the candidate is
    emitted with parity [uncertain] and the hypothesis named - never "exact".
    The ADR (P5-M1-T03 8) demands no fabricated geometry; an unverifiable
    float match is not evidence of a decoded surface, and this tool says so.
    """
    surf = []
    for off in chain["surface_families"].get("Cylinder", {}).get("offsets", []):
        # window: marker -> +0x2000 (bounded local record space)
        lo = off
        hi = min(len(data), off + 0x2000)
        window = data[lo:hi]
        hit = None
        for base in range(0, len(window) - 40, 8):
            vals = struct.unpack_from("<5d", window, base)
            if not all(math.isfinite(v) for v in vals):
                continue
            ax = vals[0], vals[1], vals[2]
            r = vals[3]
            try:
                norm = math.sqrt(ax[0] ** 2 + ax[1] ** 2 + ax[2] ** 2)
            except OverflowError:
                continue
            if not math.isfinite(norm) or norm < 1e-6:
                continue
            # an authored cylinder radius in mm must be plausible: >= 0.05 and
            # the axis must be (near) unit length for a canonical record
            if not (0.05 <= r < 1e5):
                continue
            if abs(norm - 1.0) > 1e-3:
                continue
            hit = {
                "offset": lo + base,
                "hypothesis": "analytic-cylinder(origin,dir,radius)",
                "parity": "[uncertain] structural candidate, not verified",
                "parameters": {
                    "radius": round(r, 6),
                    "axis": [round(v, 6) for v in (vals[0], vals[1], vals[2])],
                },
            }
            break  # one candidate per marker is enough for the attempt
        if hit is not None:
            surf.append(hit)
    return surf


def classify_parity(chain, gated):
    """Perface the honest parity verdict for the whole attempt."""
    observed_n = sum(len(v.get("offsets", []))
                     for v in chain["brep_records"].values())
    families_n = sum(len(v.get("offsets", []))
                     for v in chain["surface_families"].values())
    have_payload = bool(chain["nv_payloads"]) or bool(chain["second_substream_offsets"])
    return {
        "brep_record_class_markers": observed_n,
        "surface_family_references": families_n,
        "analytic_surface_candidates": len(gated),
        "exact_geometry_coefficients": "blocked_on_encoded_payload" if have_payload
        else "none_observed",
        "confidence": {
            "rep_graph_and_record_classes": "exact (byte-observed, offsets pinned)",
            "surface_family_references": "exact (byte-observed feature-tree)",
            "analytic_surface_parameters": ("[uncertain] structural candidate(s) "
                                            "only, not verified as authored "
                                            "geometry" if gated else
                                            "[uncertain] no gated decode"),
            "nurbs_control_points": "[uncertain]/blocked on the encoded CGM/NV "
                                    "payload: record boundaries not byte-"
                                    "attributable from a static sample",
        },
    }


def to_oep_geo_document(chain, gated, source_name):
    """Map the observed B-Rep chain into the documented oep-geo model.

    Document format follows docs/p6-m1-oep-geo-model-schema.md: flat
    id-keyed collections, format tag, version 1, mm units.
    """
    slug = re.sub(r"[^A-Za-z0-9]+", "-", os.path.basename(source_name)).strip("-").lower() or "catpart"
    part_id = "oep-part-%s" % slug
    body_id = "oep-body-%s" % slug

    features = []
    fid = 0
    for name, rec in sorted(chain["surface_families"].items()):
        fid += 1
        features.append({
            "id": "oep-feat-%s-%d" % (slug, fid),
            "name": name,
            "kind": name,
            "parameters": {
                "parity": "exact",
                "evidence": "byte-observed CATIA feature-tree family reference "
                            "(ASCII token present at pinned offsets); NOT a "
                            "decoded parametric feature or a verified surface",
                "count": rec["count"],
                "offsets": [hex(o) for o in rec["offsets"]],
            },
            "body_ids": [body_id],
        })
    for idx, s in enumerate(gated):
        fid += 1
        features.append({
            "id": "oep-feat-%s-%d" % (slug, fid),
            "name": "brep-analytic-cylinder-candidate-%d" % idx,
            "kind": "Cylinder",
            "parameters": {
                "parity": "[uncertain] structural candidate, not verified",
                "radius_candidate": s["parameters"]["radius"],
                "offset": hex(s["offset"]),
            },
            "body_ids": [body_id],
        })

    doc = {
        "format": "oep-geo",
        "version": 1,
        "units": "mm",
        "parts": [{
            "id": part_id,
            "name": os.path.basename(source_name),
            "body_ids": [body_id],
            "feature_ids": [f["id"] for f in features],
        }],
        "bodies": [{
            "id": body_id,
            "name": "Shape 1 B-Rep body",
            "kind": "solid",
            "feature_ids": [f["id"] for f in features],
        }],
        "features": features,
        "assemblies": [],
    }
    return doc


def observe_one(path):
    data = open(path, "rb").read()
    chain = extract_brep_chain(data)
    gated = gated_analytic_surfaces(data, chain)
    parity = classify_parity(chain, gated)
    doc = to_oep_geo_document(chain, gated, path)
    return {
        "file": os.path.basename(path),
        "sha256": None,  # filled by caller when registered
        "chain": chain,
        "gated_analytic_surfaces": gated,
        "parity": parity,
        "oep_geo_document": doc,
    }


def run_corpus(repo=None, limit=None):
    """Run extraction over the real native corpus via the shared registry."""
    sys.path.insert(0, os.path.join(os.path.dirname(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__)))), "python"))
    from oep_tooling.corpus import registry

    m = registry.load_manifest(repo=repo)
    entries = [e for e in m["data"].get("entries", [])
               if e.get("format") in ("catpart-v5", "catproduct-v5")]
    results = []
    miss = []
    for e in entries[:limit]:
        p, err = registry.resolve_entry(e, manifest=m)
        if err or not p or not os.path.isfile(p):
            miss.append((e["id"], err or "absent"))
            continue
        r = observe_one(p)
        r["id"] = e["id"]
        r["sha256"] = e.get("sha256")
        results.append(r)
    return results, miss


def main(argv=None):
    ap = argparse.ArgumentParser(prog="catpart_brep_extract",
                                 description=__doc__)
    ap.add_argument("path", nargs="?", default=None,
                    help="a real .CATPart/.CATProduct file")
    ap.add_argument("--corpus", default=None,
                    help="repo root to run over the shared native corpus")
    ap.add_argument("--limit", type=int, default=None)
    ap.add_argument("--json", default=None, help="write report JSON here")
    args = ap.parse_args(argv)

    if args.corpus:
        results, miss = run_corpus(args.corpus, limit=args.limit)
        report = {"task": "P5-M3-T01", "method": "byte-observed clean-room "
                  "B-Rep extraction attempt", "results": results,
                  "unresolved": miss, "resolved": len(results),
                  "unresolved_n": len(miss)}
    elif args.path:
        r = observe_one(args.path)
        report = {"task": "P5-M3-T01", "single": r}
    else:
        ap.error("pass a path or --corpus")

    text = json.dumps(report, indent=2, default=str)
    if args.json:
        with open(args.json, "w") as fh:
            fh.write(text + "\n")
    print(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
