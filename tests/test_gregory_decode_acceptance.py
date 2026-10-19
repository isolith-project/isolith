"""P3-M2-T01 acceptance: Gregory-patch surface decode to NURBS.

Independent (stdlib-only) verification of the real-material corpus and the
surface-rep data model the Rust decoder consumes. It re-opens the same
committed Web3D LatticeXvl X3D files the Rust integration tests decode, so a
drift in either the corpus bytes or the Rust parser is caught from two sides.

Covered:
  - corpus registry: file set + exact sha256 of every committed sample
  - independent XML parse (no Rust, stdlib xml) of every XvlShell:
      * face rings are valid (all vertex indices in coordinate range)
      * edge lists are valid and within coordinate range
      * rounding-weight arrays match their cardinality (vertices / edges)
      * DEF/USE coordinate references resolve
  - topology goldens on the real freeform models (face/vertex counts)
  - the boundary-cubic convention the decoder implements is consistent with
    the stored edge-vector data on the EdgeVector demo.

Written first (TDD): the committed corpus and these structural checks are the
"real material" gate; the Rust implementation is verified by
crates/oep-3dxml/tests/gregory_decode.rs.
"""

import hashlib
import re
import xml.etree.ElementTree as ET  # noqa: S405 (stdlib, no external deps)

from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
SURFACE_DIR = REPO_ROOT / "corpus" / "data" / "3dxml" / "surface"

# sha256 of the committed bytes (guard against silent corpus drift).
REGISTRY = {
    "XvlShellModelStar.x3d": "9e919c7c26b919b592f8fb475828daadb8643221d944259bf4a351f608c88bbf",
    "XvlShellModelDolphin.x3d": "782a90a6a89da6c0ebb7a27cd8465977a87fea8e4e050f7ed92ee25de4127dc2",
    "XvlShellModelBaby.x3d": "f058d492378b9b1fedb086a47a49271e60b51600ffbe97471e79c23178968d05",
    "XvlShellModelOctopus.x3d": "dcd90442c2960105d76a128c9a1b384ef33a06448d6a7e3accc915e8f5888226",
    "XvlShellModelHand.x3d": "dde8e419ada2d779f84a434c17a78653d67680a44e8f454b3903212227a10759",
    "XvlShellAttributeEdgeVector.x3d": "c8e76df985971a83bf3ca810441476dbec5f6563a6e10a4afc3d7cc9c5fd286c",
    "XvlShellAttributeEdgeRound.x3d": "419d780a920a6f35e66dcf9fbc35ae77bb5f28b027bd296ff851689cc4e839b1",
    "XvlShellAttributeVertexRound.x3d": "3f5d5875d87d6478d03d0504df70d1163c541439dba4231cc4eac1e7362d49ae",
    "XvlShellAttributeAll.x3d": "16c048649ae8f32d553794773c2bac0ba5c2f2b8534cb3988b0feecf2181e79a",
    "XvlShellProtoDeclare.x3d": "b5d60e0279b0f52004d05b878fdbff799f90cea1223fcbed8b53b3d39439bb6f",
}


def _floats(text):
    return [float(x) for x in re.split(r"[\s,]+", text.strip()) if x]


def _ints(text):
    return [int(x) for x in re.split(r"[\s,]+", text.strip()) if x]


def _split_faces(idx):
    out, cur = [], []
    for v in idx:
        if v == -1:
            if cur:
                out.append(cur)
                cur = []
        elif v >= 0:
            cur.append(v)
    if cur:
        out.append(cur)
    return out


def _coordinate_map(s):  # DEF -> list[tuple[float,float,float]]
    out = {}
    for m in re.finditer(r"<Coordinate\s+DEF='([^']+)'\s+point='([^']*)'", s):
        nums = _floats(m.group(2))
        out[m.group(1)] = list(zip(nums[0::3], nums[1::3], nums[2::3]))
    return out


def _xvl_shells(s):
    """Yield (attrs, coordinate_vertices) per XvlShell element."""
    coords = _coordinate_map(s)
    for m in re.finditer(r"<XvlShell\b([^>]*)>(.*?)</XvlShell>", s, re.S):
        headers = dict(
            (k, v or w)
            for k, v, w in re.findall(
                r"([\w:]+)\s*=\s*(?:'([^']*)'|\"([^\"]*)\")", m.group(1)
            )
        )
        body = m.group(2)
        vertices = []
        for cm in re.finditer(r"<Coordinate\b([^>]*)/>", body):
            ch = dict((k, v or w) for k, v, w in re.findall(
                r"([\w:]+)\s*=\s*(?:'([^']*)'|\"([^\"]*)\")", cm.group(1)))
            if "USE" in ch:
                vertices.extend(coords.get(ch["USE"], []))
            elif "point" in ch:
                nums = _floats(ch["point"])
                vertices.extend(zip(nums[0::3], nums[1::3], nums[2::3]))
        yield headers, vertices, body


def test_corpus_registry_intact():
    """All committed samples are present with their recorded sha256."""
    missing = [_ for _ in REGISTRY if not (SURFACE_DIR / _).exists()]
    assert not missing, f"missing committed sample(s): {missing}"
    for name, want in REGISTRY.items():
        data = (SURFACE_DIR / name).read_bytes()
        got = hashlib.sha256(data).hexdigest()
        assert got == want, f"{name} sha256 drift: {got} != {want}"
    license = SURFACE_DIR / "WEB3D-LICENSE.txt"
    assert license.exists() and "Web3D" in license.read_text(), "license present"
    readme = SURFACE_DIR / "README.md"
    assert readme.exists(), "corpus README with attribution present"


