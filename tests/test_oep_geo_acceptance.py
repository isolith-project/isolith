"""P6-M1-T01 acceptance: oep-geo model schema + our-own-format JSON serialization.

Independent (stdlib-only) verification of the committed canonical example
document `crates/oep-geo/tests/golden/example-bracket.oepgeo.json`:

  - the committed bytes parse as JSON with a valid OEP header:
      format == "oep-geo", version == 1, units == "mm"
  - top-level layout is exactly the five schema collections under one Document:
      parts, bodies, features, assemblies  (+ header fields)
  - id discipline: every id is a non-empty string and the id space is unique
    across ALL five collections (one document = one id space), with no
    duplicate references inside any single list
  - referential integrity (the hard links that make the schema useful):
      part.body_ids -> bodies, part.feature_ids -> features
      body.feature_ids -> features, feature.body_ids -> bodies
      instance.source -> parts or assemblies (exactly one source kind)
  - body sanity: kind in {solid, sheet, wire}, bbox min <= max, all floats finite
  - every instance transform is exactly 16 finite numbers, with a unit-length
    rotation block (orthonormal rows 0-2) and a finite translation in [12..15]
  - assembly graph: the instance graph is acyclic (sub-assembly DAG), and every
    assembly except declared roots is reachable from a root
  - golden spot values: bracket body is a solid with bbox max = (80, 40, 6); the
    fastener-set holds 4 fastener part instances at the four bracket corners;
    the top assembly references the bracket part and the fastener-set sub-assembly

Written first (TDD): the committed golden and these structural checks are the
"real material" gate; the Rust implementation is verified in two directions --
crates/oep-geo/tests/schema_serde.rs (single round-trip, byte-locked to the
same golden) and this file (independent re-parse with the stdlib json module).
"""

import json
import math
import os

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "crates", "oep-geo", "tests", "golden", "example-bracket.oepgeo.json")

EXPECTED_HEADER = {
    "format": "oep-geo",
    "version": 1,
    "units": "mm",
}
COLLECTIONS = ("parts", "bodies", "features", "assemblies")
BODY_KINDS = {"solid", "sheet", "wire"}


def _load():
    with open(GOLDEN, "r", encoding="utf-8") as fh:
        return json.load(fh)


def _resolve(doc):
    """Index a document by collection name -> {id: entry} plus a global id set."""
    by_col = {}
    all_ids = []
    for col in COLLECTIONS:
        m = {}
        for entry in doc.get(col, []):
            m[entry["id"]] = entry
            all_ids.append(entry["id"])
        by_col[col] = m
    return by_col, all_ids


# ---------------------------------------------------------------------------
# header + layout
# ---------------------------------------------------------------------------


def test_golden_present_and_parses():
    assert os.path.isfile(GOLDEN), "missing committed golden %s" % GOLDEN
    doc = _load()
    assert isinstance(doc, dict)


def test_header_exact():
    doc = _load()
    for key, val in EXPECTED_HEADER.items():
        assert doc.get(key) == val, "header field %r mismatch" % key


def test_top_level_collections_exact():
    doc = _load()
    expected = set(EXPECTED_HEADER.keys()) | set(COLLECTIONS)
    assert set(doc.keys()) == expected, \
        "unexpected top-level keys: %r" % (set(doc.keys()) - expected)


# ---------------------------------------------------------------------------
# id discipline
# ---------------------------------------------------------------------------


def test_ids_nonempty_and_unique_across_all_collections():
    doc = _load()
    _, all_ids = _resolve(doc)
    assert all_ids, "document has no entries"
    assert all(isinstance(i, str) and i for i in all_ids), "an id is empty/not a string"
    assert len(all_ids) == len(set(all_ids)), "id space is not unique across collections"


def test_id_naming_convention():
    doc = _load()
    _, all_ids = _resolve(doc)
    for i in all_ids:
        assert i.startswith("oep-"), "id %r does not follow the oep- convention" % i


