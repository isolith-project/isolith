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
import urllib.request

GITEA_BASE = "http://192.168.0.101:3200"
CREDS = os.path.expanduser("~/.git-credentials")
CACHE_SUBDIR = "cache"


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


def fetch_entry(entry, cache_dir=None):
    """Materialize an object-store registry entry into the corpus cache (or given
    cache_dir) and verify its sha256. Returns (local_path, None) or (None, err)."""
    ptr = entry.get("object_store")
    if not ptr:
        return None, "object-store entry missing object_store ptr: %s" % entry["id"]
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
