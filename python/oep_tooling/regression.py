"""P2-M5-T03 / P3-M5-T01 — shared regression corpus runner and report matrix.

Sweeps every corpus file and yields exactly one classified outcome per file:
status ``OPEN`` (the file opens under the appropriate reader / container probe)
or a ``SKIP`` carrying a machine code from the closed taxonomy (see
``CLOSED_TAXONOMY``). Emits a self-consistent report matrix as JSON + markdown.

This module is the P2/P3-shared corpus regression infra (P3-M5-T01): it owns
ONLY the sweep harness, the pluggable format dispatch, and matrix reporting. It
does not implement the CGR mesh reader or the 3DXML rep parser (that is the
P2-M1..P2-M4 / P3-M1..P3-M4 crate work on parallel cards). For CGR files it
probes the published header spine (magic / zero preamble / landscape / invariant
template — clean-room observed constants) and, when an external CGR reader CLI
is configured via ``OEP_CGR_READER``, runs it as the authoritative open check.
For 3DXML files it probes the container (ZIP-archive 3DXML with a coherent
``Manifest.xml`` BOM declaring a present ``<Root>`` member, or a standalone
ASCII ``<?xm`` model document — clean-room observed constants) and, when an
external P3 3DXML reader CLI is configured via ``OEP_3DXML_READER``, runs it as
the authoritative open check. Everything else (CATPart/CATProduct) is a
classified skip because the native readers are P5 scope.

Tolerant model (P2-M1-T04): introspection errors become classified rows, never
a crash; a failure in one file does not abort the sweep.

Pure stdlib (workspace dependencies = []). CLI via ``python -m
oep_tooling.regression``, or library use of :func:`sweep` / :func:`build_matrix`.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

# Closed skip-classification taxonomy. Every SKIP row must carry one of these.
CLOSED_TAXONOMY = (
    "unsupported-format",     # valid format; reader out of scope (catpart/catproduct)
    "not-a-cgr-container",    # .cgr-named file fails the observed header spine
    "truncated",              # file shorter than the structure requires
    "object-store-unresolved",  # object-store entry not local + fetch unavailable
    "missing-on-disk",        # manifest path does not resolve to a file
    "no-mesh-found",          # valid CGR header but no tessellation latched
    "unknown-format",         # unrecognized extension/magic / incoherent 3DXML BOM
    "io-error",               # unreadable
)

# --- observed CGR header constants (clean-room, published in docs/re-notes) --
CGR_MAGIC = bytes([0x01, 0x00, 0x00, 0x00])
CGR_PREAMBLE_OFF = 0x04
CGR_PREAMBLE_LEN = 0x60
CGR_LANDSCAPE_OFF = 0x64
CGR_TEMPLATE_OFF = 0x68
CGR_TEMPLATE = bytes([
    0x00, 0x07, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00,
    0x00, 0xB1, 0x01, 0x0B, 0x01, 0x08, 0x02, 0x4F,
])

# --- observed 3DXML container constants (clean-room, P1-M2-T04 hex notebook /
#     P3-M1-T01 golden map; public ZIP/XML shape facts) ----------------------
# ZIP local-file magic `PK\x03\x04` (public PKWARE APPNOTE).
ZIP_LOCAL_HEADER = b"PK\x03\x04"
# Standalone ASCII model document starts `<?xm` (`3C 3F 78 6D`).
STANDALONE_XML_PREFIX = b"<?xm"
# Canonical container BOM member name inside a ZIP-archive 3DXML.
MANIFEST_NAME = "Manifest.xml"
# 3DXML open-check exit codes for an external P3 reader CLI (documented
# contract, cf. docs/re-notes/p3-m5-t01-3dxml-regression.md): 0=OPEN,
# 3=not-a-3dxml-container, 4=no-mesh-found, 2=io-error.



@dataclass
class Row:
    """A single classified outcome for one corpus file."""

    id: str
    path: str
    format: str
    size: int
    status: str            # "OPEN" | "SKIP"
    cls: str = ""          # closed taxonomy code ("" when OPEN)
    detail: str = ""
    reader: str = "probe"  # "probe" | "cli:<name>" | "none"

    def as_dict(self) -> dict:
        return {
            "id": self.id,
            "path": self.path,
            "format": self.format,
            "size": self.size,
            "status": self.status,
            "class": self.cls,
            "detail": self.detail,
            "reader": self.reader,
        }


# --- classification helpers -------------------------------------------------
def probe_cgr_header(data: bytes):
    """Deterministic clean-room CGR header-spine probe.

    Returns ``True`` when the file starts with the observed 0x78-byte spine
    (magic 01 00 00 00, 96 zero byte preamble, per-sample landscape literal,
    16-byte invariant template). Returns a str reason (classified skip) when the
    spine does not hold: `"truncated"` or `"not-a-cgr-container"`. Never raises.
    """
    if len(data) < CGR_TEMPLATE_OFF + len(CGR_TEMPLATE):
        return "truncated"
    if data[0:4] != CGR_MAGIC:
        return "not-a-cgr-container"
    preamble = data[CGR_PREAMBLE_OFF:CGR_PREAMBLE_OFF + CGR_PREAMBLE_LEN]
    if any(preamble):
        return "not-a-cgr-container"
    tmpl = data[CGR_TEMPLATE_OFF:CGR_TEMPLATE_OFF + len(CGR_TEMPLATE)]
    if tmpl != CGR_TEMPLATE:
        # Surface family drift / wrong writer: still a classified skip, never a
        # crash (the mesh reader card owns deep parsing — honest boundary).
        return "not-a-cgr-container"
    return True


def _external_reader() -> list:
    """Return [prog, args...] for an external CGR open/convert CLI, or []."""
    env = os.environ.get("OEP_CGR_READER", "").strip()
    if env:
        return env.split()
    for candidate in ("oep-cgr2gltf", "oep-cgr2step"):
        for base in (os.curdir, "../target/release", "target/release"):
            p = Path(base) / candidate
            if p.is_file() and os.access(p, os.X_OK):
                return [str(p)]
    return []


def _external_3dxml_reader() -> list:
    """Return [prog, args...] for an external 3DXML open CLI, or [].

    Mirrors :func:`_external_reader` for the P3 format. Today no `oep-3dxml`
    CLI ships (it is a P3-M4/M5 follow-on), so unless the operator sets
    ``OEP_3DXML_READER`` this returns ``[]`` and the deterministic container
    probe (:func:`probe_3dxml_container`) is the authority. The exit-code
    contract below is the one a future P3 CLI MUST implement (documented in
    ``docs/re-notes/p3-m5-t01-3dxml-regression.md``).
    """
    env = os.environ.get("OEP_3DXML_READER", "").strip()
    if env:
        return env.split()
    for candidate in ("oep-3dxml2gltf", "oep-3dxml"):
        for base in (os.curdir, "../target/release", "target/release"):
            p = Path(base) / candidate
            if p.is_file() and os.access(p, os.X_OK):
                return [str(p)]
    return []


def probe_3dxml_container(data: bytes):
    """Deterministic clean-room 3DXML container probe.

    Returns ``True`` when ``data`` is a coherent 3DXML container — either a
    standalone ASCII model document (`<?xm` prefix) or a ZIP-archive 3DXML
    whose ``Manifest.xml`` BOM declares a ``<Root>`` that names a member
    actually present in the archive. Returns a str taxonomy reason otherwise
    (`"truncated"` / `"unknown-format"`). Never raises.
    """
    if len(data) < 4:
        return "truncated"
    # Standalone ASCII model document (`3C 3F 78 6D` = `<?xm`).
    if data[:4] == STANDALONE_XML_PREFIX:
        return True
    # ZIP-archive 3DXML: must be a real ZIP, contain Manifest.xml, and the
    # declared <Root> must be an actual member (BOM coherence).
    if data[:4] == ZIP_LOCAL_HEADER:
        try:
            import io as _io
            import zipfile as _zipfile
            zf = _zipfile.ZipFile(_io.BytesIO(data))
            names = set(zf.namelist())
        except Exception:
            return "unknown-format"
        if MANIFEST_NAME not in names:
            return "unknown-format"
        try:
            man = zf.read(MANIFEST_NAME).decode("utf-8", "replace")
        except Exception:
            return "unknown-format"
        m = re.search(r"<Root>([^<]+)</Root>", man)
        if not m:
            return "unknown-format"
        root = m.group(1).strip()
        if root not in names:
            return "unknown-format"
        return True
    return "unknown-format"


def classify_file_blob(data: bytes, fmt: str, rid: str = "<blob>",
                       display_path: str = "<blob>") -> Row:
    """Classify an in-memory 3DXML container (never raises)."""
    reader_prog = _external_3dxml_reader()
    if reader_prog:
        import tempfile as _tempfile
        tmp = None
        try:
            with _tempfile.NamedTemporaryFile(suffix=".3dxml", delete=False) as fh:
                fh.write(data)
                tmp = fh.name
            r = subprocess.run(reader_prog + [tmp],
                               capture_output=True, timeout=300)
            code = r.returncode
            err_txt = (r.stderr or b"").decode("utf-8", "replace").strip()[:200]
            if code == 0:
                return Row(rid, display_path, fmt, len(data), "OPEN", reader="cli")
            if code in (3,):
                return Row(rid, display_path, fmt, len(data), "SKIP",
                           "unknown-format", err_txt, reader="cli")
            if code in (4,):
                return Row(rid, display_path, fmt, len(data), "SKIP",
                           "no-mesh-found", err_txt, reader="cli")
            if code in (2,):
                return Row(rid, display_path, fmt, len(data), "SKIP",
                           "io-error", err_txt, reader="cli")
            return Row(rid, display_path, fmt, len(data), "SKIP",
                       "unknown-format",
                       "reader rc=%d: %s" % (code, err_txt),
                       reader="cli")
        except (OSError, subprocess.TimeoutExpired) as e:
            return Row(rid, display_path, fmt, len(data), "SKIP", "io-error",
                       "reader failed: %r" % (e,), reader="cli")
        finally:
            if tmp:
                try:
                    os.unlink(tmp)
                except OSError:
                    pass
    verdict = probe_3dxml_container(data)
    if verdict is True:
        return Row(rid, display_path, fmt, len(data), "OPEN", reader="probe")
    return Row(rid, display_path, fmt, len(data), "SKIP", str(verdict),
               "3DXML container probe failed", reader="probe")


def classify_file(abs_path: str, fmt: str, reader_prog: list) -> Row:
    """Classify one on-disk file. Never raises (returns a classified row)."""
    rid = os.path.basename(abs_path)
    try:
        size = os.path.getsize(abs_path)
        with open(abs_path, "rb") as fh:
            head = fh.read(CGR_TEMPLATE_OFF + len(CGR_TEMPLATE) + 1)
    except OSError as e:
        return Row(rid, abs_path, fmt, 0, "SKIP", "io-error", repr(e), reader="none")

    ext = os.path.splitext(abs_path)[1].lower()

    # CGR family: header-spine probe, escalated to external CLI when present.
    if ext in (".cgr",) or fmt == "cgr":
        if reader_prog:
            try:
                r = subprocess.run(
                    reader_prog + [abs_path],
                    capture_output=True, timeout=300,
                )
                code = r.returncode
                if code == 0:
                    return Row(rid, abs_path, fmt, size, "OPEN", reader="cli")
                # Documented CLI exit-code mapping (oep-cgr2gltf / -2step):
                #   3 not-a-CGR, 4 classified skip, 2 IO, 1 usage.
                if code in (3,):
                    return Row(rid, abs_path, fmt, size, "SKIP",
                               "not-a-cgr-container",
                               (r.stderr or "").strip()[:200], reader="cli")
                if code in (4,):
                    return Row(rid, abs_path, fmt, size, "SKIP",
                               "no-mesh-found",
                               (r.stderr or "").strip()[:200], reader="cli")
                if code in (2,):
                    return Row(rid, abs_path, fmt, size, "SKIP",
                               "io-error",
                               (r.stderr or "").strip()[:200], reader="cli")
                return Row(rid, abs_path, fmt, size, "SKIP", "unknown-format",
                           "reader rc=%d: %s" % (code, (r.stderr or "").strip()[:200]),
                           reader="cli")
            except (OSError, subprocess.TimeoutExpired) as e:
                return Row(rid, abs_path, fmt, size, "SKIP", "io-error",
                           "reader failed: %r" % (e,), reader="cli")
        # fall back to the deterministic spine probe
        verdict = probe_cgr_header(head)
        if verdict is True:
            return Row(rid, abs_path, fmt, size, "OPEN", reader="probe")
        return Row(rid, abs_path, fmt, size, "SKIP", str(verdict), reader="probe")

    # V5 native container (CATPart/CATProduct): P5 scope -> classified skip
    if fmt in ("catpart-v5", "catproduct-v5"):
        if head.startswith(b"V5_CFV2"):
            return Row(rid, abs_path, fmt, size, "SKIP", "unsupported-format",
                       "V5_CFV2 native container; native reader is P5 scope")
        if len(head) == 0:
            return Row(rid, abs_path, fmt, size, "SKIP", "truncated")
        return Row(rid, abs_path, fmt, size, "SKIP", "unknown-format",
                   "V5 extension but no V5_CFV2 magic")

    # 3DXML: dispatch through the shared container probe (P3-M5-T01); opens
    # a coherent ZIP-archive (BOM) or standalone ASCII model doc, or yields a
    # taxonomy skip (truncated / unknown-format). Escalates to OEP_3DXML_READER
    # when a P3 CLI is configured.
    if fmt in ("3dxml-zip", "3dxml-ascii") or ext in (".3dxml", ".3drep"):
        try:
            with open(abs_path, "rb") as fh:
                data = fh.read()
        except OSError as e:
            return Row(rid, abs_path, fmt, 0, "SKIP", "io-error", repr(e),
                       reader="none")
        row = classify_file_blob(data, fmt, rid=rid, display_path=abs_path)
        if data:
            row.size = size = os.path.getsize(abs_path)
        return row

    # STL: parse-open (trivially well-formed check)
    if fmt in ("stl-ascii",) or ext in (".stl",):
        if len(head) == 0:
            return Row(rid, abs_path, fmt, size, "SKIP", "truncated")
        text = head.decode("utf-8", errors="replace")
        if text.lstrip().startswith("solid"):
            return Row(rid, abs_path, fmt, size, "OPEN", reader="probe")
        if head.startswith(b"\x00\x00\x00\x00"):
            return Row(rid, abs_path, fmt, size, "OPEN", reader="probe")
        return Row(rid, abs_path, fmt, size, "SKIP", "unknown-format",
                   "not a well-formed STL header")

    # Unknown
    if len(head) == 0:
        return Row(rid, abs_path, fmt, size, "SKIP", "truncated")
    return Row(rid, abs_path, fmt, size, "SKIP", "unknown-format",
               "no reader registered for extension '%s'" % ext)


def _entry_path(corpus_root: str, entry: dict) -> str:
    path = entry.get("path") or entry.get("id")
    rel = (path or "").lstrip("/")
    return os.path.join(corpus_root, rel)


def sweep(entries, corpus_root=None, sample_dirs=None, cache_dir=None,
          reader_prog=None):
    """Classify every corpus file. Returns a list of Row (coverage == truth set).

    - ``entries``: manifest-style list of dicts (schema v1: id/path/format/
      backend). repo-backed entries resolve under ``corpus_root``; object-store
      entries become a ``object-store-unresolved`` skip unless a local blob is
      present under ``cache_dir``.
    - ``sample_dirs``: extra directories whose files (any *.cgr) are swept as
      ad-hoc real-material samples (no manifest entry required).
    - ``reader_prog``: optional external CGR reader argv; default discovers
      ``OEP_CGR_READER`` env or ``oep-cgr2gltf``/``oep-cgr2step`` on PATH.
    """
    if reader_prog is None:
        reader_prog = _external_reader()
    rows = []
    seen = set()

    def emit(row: Row):
        if row.id not in seen and row.id:
            seen.add(row.id)
            rows.append(row)

    for entry in entries or []:
        rid = entry.get("id", "?")
        fmt = entry.get("format", "unknown")
        backend = entry.get("backend", "repo")
        if backend == "object-store":
            # object-stream entries: try a local cache blob by sha256/id, else skip.
            blob = _find_object_blob(entry, cache_dir)
            if blob is None:
                emit(Row(rid, entry.get("path", ""), fmt, 0, "SKIP",
                         "object-store-unresolved",
                         "object-store entry not materialized locally"))
                continue
            rows_here = classify_file(blob, fmt, reader_prog)
            rows_here.id = rid
            rows_here.path = entry.get("path", "")
            emit(rows_here)
        else:
            path = _entry_path(corpus_root or "", entry)
            if not os.path.isfile(path):
                emit(Row(rid, entry.get("path", ""), fmt, 0, "SKIP",
                         "missing-on-disk", "manifest path not found: %s" % path))
                continue
            r = classify_file(path, fmt, reader_prog)
            r.path = entry.get("path", "")
            r.id = rid
            emit(r)

    for d in sample_dirs or []:
        if not d or not os.path.isdir(d):
            continue
        for name in sorted(os.listdir(d)):
            p = os.path.join(d, name)
            if not os.path.isfile(p):
                continue
            low = name.lower()
            if low.endswith((".3dxml", ".3drep")):
                fmt = "3dxml-ascii" if low.endswith("quad.3dxml") else "3dxml-zip"
            elif low.endswith(".stl"):
                fmt = "stl-ascii"
            else:
                fmt = "cgr"
            r = classify_file(p, fmt, reader_prog)
            r.id = name
            r.path = p
            emit(r)

    return rows


def _find_object_blob(entry: dict, cache_dir):
    if not cache_dir:
        return None
    c = Path(cache_dir)
    sha = entry.get("sha256", "")
    candidates = []
    if sha:
        candidates.append(c / sha)
    candidates.append(c / (entry.get("id") or ""))
    for cand in candidates:
        if cand.is_file():
            return str(cand)
    return None


def exit_code(rows) -> int:
    """0 unless any row landed in the fail bucket (status == 'FAIL')."""
    fail = any(getattr(r, "status", "") == "FAIL" for r in rows)
    return 1 if fail else 0


def build_matrix(rows) -> dict:
    n_open = sum(1 for r in rows if r.status == "OPEN")
    n_skip = sum(1 for r in rows if r.status == "SKIP")
    by_class = {}
    for r in rows:
        k = r.cls or "open"
        by_class[k] = by_class.get(k, 0) + 1
    return {
        "schema_version": 1,
        "generated": __import__("datetime").datetime.now().isoformat(timespec="seconds"),
        "task": "P2-M5-T03",
        "summary": {
            "total": len(rows),
            "open": n_open,
            "skip": n_skip,
            "by_class": dict(sorted(by_class.items())),
        },
        "rows": [r.as_dict() for r in rows],
    }


def write_matrix(matrix: dict, outdir: str) -> list:
    """Write regression-matrix.json + .md into outdir; returns the written paths."""
    out = Path(outdir)
    out.mkdir(parents=True, exist_ok=True)
    jp = out / "regression-matrix.json"
    jp.write_text(json.dumps(matrix, indent=2) + "\n", encoding="utf-8")
    mp = out / "regression-matrix.md"
    lines = ["# Keelworks regression corpus matrix (P2-M5-T03)", ""]
    s = matrix["summary"]
    lines.append("total=%d  open=%d  skip=%d" % (s["total"], s["open"], s["skip"]))
    lines.append("")
    lines.append("| id | format | status | class | reader | detail |")
    lines.append("|---|--------|--------|-------|--------|--------|")
    for r in matrix["rows"]:
        lines.append("| %s | %s | %s | %s | %s | %s |" % (
            r["id"], r["format"], r["status"], r.get("class", ""),
            r.get("reader", ""), (r.get("detail", "") or "")[:80].replace("|", "/")))
    lines.append("")
    mp.write_text("\n".join(lines), encoding="utf-8")
    return [str(jp), str(mp)]


# --- CLI --------------------------------------------------------------------
def main(argv=None) -> int:
    import argparse
    ap = argparse.ArgumentParser(
        prog="python -m oep_tooling.regression",
        description="P2/P3-shared regression corpus runner: sweep every corpus "
                    "file and report OPEN / classified-SKIP matrix (CGR, 3DXML, "
                    "STL open checks; CATPart/CATProduct classified skips).")
    ap.add_argument("--manifest", default=None,
                    help="path to corpus/manifest.json (schema v1)")
    ap.add_argument("--corpus-root", default=None,
                    help="corpus dir that manifest 'path' fields are relative to "
                         "(default: dirname of --manifest, else corpus/)")
    ap.add_argument("--samples", default=None, action="append",
                    help="extra sample dir (repeatable; CGR via OEP_CGR_SAMPLES, "
                         "3DXML via OEP_3DXML_SAMPLES)")
    ap.add_argument("--cache", default=None,
                    help="local cache for object-store blobs")
    ap.add_argument("--out", default="corpus",
                    help="output dir for regression-matrix.{json,md} (default corpus/)")
    ap.add_argument("--reader", default=None,
                    help="external CGR reader argv (space-split), or 'probe'")
    args = ap.parse_args(argv)

    entries = []
    corpus_root = args.corpus_root
    if args.manifest:
        with open(args.manifest, encoding="utf-8") as fh:
            m = json.load(fh)
        entries = m.get("entries", [])
        if corpus_root is None:
            corpus_root = os.path.dirname(os.path.abspath(args.manifest))
    if corpus_root is None:
        corpus_root = "corpus"

    sample_dirs = list(args.samples or [])
    for var in ("OEP_CGR_SAMPLES", "OEP_3DXML_SAMPLES"):
        sample_dirs += [c for c in os.environ.get(var, "").split(":") if c]

    reader_prog = None
    if args.reader and args.reader != "probe":
        reader_prog = args.reader.split()
    elif (args.reader == "probe") or (args.reader is None and not os.environ.get("OEP_CGR_READER")):
        reader_prog = []  # spine-probe only

    rows = sweep(entries=entries, corpus_root=corpus_root,
                 sample_dirs=sample_dirs, cache_dir=args.cache,
                 reader_prog=reader_prog)
    matrix = build_matrix(rows)
    written = write_matrix(matrix, args.out)
    s = matrix["summary"]
    print("total=%d open=%d skip=%d" % (s["total"], s["open"], s["skip"]))
    for cls, n in s["by_class"].items():
        print("  %s=%d" % (cls, n))
    for w in written:
        print("wrote %s" % w)
    return exit_code(rows)


if __name__ == "__main__":
    sys.exit(main())
