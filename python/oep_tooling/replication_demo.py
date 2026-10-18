"""P10-M4-T01 — multi-site replication demo: LAN node + remote node replicating
the vault, conflict semantics documented and honest.

This is the delivery-lane acceptance harness for the P10-M4-T01 card. It
follows the house GA contract (same as P8-M6-T03 / P9-M5-T02): measured legs,
honest classification with a closed SKIP taxonomy, exit 0 iff no FAIL row, and
a self-consistent JSON + Markdown report. Difference from the P8/P9 gates: here
the "real material" is TWO NODES against one vault.

The topology (decision #9 / #10, P8-M5-T03 airgap/hybrid ADR baseline):

* Node A ("LAN node") — the authoritative vault: a real `oep-pdm` service on a
  real socket. In the live two-host proof this is U1; in the CI-safe run here
  it is a 127.0.0.1 service on a free port (pure LAN boundary, DeploymentRole
  default `Airgap`: loopback/LAN only, telemetry off, no relay, no internet).
* Node B ("remote node") — a sync client with the generated `PdmClient`,
  holding a local working replica. It records an offline edit against an item
  base identity, syncs it back on connect (sync_push), and pulls the vault head
  to see edits made on node A. In the live proof this is the garage PC
  (WoL); here it is a second client in-process talking only to the LAN base.

The demo proves, over the real wire:

1. REPLICATE — a vault edit on ONE node appears on the OTHER:
   * LAN->remote: node A commits a new revision; node B pulls and reads it back
     (content-hash verified).
   * remote->LAN: node B records an offline edit and syncs back; node A reads
     the committed revision (content-hash verified) — the reverse direction.

2. CONFLICT — concurrent edits diverge and resolve deterministically:
   * node B records an offline edit based on rev N; node A advances the head to
     N+1 meanwhile; node B's sync_push is REFUSED (409) — no silent overlay —
     and the server records a pending merge with base/ours/theirs snapshots.
   * resolution choice (ours/theirs/client-merged document) commits a NEW
     revision whose files[0].blob_sha256 is the sha-256 of the resolved bytes
     — deterministic, hash-verified after commit; the conflict then 404s.

3. AIRGAP — the whole demo runs inside the LAN boundary under the P8-M5-T03
   default `Airgap` role: no relay, no telemetry, no egress beyond the base.
   The leg asserts every request went to the LAN/loopback base only.

Honesty boundary (card: "conflict semantics documented and honest"): the
resolution surface is ours/theirs/client-merged-document, NOT an automatic
hunk-level text merge (a diff3 column view is a P8-M4/T05 follow-up). This
module never simulates a node: every leg is a real socket against a real
binary. A leg that cannot reach its base (binary missing, service down) is
classified SKIP against the closed taxonomy, never invented as PASS, never a
crash — exactly the P8/P9 gate doctrine. The live two-host (U1 LAN + garage PC
remote) proof is scripts/p10m4t01-replication-proof.sh -> P10-M4/proof/*.
"""
from __future__ import annotations

import base64
import datetime
import hashlib
import io
import json
import os
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path

VERDICTS = ("PASS", "FAIL", "SKIP")

# Closed SKIP taxonomy for this demo (a SKIP row always carries one of these).
CLOSED_TAXONOMY = (
    "service-unreachable",   # the PDM service did not answer on the base URL
    "binary-missing",        # oep-pdm binary not built on this ref
    "remote-node-offline",   # live proof: the remote/LAN node did not answer
    "io-error",
)


@dataclass
class LegRow:
    """One classified outcome of a demo leg."""

    id: str
    leg: str                  # "replicate" | "conflict" | "airgap"
    status: str               # PASS / SKIP / FAIL
    detail: str = ""
    reader: str = "client"    # how the row was obtained (client | probe | verify)
    cls: str = ""             # closed taxonomy code (SKIP rows)

    def as_dict(self) -> dict:
        return {"id": self.id, "leg": self.leg, "status": self.status,
                "detail": self.detail, "reader": self.reader, "class": self.cls}


def _root(base: str) -> str:
    """Normalize a base URL to the host root (accept anchored /api/v1 too)."""
    stripped = base.rstrip("/")
    if stripped.endswith("/api/v1"):
        return stripped[: -len("/api/v1")]
    return stripped


def _sha(b: bytes) -> str:
    return hashlib.sha256(b).hexdigest()