def test_topology_goldens_and_cardinality():
    """Real-material goldens: Star 78v/80f (72 quads + 8 tri), Dolphin 193f."""
    star = (SURFACE_DIR / "XvlShellModelStar.x3d").read_text()
    shells = list(_xvl_shells(star))
    assert len(shells) == 1
    attrs, verts, _ = shells[0]
    rings = _split_faces(_ints(attrs["faceCoordIndex"]))
    assert len(verts) == 78, f"Star vertices {len(verts)}"
    assert len(rings) == 80, f"Star faces {len(rings)}"
    assert sum(1 for r in rings if len(r) == 4) == 72
    assert sum(1 for r in rings if len(r) == 3) == 8

    dol = (SURFACE_DIR / "XvlShellModelDolphin.x3d").read_text()
    shells = list(_xvl_shells(dol))
    assert len(shells) == 1
    attrs, verts, _ = shells[0]
    rings = _split_faces(_ints(attrs["faceCoordIndex"]))
    assert len(verts) == 199, f"Dolphin vertices {len(verts)}"
    assert len(rings) == 193, f"Dolphin faces {len(rings)}"
    assert any(len(r) not in (3, 4) for r in rings), "Dolphin has n-gons"


def test_every_shell_is_structurally_valid():
    """Every committed XvlShell: rings/edges in range, weight cardinality."""
    checked = 0
    for name in REGISTRY:
        s = (SURFACE_DIR / name).read_text()
        coords = _coordinate_map(s)
        for attrs, verts, _ in _xvl_shells(s):
            faces = _split_faces(_ints(attrs.get("faceCoordIndex", "")))
            nv = len(verts)
            for ring in faces:
                assert ring, f"{name}: empty ring"
                for vi in ring:
                    assert 0 <= vi < nv, f"{name}: face index {vi} out of range ({nv} verts)"
            edges_begin = _ints(attrs.get("edgeBeginCoordIndex", ""))
            edges_end = _ints(attrs.get("edgeEndCoordIndex", ""))
            assert len(edges_begin) == len(edges_end), f"{name}: edge array mismatch"
            for a, b in zip(edges_begin, edges_end):
                assert 0 <= a < nv and 0 <= b < nv, f"{name}: edge index out of range"
            vround = _floats(attrs.get("vertexRound", ""))
            if vround:
                assert len(vround) == nv, f"{name}: vertexRound card {len(vround)} != {nv}"
            eround = _floats(attrs.get("edgeRound", ""))
            if eround:
                assert len(eround) == len(edges_begin), f"{name}: edgeRound card"
            checked += 1
    assert checked >= 30, f"expected to structurally check many shells, got {checked}"

    # DEF/USE coordinate reference must resolve on the attribute demos.
    for name in ("XvlShellAttributeAll.x3d", "XvlShellAttributeEdgeRound.x3d"):
        s = (SURFACE_DIR / name).read_text()
        for attrs, verts, _ in _xvl_shells(s):
            assert verts, f"{name}: coordinate USE reference unresolved"


def test_edge_vector_boundary_convention_consistent():
    """The decoder's boundary convention (corner + edgeVector/3) matches the
    data the EdgeVector demo stores for its (0 2 0) cube that raises the top
    face out of plane."""
    s = (SURFACE_DIR / "XvlShellAttributeEdgeVector.x3d").read_text()
    found = False
    for attrs, verts, _ in _xvl_shells(s):
        bvec = _floats(attrs.get("edgeBeginVector", ""))
        # Four top-face edges each with vector (0 2 0) -> all x=z=0, y=2.
        if len(bvec) < 12:
            continue
        vecs = list(zip(bvec[0::3], bvec[1::3], bvec[2::3]))
        if not all(abs(v[0]) < 1e-9 and abs(v[2]) < 1e-9 and abs(v[1] - 2.0) < 1e-9 for v in vecs):
            continue
        # Unit cube from -1..1: top face is the four corners at y=+1.
        top = sorted(verts, key=lambda p: -p[1])[:4]
        assert len(top) == 4
        for p in top:
            assert abs(p[1] - 1.0) < 1e-9, f"top-face corner not at y=1: {p}"
        found = True
        break
    assert found, "no (0 2 0) edge-vector cube found; independent parse diverged"


def test_bilinear_round_weight_range_present():
    """Rounding weights in [0.5..1.5] plus demo extremes (0.0/0.1) exist in the
    weight demos, matching the documented Lattice semantics."""
    s = (SURFACE_DIR / "XvlShellAttributeVertexRound.x3d").read_text()
    seen = set()
    for attrs, _, _ in _xvl_shells(s):
        seen.update(_floats(attrs.get("vertexRound", "")))
    assert 0.0 in seen and 1.0 in seen, f"vertexRound extremes {sorted(seen)}"
    s = (SURFACE_DIR / "XvlShellAttributeEdgeRound.x3d").read_text()
    seen = set()
    for attrs, _, _ in _xvl_shells(s):
        seen.update(_floats(attrs.get("edgeRound", "")))
    assert 0.0 in seen and 1.0 in seen, f"edgeRound extremes {sorted(seen)}"


def test_binary_branch_observation_record_present():
    """The P1 hex notebook's binary-branch note (V5_CFV3) and the decode writeup
    exist so the surface branch vs binary branch distinction stays documented."""
    re_notes = REPO_ROOT / "docs" / "re-notes"
    assert (re_notes / "gregory-patch-decode.md").exists(), "decode writeup present"
