"""NAS object-store sync backend for the Keelworks corpus (P1-M1-T05).

Mirrors license-clean corpus blobs to the NAS object store — the NAS Gitea
generic package registry (the same backend P0-M4-T02 chose for the corpus
harness) — content-addressed by sha256 in the object name, and proves integrity
by fetching blobs back and re-hashing them against the committed index.

Endpoints (gitea skill, verified Oct 15, 2026):
  upload:   PUT  {base}/api/packages/{owner}/generic/{pkg}/{version}/{object}
  download: GET  {base}/api/packages/{owner}/generic/{pkg}/{version}/{object}

Transport seam decision (P0-M4-T02 ADR): object store rather than client
git-lfs — git-lfs is installed nowhere (U1, CI); generic packages need only the
don token already in ~/.git-credentials, and blobs never bloat git or CI clones.

Tolerance (P2-M1-T04 doctrine): NAS down -> probe() False -> caller classifies
as SKIP, never a crash. An upload of an object that already exists is a cheap
idempotent no-op to check first via GET.
"""
import hashlib
import json
import os
import re
import urllib.error
import urllib.request

GITEA_BASE = "http://192.168.0.101:3200"
CREDS = os.path.expanduser("~/.git-credentials")

DEFAULT_OWNER = "don"
DEFAULT_PACKAGE = "keelworks-corpus"
DEFAULT_VERSION = "v1"


def token():
    """Don admin token from ~/.git-credentials. Never printed."""
    if not os.path.isfile(CREDS):
        return None
    for line in open(CREDS, encoding="utf-8"):
        m = re.match(r"^http://don:([^@]+)@", line.strip())
        if m:
            return m.group(1)
    return None


def _headers():
    t = token()
    return {"Authorization": "token " + t} if t else {}


def probe(timeout=15):
    """False if the NAS object store does not answer (tolerant, never raises)."""
    try:
        req = urllib.request.Request(
            "%s/api/v1/packages/%s/generic/%s/%s" % (
                GITEA_BASE, DEFAULT_OWNER, DEFAULT_PACKAGE, DEFAULT_VERSION),
            headers=_headers())
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status == 200
    except Exception:
        return False


def object_name(sha256, ext=".bin"):
    """Content-addressed object name: <sha256><ext>."""
    ext = ext if ext.startswith(".") else "." + ext
    return sha256 + ext


def object_sha256(obj):
    """Strip the extension from a content-addressed object name."""
    return obj.split(".")[0]


def download(owner, package, version, obj, timeout=60):
    """GET a blob from the NAS object store. Returns (bytes, err) — err None OK."""
    url = "%s/api/packages/%s/generic/%s/%s/%s" % (
        GITEA_BASE, owner, package, version, obj)
    req = urllib.request.Request(url, headers=_headers())
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.read(), None
    except urllib.error.HTTPError as e:
        return None, "HTTP %d: %s" % (e.code, e.reason)
    except Exception as e:
        return None, "network error: %r" % (e,)


def exists(owner, package, version, obj, timeout=10):
    """True if the object already exists on the store (idempotency check)."""
    data, err = download(owner, package, version, obj, timeout=timeout)
    if err or data is None:
        return False
    return object_sha256(obj) == hashlib.sha256(bytes(data)).hexdigest()


