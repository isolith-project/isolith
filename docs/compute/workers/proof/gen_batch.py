#!/usr/bin/env python3
"""Generate the P9-M3-T02 proof batch manifest (oep-batch v1) from a table."""
import json, sys

def job(name, fn, input_item, output_item, out_format):
    return {
        "format": "oep-job",
        "version": 1,
        "name": name,
        "description": f"P9-M3-T02 proof run: {fn}",
        "input": [{"name": "source", "ref": {"kind": "item", "item_id": input_item}}],
        "executor": {"kind": "convert", "function": fn},
        "resources": {"cpu_millicores": 1000, "memory_mib": 2048},
        "output": [{
            "name": "out",
            "format": out_format,
            "target": {"kind": "attach_item", "item_id": output_item},
        }],
    }

jobs = [
    job("cgr-stc06",     "cgr2gltf", "corpus-stc06",   "out-stc06",   "gltf"),
    job("cgr-valve",     "cgr2gltf", "corpus-valve",   "out-valve",   "gltf"),
    job("step-box-ref",  "step2mesh", "corpus-box-ref", "out-box-ref",  "oep-mesh"),
    job("step-box-part", "step2mesh", "corpus-box-part","out-box-part", "oep-mesh"),
    job("step-assembly", "step2mesh", "corpus-assembly","out-assembly", "oep-mesh"),
    job("step-box-mini", "step2mesh", "corpus-box-mini","out-box-mini", "oep-mesh"),
    job("step-fc-probe", "step2mesh", "corpus-fc-probe","out-fc-probe", "oep-mesh"),
    job("step-fc-box",   "step2mesh", "corpus-fc-box",  "out-fc-box",  "oep-mesh"),
]

manifest = {
    "format": "oep-batch",
    "version": 1,
    "description": "P9-M3-T02 corpus batch - CGR->glTF (P2 dogfood) + STEP->mesh (P4 dogfood); outputs hash-verified in the P8 vault",
    "jobs": jobs,
}
out = sys.argv[1] if len(sys.argv) > 1 else "proof/batch-corpus.json"
with open(out, "w") as fh:
    json.dump(manifest, fh, indent=2)
    fh.write("\n")
print(f"wrote {out}: {len(jobs)} jobs")
