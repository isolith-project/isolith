# P0-M2-T04 Export Control Memo: ITAR / ECCN Classification of Engineering Data Flows; Air-Gap Posture

Version: 1.0 (2026-10-15)
Owner: Keelworks program governance, P0-M2 (legal posture)
Status: internal program document. This memo is an internal export-control posture reflection, not legal advice. It maps the US regulatory frameworks so engineering can build inside them; it does not replace qualified export-control counsel or a formal BIS CCATS / DDTC commodity-jurisdiction determination, which is scheduled for specific items at the appropriate gate (P11-T05, public-release gate).

## 1. Scope and purpose

Keelworks (OEP) is an open engineering platform: readers that open third-party CAD artifacts (CGR, 3DXML, CATPart), a geometry core, a self-hosted PDM server, and P2P compute, all from lawful observation and public specifications (clean-room invariant), privately developed on NAS Gitea, with a decided hybrid license (Apache-2.0 applications/libraries, AGPL-3.0 server/P2P). This memo answers two questions from the card:

1. How are Keelworks' engineering data flows classified under the US International Traffic in Arms Regulations (ITAR, 22 CFR 120-130) and the Export Administration Regulations (EAR, 15 CFR 730-774), expressed as ECCNs on the Commerce Control List?
2. What is the air-gap posture that makes a Keelworks deployment fit for an ITAR/EAR-regulated shop, and what can we honestly claim about it?

The memo feeds P8 (PDM RBAC/audit), P9 (worker sandbox), P10 (offline install), P11 (release export-control checkbox), P13 (sovereignty narrative), and the P1 corpus sweep (P1-M1-T06). It cross-references the sibling postures: P0-M2-T01 (RE doctrine and contract exposure) and P0-M2-T02 (patent posture).

## 2. The governing framework: two regimes, one order of review

US export control splits jurisdiction by what the item is, not by what tool produced it. A file full of geometry is classified by the same rules as the physical article it describes.

- ITAR, administered by the State Department's Directorate of Defense Trade Controls (DDTC), controls defense articles, defense services, and related technical data on the United States Munitions List (USML, 22 CFR 121.1), under the Arms Export Control Act (22 U.S.C. 2778) and E.O. 13637.
- EAR, administered by Commerce's Bureau of Industry and Security (BIS), controls dual-use and most other items under 15 CFR 730-774. Items on the Commerce Control List (CCL, 15 CFR Part 774) carry an Export Control Classification Number (ECCN); items subject to the EAR but not on the CCL are designated EAR99 (15 CFR 734.3(c)).
- Order of review (22 CFR 120.11; USML "specially designed" logic at 120.41-120.42): decide USML first, then 600-series / other CCL ECCNs, then EAR99. Where doubt persists, DDTC commodity jurisdiction determination (22 CFR 120.12) or BIS CCATS (15 CFR 748.3, see 750.2) resolves jurisdiction; this memo flags the specific flows that could need one.

### 2.1 What "technical data" means under ITAR

22 CFR 120.33(a)(1) defines technical data as information, other than software as defined in 22 CFR 120.40(g), which is required for the design, development, production, manufacture, assembly, operation, repair, testing, maintenance, or modification of defense articles, including blueprints, drawings, photographs, plans, instructions, and documentation. Three consequences that structure this whole memo:

- Software is NOT technical data by default. Under 120.40(g), software includes system functional design, logic flow, algorithms, application programs, operating systems, and support software. It becomes ITAR technical data only under 120.33(a)(4), when it is "directly related" to defense articles. General-purpose CAD/PDM software is not directly related to defense articles as such.
- Data that is "required for" turning a file into a physical defense article is the heart of the definition. A CAD model of a USML item (geometry, PMI, GD&T, drawings, tolerances) is typically technical data under 120.33(a)(1); the same model of an ordinary industrial bracket is not.
- The definition excludes (120.33(b)) general scientific, mathematical, or engineering principles commonly taught, information in the public domain as defined in 22 CFR 120.34, and telemetry data as defined in note 3 to USML Category XV(f).