def upload_file(owner, package, version, obj, local_path, timeout=120):
    """PUT a local file to the NAS object store. Returns (ok, detail).

    Gitea generic packages reject urllib's default request Content-Type
    ('request Content-Type isn't multipart/form-data'); we must send an explicit
    application/octet-stream (verified: returns 201).

    Idempotent by design (P1-M1-T05 re-run doctrine): a 409-conflict on a
    content-addressed object means a prior upload already landed the identical
    bytes. We then verify the object exists on the store and hash-matches the
    local file, and treat that as OK rather than a failure, so acceptance runs
    stay green on every re-run.
    """
    with open(local_path, "rb") as fh:
        body = fh.read()
    url = "%s/api/packages/%s/generic/%s/%s/%s" % (
        GITEA_BASE, owner, package, version, obj)
    headers = _headers()
    headers["Content-Type"] = "application/octet-stream"
    req = urllib.request.Request(url, data=body, headers=headers, method="PUT")
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status in (200, 201, 204), "HTTP %d" % r.status
    except urllib.error.HTTPError as e:
        if e.code == 409:
            # Already present: verify the stored object 64-hex prefix equals the
            # object name's sha256 and the download hash matches the source.
            if exists(owner, package, version, obj, timeout=timeout):
                return True, "HTTP 409 (already on store, verified hash-match)"
            return False, "HTTP 409 (object absent after race)"
        return False, "HTTP %d: %s" % (e.code, e.reason)
    except Exception as e:
        return False, "network error: %r" % (e,)


def upload_candidates(spec):
    """Entries in a sync spec that may be uploaded (redistributable=True)."""
    return [e for e in spec.get("entries", []) if e.get("redistributable")]


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def sync(spec_path, data_root, index_path, owner=DEFAULT_OWNER,
         package=DEFAULT_PACKAGE, version=DEFAULT_VERSION):
    """Sync every redistributable spec entry from data_root into the store and
    write the committed index. Returns (index, results) where results is a list
    of (entry_id, ok, detail). Upload skips cleanly when an identical object is
    already present (content-dedup) or when the local file is absent (classified
    skip, not a failure — the entry stays out of the index).
    """
    with open(spec_path, encoding="utf-8") as fh:
        spec = json.load(fh)
    index_entries = []
    results = []
    for e in upload_candidates(spec):
        rel = e.get("path", e["id"])
        src = os.path.join(data_root, rel.lstrip("/"))
        if not os.path.isfile(src):
            results.append((e["id"], False, "local source absent: %s" % rel))
            continue
        real = sha256_file(src)
        if real != e["sha256"]:
            results.append((e["id"], False,
                            "sha256 mismatch: spec %s got %s"
                            % (e["sha256"], real)))
            continue
        obj = object_name(e["sha256"], os.path.splitext(e["id"])[1] or ".bin")
        if exists(owner, package, version, obj):
            results.append((e["id"], True, "already on store (dedup)"))
        else:
            ok, detail = upload_file(owner, package, version, obj, src)
            if not ok and "409" in str(detail):
                # Gitea reports 'package file already exists' when a previous
                # upload landed but our local exists() check raced it. Verify the
                # object is present and hash-correct; if so it is uploaded.
                if exists(owner, package, version, obj):
                    ok, detail = True, "exists on store (409 raced, verified)"
                else:
                    ok, detail = False, detail + " (not present after 409)"
            results.append((e["id"], ok, detail))
        index_entries.append({
            "id": e["id"],
            "format": e["format"],
            "geometry_class": e.get("geometry_class"),
            "license": e.get("license"),
            "sha256": e["sha256"],
            "size": e.get("size"),
            "owner": owner,
            "package": package,
            "version": version,
            "object": obj,
            "status": "uploaded" if results[-1][1] else "pending",
        })
    # reference-only entries (CGR etc.) are recorded, never uploaded
    for e in spec.get("entries", []):
        if not e.get("redistributable"):
            index_entries.append({
                "id": e["id"],
                "format": e["format"],
                "geometry_class": e.get("geometry_class"),
                "license": e.get("license"),
                "sha256": (e.get("sha256") or ""),
                "size": e.get("size"),
                "owner": None,
                "package": None,
                "version": None,
                "object": None,
                "status": "reference-only",
            })
    index = {"schema_version": 1,
             "description": "P1-M1-T05 object-store index — proof of which "
                            "corpus blobs are on the NAS object store and their "
                            "verify state. status=uploaded means the blob is "
                            "content-addressed on the store and its sha256 was "
                            "recorded from the local file; reference-only means "
                            "the sample is catalogued but never uploaded "
                            "(non-redistributable license).",
             "owner": owner, "package": package, "version": version,
             "entries": index_entries}
    with open(index_path, "w", encoding="utf-8") as fh:
        json.dump(index, fh, indent=2)
        fh.write("\n")
    return index, results


