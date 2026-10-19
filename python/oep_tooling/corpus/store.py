"""NAS object store backend for the test-data registry.

Blobs live in the NAS Gitea generic package registry (the "NAS object store" in
P0-M4-T02 terms), content-addressed by sha256 in the object filename. Verified
against the live NAS instance (Oct 15, 2026): 10-200 MB uploads/downloads OK,
token auth works, `?name=` controls the attachment filename.

Endpoints (gitea skill):
  upload: PUT  {base}/api/packages/{owner}/generic/{package}/{version}/{object}
  download: GET {base}/api/packages/{owner}/generic/{package}/{version}/{object}

Resolution is tolerant on purpose (P2-M1-T04 doctrine: an unknown/absent object is
a classified skip with a warning, never a crash).
"""
import hashlib
import os
import re
import urllib.error
import urllib.parse
import urllib.request

GITEA_BASE = os.environ.get("OEP_GITEA_BASE", "http://192.168.0.101:3200")
CREDS = os.path.expanduser("~/.git-credentials")
CACHE_SUBDIR = "cache"
# Negative cache: once the object store base is observed unprovisioned (the
# generic package registry answers 404 for an absent owner/package), every
# later fetch short-circuits with the same classified skip instead of probing
# the network once per registry entry. Preserves the documented tolerant
# contract ("unknown/absent object is a classified skip with a warning, never
# a crash") while keeping a large manifest (122 object-store rows) fast in an
# environment where the mirror has not been materialized (e.g. U1 CI; NAS is
# repo/storage only per INTEGRATION-T02).
_PROBE_FAILURE = None


def token():
    """Read the don admin token from ~/.git-credentials. Never printed."""
    if not os.path.isfile(CREDS):
        return None
    for line in open(CREDS, encoding="utf-8"):
        m = re.match(r"^http://don:([^@]+)@", line.strip())
        if m:
            return m.group(1)
    return None


def object_store_url(ptr):
    """Build the download URL for an object_store pointer dict."""
    return "%s/api/packages/%s/generic/%s/%s/%s" % (
        GITEA_BASE, ptr["owner"], ptr["package"], ptr["version"], ptr["object"])


def _probe_store(ptr):
    """One bounded head request to the package base. Returns (ok, detail).

    A 404 / connection failure on the base means the store is not provisioned;
    that verdict is cached so a registry-wide resolve does one probe, not one
    probe per entry.
    """
    global _PROBE_FAILURE
    if _PROBE_FAILURE:
        return False, _PROBE_FAILURE
    base = "%s/api/packages/%s/generic" % (GITEA_BASE, urllib.parse.quote(ptr["owner"]))
    t = token()
    headers = {"Authorization": "token " + t} if t else {}
    try:
        req = urllib.request.Request(base, headers=headers, method="HEAD")
        with urllib.request.urlopen(req, timeout=10) as r:
            return True, r.status
    except urllib.error.HTTPError as e:
        if e.code in (404, 301, 302, 307, 308):
            detail = "object store base HTTP %d (mirror not provisioned)" % e.code
            _PROBE_FAILURE = detail
            return False, detail
        return True, "HTTP %d" % e.code
    except Exception as e:
        detail = "object store base unreachable: %r" % (e,)
        _PROBE_FAILURE = detail
        return False, detail


def _download(url, dest):
    """Download url to dest with the don token; returns (ok, detail)."""
    t = token()
    headers = {"Authorization": "token " + t} if t else {}
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            body = r.read()
    except urllib.error.HTTPError as e:
        return False, "HTTP %d: %s" % (e.code, e.reason)
    except Exception as e:
        return False, "network error: %r" % (e,)
    with open(dest, "wb") as fh:
        fh.write(body)
    return True, body


def object_store_unavailable():
    """Return the cached unavailability reason, or None when the store has not
    been probed (yet) as unavailable. Lets acceptance tests honor their
    documented 'skip when the mirror is not reachable' contract exactly:
    once every object-store fetch begins with a fast classified skip, the
    full-corpus census legs skip instead of asserting against a partial repo
    corpus."""
    return _PROBE_FAILURE


def fetch_entry(entry, cache_dir=None):
    """Materialize an object-store registry entry into the corpus cache (or given
    cache_dir) and verify its sha256. Returns (local_path, None) or (None, err)."""
    ptr = entry.get("object_store")
    if not ptr:
        return None, "object-store entry missing object_store ptr: %s" % entry["id"]
    ok, detail = _probe_store(ptr)
    if not ok:
        return None, "fetch skipped: %s" % detail
    cache_base = cache_dir or os.path.join(
        os.path.dirname(os.path.dirname(os.path.abspath(__file__))), CACHE_SUBDIR)
    os.makedirs(cache_base, exist_ok=True)
    # content-addressed cache key: sha256.object-suffix
    obj = ptr.get("object", "")
    fname = "%s.%s" % (entry["sha256"], obj.split(".")[-1] if "." in obj else "bin")
    dest = os.path.join(cache_base, fname)
    if os.path.isfile(dest):
        # spot-verify cached copy only (full verify on fresh download below)
        try:
            if hashlib.sha256(open(dest, "rb").read()).hexdigest() == entry["sha256"]:
                return dest, None
        except Exception:
            pass
    ok, detail = _download(object_store_url(ptr), dest)
    if not ok:
        return None, "fetch failed: %s" % detail
    real = hashlib.sha256(open(dest, "rb").read()).hexdigest()
    if real != entry["sha256"]:
        return None, "sha256 mismatch: expected %s got %s" % (entry["sha256"], real)
    return dest, None
