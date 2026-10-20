"""NAS object-store mirror for P8 vault blobs (P8-M1-T02).

Reuses the P1-M4-T01 content-addressed mirror pattern
(``python/oep_tooling/objectstore.py``) for the vault's blob directory: each
blob lives under ``<blobs-dir>/<sha256>`` (content address = lowercase hex
sha-256 of the bytes, prepared by the Rust ``oep_pdm::blobstore``), and the
mirror uploads it to the NAS Gitea generic package registry content-addressed
by the SAME sha-256 as the object name, idempotently (an existing identical
object is a no-op, a 409-conflict on a content-addressed name is re-verified
as an already-landed identical upload), and proves integrity by fetching every
blob back and re-hashing against its address.

Endpoints (gitea skill, verified Oct 15, 2026):
  upload:   PUT  {base}/api/packages/{owner}/generic/{pkg}/{version}/{object}
  download: GET  {base}/api/packages/{owner}/generic/{pkg}/{version}/{object}

Tolerance (P2-M1-T04 doctrine): NAS down -> probe() False -> callers classify
as SKIP, never a crash. A failed mirror leaves a committed ``pending`` row and
an explicit reason, never a silent gap.

A later P12 consolidation may fold this module and ``oep_tooling.objectstore``
into one NAS sync backend; today the vault lane stays self-contained so the
P8-M1-T02 PR does not depend on the corpus-lane module's branch state.
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
DEFAULT_PACKAGE = "isolith-vault"
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
    """False if the NAS object store does not answer (tolerant, never raises).

    A 404 (package/version not yet created) still counts as *reachable*: the
    first mirror upload auto-creates the package (Gitea generic packages), so
    a fresh vault package must not mask the store being up. Only transport
    failure / 5xx mean the store is down.
    """
    try:
        req = urllib.request.Request(
            "%s/api/v1/packages/%s/generic/%s/%s" % (
                GITEA_BASE, DEFAULT_OWNER, DEFAULT_PACKAGE, DEFAULT_VERSION),
            headers=_headers())
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status == 200
    except urllib.error.HTTPError as e:
        return e.code == 404  # store answered; package not uploaded yet
    except Exception:
        return False


def _is_valid_address(s):
    return len(s) == 64 and all(c.isdigit() or "a" <= c <= "f" for c in s)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def blob_addresses(blobs_dir):
    """Content addresses present in a vault blobs directory (local, no NAS).

    Every entry is validated as a 64-char lowercase-hex address (the Rust
    BlobStore writes exactly those file names); anything else is reported as a
    corrupt/foreign row so a bad blob dir is surfaced, not silently mirrored.
    Returns sorted [(address, ok, detail)].
    """
    out = []
    if not os.path.isdir(blobs_dir):
        return out
    for name in sorted(os.listdir(blobs_dir)):
        path = os.path.join(blobs_dir, name)
        if not os.path.isfile(path):
            continue
        if not _is_valid_address(name):
            out.append((name, False, "not a 64-hex content address"))
            continue
        if sha256_file(path) != name:
            out.append((name, False, "contents do not hash to the address"))
        else:
            out.append((name, True, "ok"))
    return out


def download(owner, package, version, obj, timeout=60):
    """GET a blob from the NAS object store. Returns (bytes, err)."""
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
    """True if the object already exists on the store AND hash-matches."""
    data, err = download(owner, package, version, obj, timeout=timeout)
    if err or data is None:
        return False
    return sha256_bytes(bytes(data)) == obj


def upload_file(owner, package, version, obj, local_path, timeout=120):
    """PUT a local blob. Returns (ok, detail).

    Gitea generic packages reject urllib's default request Content-Type
    ('request Content-Type isn't multipart/form-data'); send an explicit
    application/octet-stream (verified: returns 201).

    409-safe idempotency (P1-M1-T05/P1-M4-T01 shared doctrine): a conflict on
    a content-addressed name races an identical prior upload, so on 409 we
    re-verify the object exists and hash-matches before calling it OK.
    """
    with open(local_path, "rb") as fh:
        body = fh.read()
    if sha256_bytes(bytes(body)) != obj:
        return False, "local blob does not hash to its address (refusing upload)"
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
            if exists(owner, package, version, obj, timeout=timeout):
                return True, "HTTP 409 (already on store, verified hash-match)"
            return False, "HTTP 409 (object absent after race)"
        return False, "HTTP %d: %s" % (e.code, e.reason)
    except Exception as e:
        return False, "network error: %r" % (e,)


def mirror(blobs_dir, index_path, owner=DEFAULT_OWNER,
           package=DEFAULT_PACKAGE, version=DEFAULT_VERSION):
    """Upload every valid blob in a vault blobs dir, content-addressed by its
    sha-256, and write the committed mirror index. Returns (index, results)
    where results is a list of (address, ok, detail).

    Tolerant: an unreachable store leaves ``pending`` rows with reasons --
    callers must gate on probe() for live acceptance, and treat the committed
    index as the proof of what actually landed.
    """
    rows = blob_addresses(blobs_dir)
    entries = []
    results = []
    for addr, ok_local, detail in rows:
        if not ok_local:
            results.append((addr, False, detail))
            entries.append({"address": addr, "status": "pending",
                            "detail": detail})
            continue
        if exists(owner, package, version, addr):
            results.append((addr, True, "already on store (dedup)"))
            entries.append({"address": addr, "status": "uploaded",
                            "detail": "dedup"})
            continue
        ok, detail2 = upload_file(owner, package, version, addr,
                                  os.path.join(blobs_dir, addr))
        results.append((addr, ok, detail2))
        entries.append({"address": addr,
                        "status": "uploaded" if ok else "pending",
                        "detail": detail2})
    entries.sort(key=lambda e: e["address"])
    index = {
        "schema_version": 1,
        "description": ("P8-M1-T02 vault blob mirror index -- every vault blob "
                        "is content-addressed on the NAS object store by its "
                        "sha-256 object name; status=uploaded means verified "
                        "present, pending means not (with a real reason)."),
        "owner": owner, "package": package, "version": version,
        "entries": entries,
    }
    with open(index_path, "w", encoding="utf-8") as fh:
        json.dump(index, fh, indent=2)
        fh.write("\n")
    return index, results


def verify_mirror(index_path, timeout=60):
    """GET every uploaded mirror blob back and assert its sha-256 matches the
    address (P1-M4-T01 checksum-verification semantics, live NAS only). Returns
    (ok_count, [(address, ok, detail)])."""
    with open(index_path, encoding="utf-8") as fh:
        idx = json.load(fh)
    owner = idx.get("owner", DEFAULT_OWNER)
    package = idx.get("package", DEFAULT_PACKAGE)
    version = idx.get("version", DEFAULT_VERSION)
    out = []
    ok_count = 0
    for e in idx.get("entries", []):
        if e.get("status") != "uploaded":
            continue
        data, err = download(e.get("owner", owner), e.get("package", package),
                             e.get("version", version), e["address"],
                             timeout=timeout)
        if err or data is None:
            out.append((e["address"], False, "fetch failed: %s" % err))
            continue
        real = sha256_bytes(bytes(data))
        ok = real == e["address"]
        if ok:
            ok_count += 1
        out.append((e["address"], ok,
                    ("expected %s got %s" % (e["address"], real)) if not ok else "ok"))
    return ok_count, out