def test_no_duplicate_reference_within_a_list(doc=None):
    doc = doc or _load()
    for part in doc["parts"]:
        for key in ("body_ids", "feature_ids"):
            ids = part[key]
            assert len(ids) == len(set(ids)), "duplicate %s in part %r" % (key, part["id"])
    for body in doc["bodies"]:
        ids = body["feature_ids"]
        assert len(ids) == len(set(ids)), "duplicate feature_ids in body %r" % body["id"]
    for feat in doc["features"]:
        ids = feat["body_ids"]
        assert len(ids) == len(set(ids)), "duplicate body_ids in feature %r" % feat["id"]


# ---------------------------------------------------------------------------
# referential integrity
# ---------------------------------------------------------------------------


def test_part_references_resolve():
    doc = _load()
    by_col, _ = _resolve(doc)
    for part in doc["parts"]:
        for ref in part["body_ids"]:
            assert ref in by_col["bodies"], "part %r refs missing body %r" % (part["id"], ref)
        for ref in part["feature_ids"]:
            assert ref in by_col["features"], "part %r refs missing feature %r" % (part["id"], ref)


def test_body_references_resolve():
    doc = _load()
    by_col, _ = _resolve(doc)
    for body in doc["bodies"]:
        for ref in body["feature_ids"]:
            assert ref in by_col["features"], "body %r refs missing feature %r" % (body["id"], ref)


def test_feature_references_resolve():
    doc = _load()
    by_col, _ = _resolve(doc)
    for feat in doc["features"]:
        for ref in feat["body_ids"]:
            assert ref in by_col["bodies"], "feature %r refs missing body %r" % (feat["id"], ref)


def test_all_bodies_and_features_are_used():
    doc = _load()
    by_col, _ = _resolve(doc)
    used_bodies = set()
    used_features = set()
    for part in doc["parts"]:
        used_bodies.update(part["body_ids"])
        used_features.update(part["feature_ids"])
    assert used_bodies == set(by_col["bodies"]), "unreferenced bodies present"
    assert used_features == set(by_col["features"]), "unreferenced features present"


# ---------------------------------------------------------------------------
# body sanity
# ---------------------------------------------------------------------------


def test_bodies_valid():
    doc = _load()
    for body in doc["bodies"]:
        assert body["kind"] in BODY_KINDS, "bad kind %r on body %r" % (body["kind"], body["id"])
        bbox = body.get("bbox")
        if bbox is not None:
            mn, mx = bbox["min"], bbox["max"]
            assert len(mn) == len(mx) == 3
            for axis in range(3):
                assert math.isfinite(mn[axis]) and math.isfinite(mx[axis])
                assert mn[axis] <= mx[axis], "bbox min>max on body %r" % body["id"]


# ---------------------------------------------------------------------------
# instance + transform sanity
# ---------------------------------------------------------------------------


def _floats(seq):
    return [float(x) for x in seq]


def test_every_instance_resolves_exactly_one_source():
    doc = _load()
    by_col, _ = _resolve(doc)
    for asm in doc["assemblies"]:
        for inst in asm["instances"]:
            src = inst["source"]
            # interned tag must be exactly one of part / sub-assembly
            assert set(src.keys()) == {"kind", "id"}, "bad source shape on %r" % inst["id"]
            assert src["kind"] in ("part", "sub-assembly"), "bad source kind %r" % src["kind"]
            target = by_col["parts"] if src["kind"] == "part" else by_col["assemblies"]
            assert src["id"] in target, "instance %r refs missing %s %r" % (
                inst["id"], src["kind"], src["id"])


def test_transforms_are_16_finite_floats():
    doc = _load()
    for asm in doc["assemblies"]:
        for inst in asm["instances"]:
            t = inst["transform"]
            assert len(t) == 16, "transform not 16 entries on %r" % inst["id"]
            vals = _floats(t)
            assert all(map(math.isfinite, vals)), "non-finite transform on %r" % inst["id"]
            # rotation block rows 0-2 must be unit length (orthonormal-ish)
            for row in range(3):
                mag = math.sqrt(sum(vals[row + 4 * c] ** 2 for c in range(3)))
                assert abs(mag - 1.0) < 1e-9, "non-unit rotation row on %r" % inst["id"]
            # homogeneous row: last entry == 1
            assert abs(vals[15] - 1.0) < 1e-12, "non-homogeneous transform on %r" % inst["id"]