Public domain (120.34(a)) means information that is published and generally accessible to the public, including through public release in any form and through fundamental research at accredited US institutions where the results are ordinarily published and shared broadly. That is the statutory slot that open-source code and public specs occupy: information in the public domain is not ITAR technical data.

### 2.2 What is NOT subject to the EAR at all

15 CFR 734.3(b)(3) removes several classes from EAR scope entirely, including information and software that are published as described in 15 CFR 734.7, that arise from fundamental research (734.8), that appear in patents, or that are non-proprietary system descriptions. 15 CFR 734.7(a) treats software as published when it is made available to the public without restrictions on further dissemination, expressly including posting on the internet. Two carve-outs matter for us later:

- 15 CFR 734.7(b): published encryption software classified under ECCN 5D002 remains subject to the EAR unless it is publicly available encryption object code whose source meets 15 CFR 742.15(b). This touches the PDM server's TLS/encryption surface (see section 5.2).
- 15 CFR 734.7(c): software or technology for producing certain firearm items (ECCNs 0A501, 0A506, 0A507, 0A509) that is posted online and ready for insertion into a CNC/AM machine (AMF or G-code) REMAINS subject to the EAR even when published. That is a genuine backstop for any digital-manufacturing output our exporters later feed (P2-M5, CAM-style passes): "published" does not rescue firearm-production G-code/CAM files.

## 3. Classification of the Keelworks engineering data flows

The table below classifies each named flow. The spine rule is stated first so the table stays honest: the classification of a data flow rides on the article the data describes, not on the OEP software that carries it.

[A] Source code of OEP (readers, geometry core, PDM, workers) once publicly released:
- Not subject to the EAR (published, 15 CFR 734.3(b)(3) + 734.7(a)); not ITAR technical data (public domain, 22 CFR 120.34). ECCN: none; not on the CCL. [uncertain] While Keelworks is still private pre-publication on NAS Gitea, the code is not yet "published" under 734.7; the program must confirm the private tree contains no ITAR technical data before any foreign-person access (that is the P1-M1-T06 sweep discipline extended to source), and treat the pending-encryption surface per 5.2.
- This is the export-clean core of the sovereignty narrative (section 6): open public release of the tool is what makes the tool itself export-clean.

[B] CAD model / B-Rep / tessellation geometry (customer content: CGR, CATPart, 3DXML, STEP):
- If it describes an item on the USML: ITAR technical data (22 CFR 120.33(a)(1)); export/release to a foreign person requires a license or exemption under Parts 123-125. If it describes a "600-series" item transferred to the EAR (Export Control Reform; e.g., military electronics under ECCNs 3A611/3D611 with technology under 3E611, military aircraft under 9A610/9E610 and 9A619/9E619): EAR-controlled technology; BIS license/exception governs, including the 600-series end-user/end-use posture.
- If it describes ordinary commercial/industrial articles: EAR99 (not on the CCL). No license to most destinations, but general prohibitions still apply (restricted parties, embargoed destinations under 15 CFR 742/746, and prohibited end uses), so EAR99 is not "no controls."
- OEP's own interchange formats (STEP AP242, QIF, JT, 3DXML ASCII) are containers, not classifications; the classification is the payload's (see section 4 note on the cargo rule).

[C] PMI / GD&T / drawings / tolerancing:
- Same analysis as [B]. PMI attached to a USML article is technical data; PMI on a commercial part is EAR99. Because PMI is precisely what makes a file "required for production/manufacture," a drawing set of a defense article is the clearest ITAR technical-data case in the platform.

