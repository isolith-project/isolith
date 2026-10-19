"""QIF 3.0 official-schema fetch + XSD validation helper (P4-M4-T01).

QIF is a DMSC / ISO 23952:2020 XML standard published under a free-to-
implementers license. The official schema bundle (xsd/QIFApplications/*.xsd +
xsd/QIFLibrary/*.xsd) is downloadable directly from DMSC (the registration
form is optional; the zip asset URL is stable):

    https://qifstandards.org/wp-content/uploads/2014/08/QIF3.0-2018-ANSI.zip

Per standards/qif/keelworks-use.md we do NOT re-host the bundle inside the
repo; tests that need real-schema validation are gated on the schema being
present on disk (OEP_QIF_XSD_ROOT), exactly like the real-material legs in
the corpus/lane tests. This module gives reproducible fetch + validation.

Usage:
    python -m oep_tooling.qif_xsd fetch <dest-dir>
        # downloads the official zip, extracts the xsd/ tree into <dest-dir>
    python -m oep_tooling.qif_xsd validate <qif-file> [<schema-root>]
        # exits 0 if <qif-file> validates against the QIFDocument.xsd tree
        # (schema root defaults to $OEP_QIF_XSD_ROOT)
"""
from __future__ import annotations

import hashlib
import os
import sys
import urllib.request
import zipfile

# Official static asset (verified 200 on 2026-10-16).
QIF3_ZIP_URL = "https://qifstandards.org/wp-content/uploads/2014/08/QIF3.0-2018-ANSI.zip"
QIF3_ZIP_SHA256 = "86cd30daf5d690814026ca39a4f0fcf3af6d147ada941916e6181b9bc81fd0d9"
QIF_NS = "http://qifstandards.org/xsd/qif3"


def schema_root() -> str | None:
    """Schema root from $OEP_QIF_XSD_ROOT when set and present on disk."""
    root = os.environ.get("OEP_QIF_XSD_ROOT")
    if root and os.path.isfile(os.path.join(root, "QIFApplications", "QIFDocument.xsd")):
        return root
    return None


def validate(qif_path: str, xsd_root: str | None = None) -> list[str]:
    """Validate `qif_path` against the official QIF 3.0 XSD.

    Returns a list of validation errors (empty == valid). Raises ImportError
    when lxml is unavailable -- callers gate on that (the repo keeps pyproject
    deps empty; lxml is an optional dev tool).
    """
    from lxml import etree  # optional dev tool, not a repo dependency

    root = xsd_root or schema_root()
    if root is None:
        raise RuntimeError(
            "official QIF 3.0 schema not present; set OEP_QIF_XSD_ROOT or run "
            "`python -m oep_tooling.qif_xsd fetch <dest>`"
        )
    class _Resolver(etree.Resolver):
        def resolve(self, url, pubid, context):
            base = os.path.basename(url)
            for d in ("QIFApplications", "QIFLibrary"):
                p = os.path.join(root, d, base)  # type: ignore[union-attr, unused-ignore]
                if os.path.exists(p):
                    return self.resolve_filename(p, context)
            return None

    parser = etree.XMLParser()
    parser.resolvers.add(_Resolver())
    xsdoc = etree.parse(os.path.join(root, "QIFApplications", "QIFDocument.xsd"), parser)
    xsd = etree.XMLSchema(xsdoc)
    doc = etree.parse(qif_path)
    if xsd.validate(doc):
        return []
    return [str(e) for e in xsd.error_log]


def fetch(dest: str) -> str:
    """Download the official QIF 3.0 bundle and extract the xsd/ tree.

    Returns the schema root (`<dest>/xsd`). Prints extracted file list.
    """
    os.makedirs(dest, exist_ok=True)
    zip_path = os.path.join(dest, "QIF3.0-2018-ANSI.zip")
    print(f"downloading {QIF3_ZIP_URL} -> {zip_path}")
    urllib.request.urlretrieve(QIF3_ZIP_URL, zip_path)  # noqa: S310 (public standard asset)
    with open(zip_path, "rb") as fh:
        sha = hashlib.sha256(fh.read()).hexdigest()
    print(f"sha256 {sha}")
    with zipfile.ZipFile(zip_path) as z:
        for n in z.namelist():
            if n.startswith("xsd/"):
                z.extract(n, dest)
    return os.path.join(dest, "xsd")


def _main(argv: list[str]) -> int:
    if len(argv) >= 1 and argv[0] == "fetch":
        root = fetch(argv[1] if len(argv) > 1 else ".")
        print(f"schema root: {root}")
        return 0
    if len(argv) >= 1 and argv[0] == "validate":
        qif = argv[1]
        root = argv[2] if len(argv) > 2 else schema_root()
        errs = validate(qif, root)
        if not errs:
            print(f"VALID: {qif}")
            return 0
        print(f"INVALID: {qif}")
        for e in errs[:20]:
            print("  -", e)
        return 1
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(_main(sys.argv[1:]))
