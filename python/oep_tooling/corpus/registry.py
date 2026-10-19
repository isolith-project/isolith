"""Shared test-data registry: locate + verify corpus entries from corpus/manifest.json.

Registry schema v1 (see corpus/README.md):

    schema_version: 1
    entries: [
      {
        id: unique slug
        path: path relative to corpus/ root (local layout of the file)
        format: machine format tag, e.g. "stl-ascii", "cgr", "3dxml"
        geometry_class: prism | surface | assembly | dataset | text
        source: provenance of the file
        license: SPDX id (clean-room: must be redistributable)
        size: bytes
        sha256: 64-hex content hash (integrity source of truth)
        backend: "repo" (committed under corpus/) | "object-store" (NAS blob)
        object_store: {owner, package, version, object}  # required when backend=object-store
      }
    ]
"""
import hashlib
import json
import os
import urllib.parse

from oep_tooling.corpus import store

DEFAULT_MANIFEST = "corpus/manifest.json"
SUPPORTED_BACKENDS = ("repo", "object-store")


def repo_root(path=None):
    """Find repo root: the directory containing corpus/manifest.json."""
    p = os.path.abspath(path or os.getcwd())
    while True:
        if os.path.isfile(os.path.join(p, DEFAULT_MANIFEST)):
            return p
        parent = os.path.dirname(p)
        if parent == p:
            return None
        p = parent


def load_manifest(repo=None):
    base = repo or os.getcwd()
    r = repo_root(base)
    if r is None:
        # base may itself be a repo root (manifest at <base>/corpus/manifest.json)
        if os.path.isfile(os.path.join(base, DEFAULT_MANIFEST)):
            r = base
    r = r or os.path.abspath(base)
    mpath = os.path.join(r, DEFAULT_MANIFEST)
    if not os.path.isfile(mpath):
        raise FileNotFoundError("registry manifest not found: %s" % mpath)
    with open(mpath, encoding="utf-8") as fh:
        data = json.load(fh)
    if data.get("schema_version") != 1:
        raise ValueError("unsupported corpus manifest schema_version=%r" % data.get("schema_version"))
    return {"manifest_path": mpath, "corpus_dir": os.path.join(r, "corpus"), "data": data}


def find_entry(manifest, entry_id):
    for e in manifest["data"].get("entries", []):
        if e["id"] == entry_id:
            return e
    return None


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def resolve_entry(entry, manifest=None, cache_dir=None):
    """Return (local_path, error). error is None on success. Repo entries resolve
    directly; object-store entries are materialized via NAS object store into the
    corpus cache (error if fetch/verify fails)."""
    manifest = manifest or load_manifest()
    if entry["backend"] == "repo":
        p = os.path.join(manifest["corpus_dir"], entry["path"].lstrip("/"))
        if not os.path.isfile(p):
            return None, "repo file absent: %s" % p
        return p, None
    if entry["backend"] == "object-store":
        cache_dir = cache_dir or os.path.join(manifest["corpus_dir"], "cache")
        return store.fetch_entry(entry, cache_dir=cache_dir)
    return None, "unknown backend: %s" % entry.get("backend")


def verify_entry_checksum(entry, manifest=None):
    """Verify the real on-disk sha256 of a repo-backed entry matches the manifest.
    Returns (ok, detail)."""
    path, err = resolve_entry(entry, manifest=manifest)
    if err:
        return False, err
    real = sha256_file(path)
    return real == entry["sha256"], "expected %s got %s" % (entry["sha256"], real)


def verify_repo_checksums(manifest=None):
    """Verify every repo-backed entry's real checksum. Returns list of
    (entry_id, ok, detail)."""
    manifest = manifest or load_manifest()
    out = []
    for e in manifest["data"].get("entries", []):
        if e["backend"] == "repo":
            ok, detail = verify_entry_checksum(e, manifest)
            out.append((e["id"], ok, detail))
        else:
            out.append((e["id"], True, "non-repo backend skipped"))
    return out


def format_for_upload(entry):
    """Internet Archive-friendly URL-safe identifier for an entry (later P1-M1-T05)."""
    return urllib.parse.quote(entry["id"], safe=".-_")