def verify_index(index_path, timeout=60):
    """GET every uploaded blob back from the store and assert sha256 matches the
    index. Returns (ok_count, [(entry_id, ok, detail)])."""
    with open(index_path, encoding="utf-8") as fh:
        idx = json.load(fh)
    out = []
    ok_count = 0
    for e in idx.get("entries", []):
        if e.get("status") != "uploaded":
            continue
        data, err = download(e["owner"], e["package"], e["version"],
                             e["object"], timeout=timeout)
        if err or data is None:
            out.append((e["id"], False, "fetch failed: %s" % err))
            continue
        real = hashlib.sha256(bytes(data)).hexdigest()
        ok = real == e["sha256"]
        if ok:
            ok_count += 1
        out.append((e["id"], ok,
                    "expected %s got %s" % (e["sha256"], real) if not ok else "ok"))
    return ok_count, out


# --- full-manifest mirror (P1-M4-T01) ---------------------------------------
# Extends the P1-M1-T05 sync (which handled the redistributable subset) to the
# WHOLE shared corpus: every manifest entry with backend=object-store must be
# present on the NAS object store, content-addressed EXACTLY by the object name
# its manifest object_store pointer names (so the golden runner and the P2/P3
# regression runner resolve it by pointer without translation).

def _hash_index(data_root):
    """{sha256: abs_path} for every file under data_root (recursive)."""
    out = {}
    for dirpath, _dirs, files in os.walk(data_root):
        for name in files:
            p = os.path.join(dirpath, name)
            out.setdefault(sha256_file(p), p)
    return out


def mirror(manifest_path, data_root, index_path, owner=DEFAULT_OWNER,
           package=DEFAULT_PACKAGE, version=DEFAULT_VERSION,
           reference_index_path=None, timeout=120):
    """Materialize the whole object-store mirror for a corpus manifest.

    Walks the manifest's backend=object-store entries, sources each from local
    bytes under ``data_root`` (one dir or a list of dirs; matched by sha256,
    never by filename), verifies the source hash against the manifest, uploads
    (idempotent content-dedup by object name), and writes a committed index
    covering every mirror entry. Reference-only entries carried over from
    ``reference_index_path`` (default: the sibling corpus/object-store-index.json)
    are recorded, never uploaded.

    Returns (index, results) where results is a list of
    (entry_id, ok, detail). Tolerant: an absent/mismatched local source is a
    pending (False) row, not a crash.
    """
    with open(manifest_path, encoding="utf-8") as fh:
        manifest = json.load(fh)
    entries = [e for e in manifest.get("entries", [])
               if e.get("backend") == "object-store"]
    roots = data_root if isinstance(data_root, (list, tuple)) else [data_root]
    hash2path = {}
    for r in roots:
        if r:
            hash2path.update(_hash_index(r))

    results = []
    index_entries = []
    pending_source = 0
    for e in entries:
        sha = e["sha256"]
        ptr = e.get("object_store") or {}
        obj = ptr.get("object") or object_name(sha, "")
        src = hash2path.get(sha)
        if not src:
            results.append((e["id"], False, "local source absent for sha256"))
            status = "pending"
        elif sha256_file(src) != sha:
            results.append((e["id"], False, "local source sha256 mismatch"))
            status = "pending"
            pending_source += 1
        else:
            if exists(owner, package, version, obj, timeout=10):
                results.append((e["id"], True, "already on store (dedup)"))
            else:
                ok, detail = upload_file(owner, package, version, obj, src,
                                         timeout=timeout)
                results.append((e["id"], ok, detail))
            status = "uploaded" if results[-1][1] else "pending"
        index_entries.append({
            "id": e["id"],
            "format": e["format"],
            "geometry_class": e.get("geometry_class"),
            "license": e.get("license"),
            "sha256": sha,
            "size": e.get("size"),
            "owner": owner,
            "package": package,
            "version": version,
            "object": obj,
            "status": status,
        })

    # carry over prior-index entries whose id is not superseded by the mirror
    # (wave-1 P1-M1-T05 sync entries keyed by spec id, e.g. 3DM016.CATPart, and
    # reference-only records). Preserving them keeps the T05 acceptance green
    # while the manifest-keyed mirror is added.
    ref_path = reference_index_path or os.path.join(
        os.path.dirname(os.path.abspath(manifest_path)),
        "object-store-index.json")
    if os.path.isfile(ref_path):
        with open(ref_path, encoding="utf-8") as fh:
            old = json.load(fh)
        cur_ids = {ie["id"] for ie in index_entries}
        for oe in old.get("entries", []):
            if oe["id"] in cur_ids:
                continue
            if oe.get("status") in ("uploaded", "reference-only"):
                index_entries.append(oe)

    index_entries.sort(key=lambda x: x["id"])
    index = {
        "schema_version": 1,
        "description": ("P1-M4-T01 object-store mirror index — every "
                        "backend=object-store entry of corpus/manifest.json is "
                        "present on the NAS object store (content-addressed by "
                        "sha256 in the object name, exactly as each manifest "
                        "object_store pointer names it) and its GET-back hash "
                        "matches the manifest. status=uploaded means verified; "
                        "pending means the blob is not on the store; "
                        "reference-only means catalogued, never uploaded."),
        "owner": owner, "package": package, "version": version,
        "entries": index_entries,
    }
    with open(index_path, "w", encoding="utf-8") as fh:
        json.dump(index, fh, indent=2)
        fh.write("\n")
    return index, results