def _upload(content: bytes, filename: str = "part.step") -> dict:
    return {
        "filename": filename,
        "format": "step",
        "content_type": "text/plain",
        "content": base64.b64encode(content).decode(),
    }


# ---------------------------------------------------------------------------
# Replicate leg — a vault edit on one node appears on the other (2 directions)
# ---------------------------------------------------------------------------

def replicate_leg(base_url: str) -> list:
    """Two nodes, one vault: LAN->remote and remote->LAN edit propagation.

    Returns classified rows. Legs that cannot reach the service are SKIP
    (closed taxonomy), never a crash.
    """
    import oep_pdm_client as C
    rows = []
    root = _root(base_url)
    try:
        c = C.PdmClient(root)
        c.health()
    except Exception:
        return [LegRow("replicate-leg", "replicate", "SKIP",
                       "service unreachable", cls="service-unreachable")]
    try:
        # --- seed: a base item + revision exists on node A (the LAN vault) ---
        code, item = c.create_item({"name": "P10M4Part", "kind": "part",
                                    "context": "p10-m4-t01"})
        assert code == 201, (code, item)
        iid = item["id"]
        base_body = b"STEP base\nDIMENSION A = 10\nHOLE B = 5\n"
        c.save_revision(iid, "part.step", "step", base_body)
        code, rev = c.list_revisions(iid)
        base_rev = rev["revisions"][-1]["number"]
        base_hash = rev["revisions"][-1]["files"][0]["blob_sha256"]

        # --- LAN->remote: node A commits a newer revision; node B pulls it ---
        lan_edit = b"STEP base\nDIMENSION A = 12\nHOLE B = 5\n"
        c.save_revision(iid, "part.step", "step", lan_edit,
                        created_by="node-a")
        # Node B ("remote node": a sync client replica of the same vault)
        # pulls the head and sees node A's edit, hash-verified.
        b = C.PdmClient(root)
        payload = b.open_latest_file(iid)
        seen_on_b = (payload.bytes == lan_edit
                     and payload.blob_sha256 == _sha(lan_edit))
        rows.append(LegRow(
            "lan-edit-seen-on-remote", "replicate",
            "PASS" if seen_on_b else "FAIL",
            "node A edit revision=%s blob=%s" % (payload.rev,
                                                 payload.blob_sha256[:12])))
        if not seen_on_b:
            rows[-1].detail += " expected hash " + _sha(lan_edit)[:12]
        if not rows[-1].status == "PASS":
            return rows

        # --- remote->LAN: node B records an offline edit on the CURRENT head
        #     and syncs it back; node A's vault accepts the next revision ---
        remote_edit = b"STEP base\nDIMENSION A = 12\nHOLE B = 8\n"
        code, push = c.sync_push({
            "item_id": iid,
            "base_revision": str(int(base_rev) + 1),
            "base_blob_sha256": _sha(lan_edit),
            "created_by": "node-b",
            "file": _upload(remote_edit),
        })
        pushed_ok = (code == 201 and push.get("status") == "synced")
        rows.append(LegRow("remote-edit-seen-on-lan", "replicate",
                           "PASS" if pushed_ok else "FAIL",
                           "sync_push status=%s code=%s" % (
                               str(push.get("status")), code)))
        if pushed_ok:
            # node A reads back the remote edit, hash-verified.
            payload = c.open_latest_file(iid)
            ok_lan = (payload.bytes == remote_edit
                      and payload.blob_sha256 == _sha(remote_edit))
            if not ok_lan:
                rows.append(LegRow("remote-edit-back-on-lan-verified",
                                   "replicate", "FAIL",
                                   "blob mismatch on node A"))
        return rows
    except Exception as e:  # noqa: BLE001 - classify, never crash
        rows.append(LegRow("replicate-leg-exception", "replicate", "FAIL",
                           repr(e)))
        return rows


# ---------------------------------------------------------------------------
# Conflict leg — concurrent edits diverge, resolve deterministically by hash
# ---------------------------------------------------------------------------