[D] PDM vault blobs and relationship graph (P8):
- The vault is an aggregation of flows [B]/[C]. Its classification is the max classification of any blob or derived object it stores, and the relationship graph adds context that can itself be "information required for" assembly/modification. Practical rule for the platform: enforce per-object attribute-based controls (P8-M3), audit reads and releases (P8-M3-T03), and treat the vault as needing an air-gap or an E2E-encryption boundary whenever it holds any [B]/[C] data that is ITAR- or 600-series-controlled.

[E] Render/export worker outputs (P9 compute): STL, glTF, OBJ, PDF drawings, meshes:
- Derivative of [B]/[C], so the same classification as the source data. Two aggravating facts: (1) a rendered image or drawing can reveal technical data through visual inspection, which is itself a form of release under 22 CFR 120.56(a)(1) when a foreign person inspects a defense article; (2) the P9 worker sandbox must have no network egress by default (P9-M3-T03), because a render job crossing a boundary could be a technical-data export wearing a JPEG costume. Air-gap enforcement starts at the worker.

[F] Telemetry / crash reports:
- Decided P0-M5-T02: none by default (sovereignty principle), single opt-in exception, crash reports only, bounded to diagnostics, sent to a user/self-host-controlled endpoint. Export relevance: crash telemetry must never carry model geometry, PMI, file paths revealing defense-article identity, or controlled data in any form. For an ITAR/EAR deployment the opt-in is HARD-DISABLED (see section 6): metadata about a defense article can still be "information related to" it, and the cleanest posture is no egress at all.

[G] Build artifacts / binaries / releases:
- Once built from publicly released source, the same published analysis applies (734.7), subject to 5.2 (encryption) and 734.7(c) (firearm CAM). Each release should ship the P11-T05 export-control classification file (an "export control classification number / no-license-required" notice) with the artifact, so downstream users and CDNs have a durable classification statement instead of guessing.

Flow table summary (what leaves the platform boundary, and under which regime):

- Source release (public OSS): not subject to EAR / not ITAR (published + public domain).
- Customer geometry of USML article: ITAR technical data (120.33).
- Customer geometry of 600-series item: EAR-controlled 600-series technology (3E611/9E610/9E619 line).
- Customer geometry of commercial item: EAR99.
- PMI/drawings of defense article: ITAR technical data (clearest case).
- Worker/rendered outputs: derivative; same class as source; worker has no egress.
- Telemetry: none by default; hard-disabled for controlled deployments; diagnostics only.
- BIM/CAM/G-code for firearm items: stays EAR-controlled even if published (734.7(c)).

## 4. The three rules that keep this memo honest

1. The cargo rule. Keelworks does not change the classification of data; it transports it. A STEP file of a USML component is ITAR technical data whether it was made in CATIA, FreeCAD, or OEP. The platform's obligation is to carry classification metadata faithfully (source article, controlled/not, country of destination) and to enforce controls, not to guess its customers' classifications. The P4/P6 geometry model and P8 vault schema should reserve an optional classification attribute on items so the flag survives the pipeline.
2. The publication rule. The single most powerful thing the program does for export posture is public release under open licenses: it moves the tool itself, and every flow derived purely from the tool's own code/interfaces, outside EAR scope (734.3(b)(3), 734.7) and outside ITAR technical data (120.34). Clean-room means we never embed vendor binary/source in the tool, so nothing of a protected vendor's controlled data rides in our readers by construction (pairs with P0-M2-T02 patent posture).
3. The custody rule. Under 22 CFR 120.56, technical data is "released" (an export) to a foreign person through use of access information to enable access to UNENCRYPTED technical data (120.56(a)(3)-(4), with access information defined in 120.55 as decryption keys, network codes, passwords). Authorization is required to provide that access information if it can enable access to unencrypted controlled data (120.56(b)). Consequence for P8: who holds the decryption keys is a defense-trade-controls decision. Keys must stay with US persons / the data owner, never with the service, for any controlled vault.

## 5. Air-gap posture and the encryption carve-out

### 5.1 The ITAR encryption carve-out (22 CFR 120.54(a)(5))