def test_visible_is_bool():
    doc = _load()
    for asm in doc["assemblies"]:
        for inst in asm["instances"]:
            assert isinstance(inst["visible"], bool), "visible not bool on %r" % inst["id"]


# ---------------------------------------------------------------------------
# assembly graph (DAG check, root-finding)
# ---------------------------------------------------------------------------


def _instance_graph(doc):
    """Return (adjacency: {asm_id: set(sub_assembly_ids)}, roots)."""
    adj = {}
    all_asm = set(a["id"] for a in doc["assemblies"])
    for asm in doc["assemblies"]:
        subs = set()
        for inst in asm["instances"]:
            src = inst["source"]
            if src["kind"] == "sub-assembly":
                subs.add(src["id"])
        adj[asm["id"]] = subs
    referenced = set()
    for subs in adj.values():
        referenced.update(subs)
    roots = all_asm - referenced
    return adj, roots


def test_assembly_graph_is_acyclic():
    doc = _load()
    adj, _ = _instance_graph(doc)
    visiting = set()
    done = set()

    def visit(node):
        assert node not in visiting, "cycle detected through assembly %r" % node
        if node in done:
            return
        visiting.add(node)
        for nxt in adj[node]:
            visit(nxt)
        visiting.discard(node)
        done.add(node)

    for asm in doc["assemblies"]:
        visit(asm["id"])


def test_every_assembly_reachable_from_a_root():
    doc = _load()
    adj, roots = _instance_graph(doc)
    assert roots, "no root assembly (every assembly is a sub-assembly)"
    reachable = set()

    def walk(node):
        if node in reachable:
            return
        reachable.add(node)
        for nxt in adj[node]:
            walk(nxt)

    for r in roots:
        walk(r)
    assert reachable == set(adj.keys()), "some assemblies unreachable from any root"


# ---------------------------------------------------------------------------
# golden spot values (fixes the canonical example to a known product)
# ---------------------------------------------------------------------------


def test_golden_spot_values():
    doc = _load()
    by_col, _ = _resolve(doc)

    assert set(by_col["parts"]) == {"oep-part-bracket", "oep-part-fastener"}
    assert set(by_col["bodies"]) == {"oep-body-bracket", "oep-body-fastener"}

    bracket = by_col["bodies"]["oep-body-bracket"]
    assert bracket["kind"] == "solid"
    assert bracket["bbox"]["max"] == [80.0, 40.0, 6.0]
    assert bracket["bbox"]["min"] == [0.0, 0.0, 0.0]

    fastener = by_col["bodies"]["oep-body-fastener"]
    assert fastener["bbox"]["max"] == [10.0, 10.0, 20.0]

    # fastener-set: exactly 4 instances, all sourcing the fastener part,
    # placed at the four corners of the 80x40 bracket.
    fset = by_col["assemblies"]["oep-assembly-fastener-set"]
    insts = fset["instances"]
    assert len(insts) == 4
    positions = set()
    for inst in insts:
        assert inst["source"] == {"kind": "part", "id": "oep-part-fastener"}
        positions.add(tuple(_floats(inst["transform"][12:15])))
    assert positions == {(10.0, 10.0, 0.0), (70.0, 10.0, 0.0), (10.0, 30.0, 0.0), (70.0, 30.0, 0.0)}

    # top assembly: bracket part at identity + one fastener-set sub-assembly.
    top = by_col["assemblies"]["oep-assembly-bracket-assembly"]
    assert len(top["instances"]) == 2
    sources = sorted((i["source"]["kind"], i["source"]["id"]) for i in top["instances"])
    assert sources == [
        ("part", "oep-part-bracket"),
        ("sub-assembly", "oep-assembly-fastener-set"),
    ]
    bracket_inst = next(i for i in top["instances"] if i["source"]["id"] == "oep-part-bracket")
    assert _floats(bracket_inst["transform"]) == [1.0, 0.0, 0.0, 0.0] * 4

    # feature vocabulary uses the open PascalCase kind convention.
    for feat in doc["features"]:
        assert feat["kind"][0].isupper(), "feature kind not PascalCase: %r" % feat["kind"]