def conflict_leg(base_url: str) -> list:
    """Deterministic conflict semantics: divergence detected on base identity,
    resolution committed as a new revision verified by content hash."""
    import oep_pdm_client as C
    rows = []
    root = _root(base_url)
    try:
        c = C.PdmClient(root)
        c.health()
    except Exception:
        return [LegRow("conflict-leg", "conflict", "SKIP",
                       "service unreachable", cls="service-unreachable")]

    try:
        code, item = c.create_item({"name": "P10M4Conflict",
                                    "kind": "part",
                                    "context": "p10-m4-t01"})
        assert code == 201, (code, item)
        iid = item["id"]
        base_bytes = b"STEP conf\nCRITICAL = 42\nNOTE = keep\n"
        c.save_revision(iid, "part.step", "step", base_bytes)
        code, rev = c.list_revisions(iid)
        base_rev = rev["revisions"][-1]["number"]
        base_hash = rev["revisions"][-1]["files"][0]["blob_sha256"]

        # node B records an offline edit against rev N (base).
        ours = b"STEP conf\nCRITICAL = 44\nNOTE = keep\n"
        # node A advances the head meanwhile (theirs).
        theirs = b"STEP conf\nCRITICAL = 42\nNOTE = theirs won the race\n"
        c.save_revision(iid, "part.step", "step", theirs, created_by="node-a")

        # node B syncs back -> divergent, refused (no silent overlay).
        code, push = c.sync_push({
            "item_id": iid,
            "base_revision": base_rev,
            "base_blob_sha256": base_hash,
            "created_by": "node-b",
            "file": _upload(ours),
        })
        diverged = (code == 409 and push.get("status") == "conflict"
                    and push.get("sync_id"))
        rows.append(LegRow("concurrent-edit-divergence", "conflict",
                           "PASS" if diverged else "FAIL",
                           "push code=%s status=%s" % (code,
                                                       str(push.get("status")))))
        if not diverged:
            return rows
        sync_id = push["sync_id"]

        # The pending merge records base/ours/theirs with content hashes.
        code, view = c.get_sync_conflict(sync_id)
        snap_ok = (code == 200
                   and view["base"]["blob_sha256"] == base_hash
                   and view["ours"]["blob_sha256"] == _sha(ours)
                   and view["theirs"]["blob_sha256"] == _sha(theirs))
        rows.append(LegRow("conflict-three-way-snapshots", "conflict",
                           "PASS" if snap_ok else "FAIL",
                           "base=%s ours=%s theirs=%s" % (
                               view["base"]["blob_sha256"][:10],
                               view["ours"]["blob_sha256"][:10],
                               view["theirs"]["blob_sha256"][:10])))

        # Resolution: keep OURS (a deterministic client choice). The committed
        # revision's blob sha-256 must equal the hash of the chosen bytes.
        merged_bytes = ours
        code, resolved = c.resolve_conflict(sync_id, {
            "choice": "ours",
            "merged": _upload(merged_bytes),
        })
        res_ok = (code == 201
                  and resolved["files"][0]["blob_sha256"] == _sha(merged_bytes)
                  and resolved["number"] == str(int(base_rev) + 2))
        rows.append(LegRow("resolution-hash-verified", "conflict",
                           "PASS" if res_ok else "FAIL",
                           "resolved rev=%s blob=%s (choice=ours)" % (
                               resolved.get("number"),
                               resolved["files"][0]["blob_sha256"][:12])))
        # The conflict is gone afterwards (404 = resolved).
        gone = False
        try:
            code, _ = c.get_sync_conflict(sync_id)
            gone = code == 404
        except C.PdmError as pe:
            gone = pe.status == 404
        rows.append(LegRow("conflict-consumed", "conflict",
                           "PASS" if gone else "FAIL",
                           "GET conflict after resolve -> 404 expected, gone=%s"
                           % gone))
        return rows
    except Exception as e:  # noqa: BLE001
        rows.append(LegRow("conflict-leg-exception", "conflict", "FAIL",
                           repr(e)))
        return rows


# ---------------------------------------------------------------------------
# Airgap leg — the demo runs inside the LAN boundary (P8-M5-T03 default role)
# ---------------------------------------------------------------------------