Sending, taking, or storing unclassified technical data is NOT an export, reexport, retransfer, or temporary import when it is: (i) unclassified, (ii) secured with end-to-end encryption, and (iii) secured with cryptographic modules compliant with FIPS 140-2 or its successors (i.e., FIPS 140-3) or other crypto providing security strength at least comparable to AES-128, and the data is not sent to or stored in a proscribed country under 22 CFR 126.1. End-to-end encryption is defined in 120.54(b): data is never in unencrypted form between the originator's security boundary and the recipient's boundary, and the means of decryption are not provided to any third party. And 120.54(c): the ability merely to access data in encrypted form satisfying (a)(5) is not itself a release or export.

This is the legal foundation for an OEP hybrid-cloud / remote-access deployment that still handles controlled data: E2E encryption with FIPS-validated modules, decryption only inside US-person-controlled boundaries, no key custody by the service, no 126.1-country touch. It is also the honest boundary of that carve-out:

- It does not lift release/access restrictions. The moment a foreign-person-boundary receives the decryption key, or any access information enabling unencrypted access, the 120.56 release analysis re-engages (120.56(b)). E2E means keys never reach any third party, including our own cloud/P2P relay.
- It applies to data that is UNSECLASSIFIED technical data. Classified data is a different regime entirely (22 CFR 120.33(a)(2)).
- Exports to 22 CFR 126.1 proscribed destinations are excluded regardless of encryption.

### 5.2 The PDM server encryption surface under the EAR

Because OEP ships open-source software that includes a PDM server with TLS/encryption features, the encryption items rules matter at classification time:

- If released publicly as source under an open license, the tool and its TLS use fall out of scope under 734.3(b)(3)/734.7(a), with the single encryption caveat of published-software 5D002 in 734.7(b).
- If a proprietary variant of the server were ever distributed in binary-only form (e.g., a commercial "managed PDM" edition), the mass-market / ENC analysis under 15 CFR 740.17 applies; mass-market encryption software meeting eligibility is released from EI and NS controls and designated 5D992.c (see related controls in ECCN 5D002 / 5D992 under Part 774). The P11-T05 classification file should state this per release so nobody has to reverse the branch. [uncertain] whether we will ever ship a closed binary; if we do, a CCATS for the specific build is the safe route.

### 5.3 Deployment postures

- Pure air gap (ITAR shop on NAS/U1): no internet, no telemetry (opto-in disabled), no update callbacks, no cloud round-trips. The offline install path (P10-M2-T02, USB/ISO) is this posture's deliverable. Everything stays inside the shop boundary; nothing is exported because nothing leaves. This is the posture the reference vertical (P14, "small defense subcontractor shop") starts from.
- Hybrid / remote (LAN + Bastion-shaped relay, mobile view): only when the 120.54(a)(5) conditions are met plus the custody rule (section 4 rule 3) plus a policy that controlled vaults are not accessible to foreign persons via access information. The P2P relay must be relay-only, never key-holding, and never in a 126.1 country.
- Worker/compute sandbox (P9-M3-T03): no network egress by default in every posture. Remote/external execution of a render or export job on controlled data is itself the transmission that can become an export; the default-no-egress rule and a hard geographic boundary on worker nodes are required for a controlled-data shop.

## 6. The sovereignty narrative, stated precisely

The P13-T04 narrative should claim exactly what is true:

- The tool is export-clean as published open source: not subject to the EAR (published) and not ITAR technical data (public domain), with a per-release classification file (P11-T05).
- ITAR-cleanliness of a DEPLOYMENT is a customer configuration/policy obligation, which Keelworks genuinely supports: air-gap install (P10-M2-T02), no telemetry by default with hard-disable for controlled environments (P0-M5-T02), E2E/FIPS boundary and key custody (120.54/120.56), worker no-egress (P9-M3-T03), RBAC + audit (P8-M3).
- What we must NOT claim: that storing or transmitting a customer's defense-article technical data in OEP makes it non-ITAR, or that any deployment is automatically compliant. The cargo rule (section 4) makes that the customer's exporters' obligation; our product promise is capability and defaults, not their compliance.

