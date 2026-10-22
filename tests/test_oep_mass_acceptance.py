"""Acceptance test for Isolith editor-spine analysis (decision #19 pull-forward):
mass properties, interference/clash detection, exploded views.

The committed real material (crates/oep-analysis/tests/golden/clash-fixture.*)
is re-derived here with the stdlib independently of the Rust implementation, so
a drift in either the crate or the fixture bytes is caught:

  fixture   => a two-part assembly whose parts PENETRATE (i-base base plate
               40x30x10 at origin; i-block 20x10x10 at (25,20,5))
  mass      => base plate volume 12000 mm3, mass 32.4 g (aluminium 2.70 g/cm3),
               centroid (20,15,5); block volume 2000 mm3, mass 15.7 g (steel)
  clash     => the known clashing pair (base x block) is flagged; overlap
               x in [25,40] (15), y in [20,30] (10), z in [5,10] (5)
               => volume 750 mm3, location (32.5, 25.0, 7.5)
  exploded  => deterministic offsets (same input + pull => same output)

And the crate's own suite runs green via `cargo test -p oep-analysis`
(the real binary, matching the test_p9_m3_sandbox pattern).

Run:  python3 tests/test_oep_mass_acceptance.py   (and under pytest)
Exit 0 = pass; nonzero = fail. Stdlib only.
"""
import json
import os
import shutil
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = os.path.join(REPO, "crates", "oep-analysis")
GOLD = os.path.join(CRATE, "tests", "golden")
DOC = os.path.join(GOLD, "clash-fixture.oepgeo.json")
MAT = os.path.join(GOLD, "clash-fixture.materials.json")
LAYOUT = os.path.join(REPO, "docs", "layout.md")

TOL = 1e-9

REQUIRED_TESTS = [
    "base_plate_mass_properties_match_analytical",
    "block_mass_properties_match_analytical",
    "known_clashing_pair_is_flagged",
    "exploded_view_is_deterministic_at_constant_pull",
    "exploded_view_offset_scales_with_pull",
    "exploded_document_renders_and_is_deterministic",
    "clash_report_is_deterministic",
]

_failures = []


def check(name, ok, detail=""):
    status = "PASS" if ok else "FAIL"
    line = "[%s] %s" % (status, name)
    if detail:
        line += " -- " + detail
    print(line)
    if not ok:
        _failures.append(name)


def approx(a, b, tol=TOL):
    return abs(a - b) <= tol * max(1.0, abs(a), abs(b))


def _load(path):
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def test_crate_exists_and_is_member():
    check("crate exists", os.path.isdir(CRATE), CRATE)
    with open(os.path.join(REPO, "Cargo.toml"), encoding="utf-8") as fh:
        root = fh.read()
    check("workspace member declared", '"crates/oep-analysis"' in root)
    check("layout.md maps oep-analysis",
          os.path.isfile(LAYOUT) and "oep-analysis" in open(LAYOUT, encoding="utf-8").read())


def test_fixture_files_present():
    check("fixture doc present", os.path.isfile(DOC))
    check("fixture material present", os.path.isfile(MAT))


def test_decodes_document():
    doc = _load(DOC)
    check("format oep-geo", doc.get("format") == "oep-geo")
    check("version 1", doc.get("version") == 1)
    # Exactly one id space across the two parts/bodies/instances.
    ids = []
    for part in doc.get("parts", []):
        ids.append(part["id"])
        ids.extend(part.get("body_ids", []))
    insts = [i["id"] for a in doc.get("assemblies", []) for i in a.get("instances", [])]
    ids += insts
    check("unique ids", len(set(ids)) == len(ids))


