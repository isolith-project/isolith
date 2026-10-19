#!/usr/bin/env python3
"""P4-M4-T01 acceptance test: QIF basic-cut round trip + syntax validation.

Verifies the oep-qif basic cut (MBD quality data) against REAL QIF 3.0
material, INDEPENDENTLY of the Rust writer:

  1. ROUND TRIP (Rust integration test, `crates/oep-qif/tests/qif_roundtrip.rs`)
     imports the real NIST CTC-01 ASME1 AP242 MBD fixture into the model,
     exports normalized QIF, re-imports, and asserts model equality. The
     exported canonical bytes are byte-locked to
     `crates/oep-qif/tests/golden/nist-mbd-basic-export.qif`.

  2. SYNTAX VALIDATION (this file): the committed fixture AND the exported
     golden are validated against the OFFICIAL QIF 3.0 XSD
     (QIFDocument.xsd + the QIFApplications/QIFLibrary tree from
     qifstandards.org). This leg RUNS when lxml is present AND the schema is
     available at $OEP_QIF_XSD_ROOT (see python/oep_tooling/qif_xsd.py);
     otherwise it SKIPs loudly with the fetch command. Independent: re-parses
     the golden with stdlib xml.etree and asserts the basic-cut content
     (3 datums A/B/C, 14 characteristics across 6 GD&T kinds, mm unit,
     ASME Y14.5 standard, 14 nominals).
"""
import os
import sys
import xml.etree.ElementTree as ET

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(REPO, "crates", "oep-qif", "tests", "golden")

QIF_NS = "http://qifstandards.org/xsd/qif3"
NS = {"q": QIF_NS}


def golden(name):
    p = os.path.join(GOLDEN, name)
    assert os.path.isfile(p), "missing golden %s" % p
    return p


def test_independent_reparse_of_exported_golden():
    """Re-parse the committed export (independent of the Rust writer)."""
    p = golden("nist-mbd-basic-export.qif")
    root = ET.parse(p).getroot()
    assert root.tag == "{%s}QIFDocument" % QIF_NS
    assert root.get("versionQIF") == "3.0.0"

    datums = root.findall(".//q:DatumDefinition", NS)
    labels = sorted(d.findtext("q:DatumLabel", default="", namespaces=NS) for d in datums)
    assert labels == ["A", "B", "C"], labels

    defs = root.findall(".//q:CharacteristicDefinitions/*", NS)
    assert len(defs) == 14, len(defs)
    kinds = {d.tag.rsplit("}", 1)[-1].replace("CharacteristicDefinition", "")
            for d in defs}
    assert kinds == {"Position", "SurfaceProfile", "Perpendicularity",
                     "Flatness", "Diameter", "Angle"}, kinds

    noms = root.findall(".//q:CharacteristicNominals/*", NS)
    assert len(noms) == 14, len(noms)

    # primary linear unit is mm
    unit = root.findtext(".//q:PrimaryUnits/q:LinearUnit/q:UnitName", default="",
                         namespaces=NS)
    assert unit == "mm", unit

    # ASME Y14.5 standard carried
    org = root.findtext(".//q:StandardsDefinitions/q:Standard/q:Organization/"
                        "q:StandardsOrganizationEnum", default="", namespaces=NS)
    desig = root.findtext(".//q:StandardsDefinitions/q:Standard/q:Designator",
                          default="", namespaces=NS)
    assert org == "ASME" and "Y14.5" in desig, (org, desig)


def test_exported_golden_validates_against_official_xsd():
    """The exported output is QIF-syntactically valid (official XSD).

    SKips (loudly, with the one-liner to fetch the official schema) when the
    schema bundle is absent or lxml is unavailable -- same convention as the
    real-material legs in the corpus lane.
    """
    try:
        sys.path.insert(0, os.path.join(REPO, "python"))
        from oep_tooling import qif_xsd
    except ImportError as e:  # pragma: no cover - env dependent
        print("SKIP XSD leg: %s" % e)
        return
    fmt = (
        "SKIP XSD leg: official QIF 3.0 schema not on disk; run "
        "`python -m oep_tooling.qif_xsd fetch <dest>` and set "
        "OEP_QIF_XSD_ROOT=<dest>/xsd"
    )
    if qif_xsd.schema_root() is None:
        print(fmt)
        return
    errs = qif_xsd.validate(golden("nist-mbd-basic-export.qif"))
    assert not errs, "exported golden failed official XSD validation:\n%s" % "\n".join(errs[:8])


def test_source_fixture_is_official_xsd_valid():
    """The committed REAL material is itself a valid QIF 3.0 document."""
    try:
        sys.path.insert(0, os.path.join(REPO, "python"))
        from oep_tooling import qif_xsd
    except ImportError as e:  # pragma: no cover - env dependent
        print("SKIP fixture XSD leg: %s" % e)
        return
    if qif_xsd.schema_root() is None:
        print("SKIP fixture XSD leg: OEP_QIF_XSD_ROOT not set")
        return
    errs = qif_xsd.validate(golden("nist_ctc_01_asme1_ap242.qif"))
    assert not errs, "source fixture failed official XSD validation:\n%s" % "\n".join(errs[:8])


if __name__ == "__main__":
    test_independent_reparse_of_exported_golden()
    test_exported_golden_validates_against_official_xsd()
    test_source_fixture_is_official_xsd_valid()
    print("QIF acceptance: round-trip golden reparse + XSD legs PASS (or benign SKIP)")