## 7. Workstream mapping and required engineering hooks

- P2/P3/P4 readers: never bundle vendor binaries/source (clean-room); carry an optional controlled-classification attribute so downstream does not lose the flag; decline access-protected files per P0-M2-T01 (no circumvention).
- P6 geometry core / P4 interchange: reservation for classification metadata on the internal model; STEP/QIF/JT/3DXML are cargo containers.
- P8 PDM: attribute-based RBAC per object (P8-M3), tamper-evident audit of reads/releases (P8-M3-T03), key-custody boundary separating the service from decryption keys (120.56), optional controlled flag on items.
- P9 compute: worker sandbox with no network egress by default (P9-M3-T03), geographic/topology boundary on worker nodes, no telemetry from workers.
- P10 delivery: USB/ISO offline install (P10-M2-T02) as a first-class path; deployment docs covering the 120.54(a)(5) conditions and proscribed-destination checks (126.1).
- P11 release: per-release export-control classification file (P11-T05) checked into each release artifact; encryption/5D002/5D992 note; firearm CAM (734.7(c)) backstop note where relevant.
- P1 corpus: the P1-M1-T06 ITAR/privacy sweep extends to the private source tree pre-publication (section 3[A]).
- P13 narrative: only the precise claims in section 6.

## 8. Recommendations and next actions

1. Adopt the classification frame (tool published/non-subject; data rides on article; 600-series vs EAR99) as the program's authoritative export-position statement. Version 1.0 dated 2026-10-15.
2. Add the optional controlled-classification attribute to P6 model / P8 vault schema (Section 4 rule 1) in the relevant cards (P6-M1, P8-M1).
3. Encode the hard-disable: controlled-data deployments get telemetry OPT-OUT FORCED and worker no-egress default in the P8/P9 acceptance criteria and CI (pair with P0-M5-T02).
4. Write the P11-T05 release classification file template now (it is a form, not a mystery) with slots for ECCN/EAR99, ENC note, 734.7(c) backstop, and ITAR-none statement; ship it with the first public release.
5. Keep the private tree ITAR-clean pre-publication: extend the P1-M1-T06 sweep to source, and gate any foreign-person contributor access on the sweep before first public release (plan doctrine 5, P0-M2 gate).
6. Route the genuinely uncertain items to counsel/agency: (a) the 5D002/5D992 ENC note if we ever ship a closed binary (5.2), (b) any specific defense-article geometry class a customer insists on storing (their exporter's CCATS/CJ, not ours, but confirm contractual flow-down), (c) the classified-data carve-out (never handled). This memo is posture, not a determination.
7. Re-run the classification snapshot before first public release and at each major USML/ECCN reform (2026-10-01 proposed rule 91 FR 62361 proposes definitional and structural USML changes; 91 FR 55460, 2026-08-28 IFR and correction 91 FR 58610 amend Part 120; these were current observations, see verification log).

## 9. Open questions

- [uncertain] Exact ECCN of the AGPL PDM server build once shipped (5D002/5D992 ENC note) — resolved at release time, see recommendation 4.
- [uncertain] Whether any Keelworks customer classifies their own payloads consistently; P8 classification attribute + contract flow-down mitigate, but we should define an "unclassified by default, flag to elevate" convention.
- [uncertain] CCATS/CJ need for specific future exports (commercial managed-PDM binary, if any) — trigger at first such release.

## 10. Scope and non-goals (what this memo is not)

This memo does not advise on: classified-defense information (different regime), OFAC sanctions beyond the 126.1 reference, nuclear/NRC or DOE 810 regimes, or any foreign (EU/UK) dual-use regimes. It is not legal advice; the positions here are program posture to be confirmed by qualified US export counsel before first public release (public-release gate, per P0-M2 series and plan doctrine 5).