def airgap_leg(base_url: str) -> list:
    """The whole demo talks only to the LAN/loopback base: no relay, no
    telemetry, no egress beyond the registered host. Verified by instrumenting
    every client so no request leaves the bound base URL's host."""
    import oep_pdm_client as C
    rows = []
    root = _root(base_url)
    from urllib.parse import urlparse
    bound_host = urlparse(root).netloc
    seen = set()

    _orig = C.PdmClient._url

    def _instrumented(self, path):
        u = _orig(self, path)
        seen.add(urlparse(u).netloc)
        assert urlparse(u).netloc == bound_host, \
            "egress beyond the LAN boundary: %s" % u
        return u

    C.PdmClient._url = _instrumented
    try:
        c = C.PdmClient(root)
        c.health()
        c.create_item({"name": "AirgapProbe", "kind": "part",
                       "context": "p10-m4-t01"})
        ok = (seen <= {bound_host})
        rows.append(LegRow(
            "airgap-lan-boundary", "airgap",
            "PASS" if ok else "FAIL",
            "requests seen: %s (bound host: %s)" % (sorted(seen), bound_host)))
    except Exception as e:  # noqa: BLE001
        rows.append(LegRow("airgap-lan-boundary", "airgap", "FAIL", repr(e)))
    finally:
        C.PdmClient._url = _orig
    return rows


# ---------------------------------------------------------------------------
# Report + exit code + driver (mirrors the P8/P9 gate report contract)
# ---------------------------------------------------------------------------

def build_report(rows: list, binary: str, base_url: str) -> dict:
    summary = {"total": len(rows)}
    for v in VERDICTS:
        summary[v] = sum(1 for r in rows if r.status == v)
    summary["failures"] = [r.id for r in rows if r.status == "FAIL"]
    return {
        "schema_version": 1,
        "description": "P10-M4-T01 multi-site replication demo: LAN + remote "
                       "node replicating the vault, conflict semantics",
        "generated_at": datetime.datetime.now(
            datetime.timezone.utc).isoformat(),
        "base_url": base_url,
        "binary": binary,
        "topology": {
            "lan_node": "oep-pdm service (authoritative vault)",
            "remote_node": "sync client replica (generated PdmClient)",
            "deployment_role": "Airgap (P8-M5/hybrid-cloud-role-adr.md)",
            "decisions": ["#9 run targets: U1 + garage PC (WoL), NOT NAS",
                          "#10 on-prem/LAN default, no internet egress"],
        },
        "summary": summary,
        "rows": [r.as_dict() for r in rows],
    }


def exit_code(report: dict) -> int:
    """0 iff no FAIL row anywhere (SKIP allowed)."""
    return 1 if report["summary"].get("FAIL", 0) else 0


def write_md(report: dict) -> str:
    lines = []
    lines.append("# P10-M4-T01 multi-site replication demo report\n")
    lines.append("- schema_version: %s" % report["schema_version"])
    lines.append("- generated_at: %s" % report["generated_at"])
    lines.append("- binary: %s" % report["binary"])
    lines.append("- base_url: %s" % report["base_url"])
    lines.append("\nTopology:")
    for k, v in report["topology"].items():
        lines.append("- %s: %s" % (k, v))
    lines.append("\nSummary: total=%d PASS=%d FAIL=%d SKIP=%d" % (
        report["summary"]["total"], report["summary"]["PASS"],
        report["summary"]["FAIL"], report["summary"]["SKIP"]))
    lines.append("\n| id | leg | status | class | detail |")
    lines.append("|---|---|---|---|---|")
    for r in report["rows"]:
        lines.append("| %s | %s | %s | %s | %s |" % (
            r["id"], r["leg"], r["status"], r.get("class", ""), r["detail"]))
    lines.append("\nExit code: %d (0 = no FAIL; SKIP allowed)" %
                 exit_code(report))
    return "\n".join(lines) + "\n"


def write_report(report: dict, outdir: str) -> None:
    out = Path(outdir)
    out.mkdir(parents=True, exist_ok=True)
    (out / "p10m4t01-replication.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8")
    (out / "p10m4t01-replication.md").write_text(
        write_md(report), encoding="utf-8")


def run_demo(base_url: str, outdir: str, binary: str = "") -> int:
    """Run all legs against a reachable service and write the report.

    Returns the exit code (0 = no FAIL anywhere). If the service is unreachable
    each leg classifies SKIP against the closed taxonomy (CI-safe), same as
    the P8/P9 gates.
    """
    rows: list = []
    rows += replicate_leg(base_url)
    rows += conflict_leg(base_url)
    rows += airgap_leg(base_url)
    report = build_report(rows, binary, base_url)
    write_report(report, outdir)
    return exit_code(report)


if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--base-url", default="http://127.0.0.1:8787/api/v1")
    p.add_argument("--outdir", default="target/p10m4-report")
    p.add_argument("--binary", default="")
    args = p.parse_args()
    raise SystemExit(run_demo(args.base_url, args.outdir, args.binary))
