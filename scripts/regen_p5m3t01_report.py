"""Regenerate the committed P5-M3-T01 report deterministically (no timestamp)."""
import sys, json, os
sys.path.insert(0, "python")
from oep_tooling.corpus import registry
import oep_tooling.catpart_brep_extract as B

m = registry.load_manifest(repo=".")
entries = [e for e in m["data"]["entries"]
           if e.get("format") in ("catpart-v5", "catproduct-v5")]
rows = []
miss = 0
for e in entries:
    p, err = registry.resolve_entry(e, manifest=m)
    if err or not p or not os.path.isfile(p):
        miss += 1
        continue
    r = B.observe_one(p)
    rows.append({
        "id": e["id"],
        "format": e["format"],
        "brep_record_class_markers": r["parity"]["brep_record_class_markers"],
        "surface_family_references": r["parity"]["surface_family_references"],
        "analytic_surface_candidates": r["parity"]["analytic_surface_candidates"],
        "exact_geometry_coefficients": r["parity"]["exact_geometry_coefficients"],
        "magic_ok": r["chain"]["magic_ok"],
        "second_substream": bool(r["chain"].get("second_substream_offsets")),
        "nv_payload": bool(r["chain"].get("nv_payloads")),
        "gated": [s["parameters"] for s in r["gated_analytic_surfaces"]],
    })
summary = {
    "task": "P5-M3-T01",
    "resolved": len(rows),
    "unresolved": miss,
    "files": rows,
}
out = "docs/re-notes/p5-m3-t01-brep-extraction-report.json"
with open(out, "w") as fh:
    json.dump(summary, fh, indent=1)
    fh.write("\n")
print("wrote", out, len(rows), "rows", miss, "missed")