def test_mass_properties_independent():
    doc = _load(DOC)
    mat = _load(MAT)
    density = {m["id"]: m["density_g_cm3"] for m in mat["materials"]}
    assign = mat["body_material"]
    bodies = {b["id"]: b for b in doc["bodies"]}

    # Base plate: 40x30x10 mm solid, aluminium.
    b = bodies["oep-body-base"]
    mn, mx = b["bbox"]["min"], b["bbox"]["max"]
    lx, ly, lz = (mx[i] - mn[i] for i in range(3))
    vol = lx * ly * lz
    dens = density[assign["oep-body-base"]]
    mass = dens * vol / 1000.0  # mm3 -> cm3
    centroid = [(mx[i] + mn[i]) / 2.0 for i in range(3)]
    check("base volume", approx(vol, 12000.0), "vol=%.6f" % vol)
    check("base mass", approx(mass, 32.4), "mass=%.6f" % mass)
    check("base centroid", all(approx(centroid[i], (20.0, 15.0, 5.0)[i]) for i in range(3)))
    # Ixx = m/12 (30^2 + 10^2) = 2.7 * 1000 = 2700 g.mm2.
    ixx = mass / 12.0 * (ly * ly + lz * lz)
    check("base Ixx", approx(ixx, 2700.0), "ixx=%.6f" % ixx)

    # Block: 20x10x10 mm solid, steel.
    b2 = bodies["oep-body-block"]
    mn2, mx2 = b2["bbox"]["min"], b2["bbox"]["max"]
    vol2 = (mx2[0] - mn2[0]) * (mx2[1] - mn2[1]) * (mx2[2] - mn2[2])
    dens2 = density[assign["oep-body-block"]]
    mass2 = dens2 * vol2 / 1000.0
    check("block volume", approx(vol2, 2000.0), "vol=%.6f" % vol2)
    check("block mass", approx(mass2, 15.7), "mass=%.6f" % mass2)


def _instance_world_bbox(doc, part_id, transform):
    """World AABB of a part's first solid body under a transform (translation)."""
    body = next(b for b in doc["bodies"] if b["id"] in
                next(p for p in doc["parts"] if p["id"] == part_id)["body_ids"])
    mn = body["bbox"]["min"]
    mx = body["bbox"]["max"]
    world_min = [mn[i] + transform[12 + i] for i in range(3)]
    world_max = [mx[i] + transform[12 + i] for i in range(3)]
    return world_min, world_max


def test_known_clashing_pair_flagged_independent():
    doc = _load(DOC)
    asm = doc["assemblies"][0]
    base_part = next(i for i in asm["instances"] if i["id"] == "i-base")
    block = next(i for i in asm["instances"] if i["id"] == "i-block")

    mn_a, mx_a = _instance_world_bbox(doc, base_part["source"]["id"], base_part["transform"])
    mn_b, mx_b = _instance_world_bbox(doc, block["source"]["id"], block["transform"])

    ov_min = [max(mn_a[i], mn_b[i]) for i in range(3)]
    ov_max = [min(mx_a[i], mx_b[i]) for i in range(3)]
    ext = [ov_max[i] - ov_min[i] for i in range(3)]
    vol = ext[0] * ext[1] * ext[2]
    loc = [(ov_max[i] + ov_min[i]) / 2.0 for i in range(3)]
    check("overlap positive on all axes", all(e > 0 for e in ext), str(ext))
    check("clash volume", approx(vol, 750.0), "vol=%.6f" % vol)
    check("clash location", all(approx(loc[i], (32.5, 25.0, 7.5)[i]) for i in range(3)))


def test_material_format_pinned():
    mat = _load(MAT)
    check("material format", mat.get("format") == "oep-analysis-material")
    check("material version", mat.get("version") == 1)


def run_crate_suite():
    if shutil.which("cargo") is None:
        check("cargo available", False, "cargo not on PATH; cannot run the analysis suite")
        return
    r = subprocess.run(
        ["cargo", "test", "-p", "oep-analysis"],
        cwd=REPO,
        capture_output=True,
        text=True,
        timeout=900,
    )
    out = r.stdout + "\n" + r.stderr
    check("cargo test -p oep-analysis green", r.returncode == 0,
          out.strip()[-800:] if r.returncode else "")
    missing = [t for t in REQUIRED_TESTS if t not in out]
    check("all acceptance tests ran", not missing, "missing: %s" % missing)


def main():
    print("Isolith editor-spine analysis acceptance (mass / clash / exploded)")
    print("repo: %s" % REPO)
    print("=" * 60)
    test_crate_exists_and_is_member()
    test_fixture_files_present()
    test_decodes_document()
    test_material_format_pinned()
    test_mass_properties_independent()
    test_known_clashing_pair_flagged_independent()
    run_crate_suite()
    print("=" * 60)
    if _failures:
        print("FAILED %d check(s): %s" % (len(_failures), ", ".join(_failures)))
        return 1
    print("ALL CHECKS PASSED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