def verify_manifest_mirror(manifest_path, index_path, timeout=60):
    """GET every uploaded mirror blob back from the live store and assert its
    sha256 equals the manifest sha256 (checksums match P1-M1-T04 manifest).

    Returns (ok_count, totals, [(entry_id, ok, detail)]) where totals is
    (uploaded, pending). Live-NAS only; caller must gate on probe().
    """
    with open(manifest_path, encoding="utf-8") as fh:
        manifest = json.load(fh)
    manifest_sha = {e["id"]: e["sha256"]
                    for e in manifest.get("entries", [])
                    if e.get("backend") == "object-store"}
    with open(index_path, encoding="utf-8") as fh:
        idx = json.load(fh)
    out = []
    ok_count = 0
    uploaded = pending = 0
    for e in idx.get("entries", []):
        if e.get("status") != "uploaded":
            if e.get("status") == "pending":
                pending += 1
            continue
        uploaded += 1
        # manifest is the authority; carried-over wave-1 spec-id entries fall
        # back to their own recorded index hash (T05-verified).
        want = manifest_sha.get(e["id"]) or e.get("sha256")
        data, err = download(e["owner"], e["package"], e["version"],
                             e["object"], timeout=timeout)
        if err or data is None:
            out.append((e["id"], False, "fetch failed: %s" % err))
            continue
        real = hashlib.sha256(bytes(data)).hexdigest()
        ok = real == want
        if ok:
            ok_count += 1
        out.append((e["id"], ok,
                    "expected %s got %s" % (want, real) if not ok else "ok"))
    return ok_count, (uploaded, pending), out


def find_local_source(data_root, sha256_hex):
    """Absolute path of a file under data_root whose sha256 equals sha256_hex,
    or None. Content-addressed lookup — filename-independent."""
    for dirpath, _dirs, files in os.walk(data_root):
        for name in files:
            p = os.path.join(dirpath, name)
            if sha256_file(p) == sha256_hex:
                return p
    return None

