# P0-M2-T04 Verification Log: Export Control Memo

Real-material verification of every regulatory anchor cited in the memo,
performed live on 2026-10-15 during this task run. This log is the proof that
the memo is grounded in observed primary sources, not memory. Each line names
what was checked, where it was observed, and the result.

## Primary CFR definitions verified verbatim on eCFR (2026-10-15)

1. 22 CFR 120.33 (technical data) - observed full text of (a)(1)-(4) and (b) on
   eCFR. Confirmed: software as defined in 120.40(g) excluded from (a)(1) but
   captured in (a)(4) when directly related to defense articles; blueprints /
   drawings / photographs / plans / instructions / documentation named;
   public-domain, general engineering principles, and telemetry-data exclusions
   in (b). PASS.
2. 22 CFR 120.34 (public domain) - observed full text of (a)(1)-(8) incl. the
   fundamental-research criterion (a)(8) and the government-access-control
   limitation (b). eCFR "up to date as of 10/08/2026, last amended 10/01/2026".
   PASS.
3. 22 CFR 120.40 (compositional terms) - observed (g) software definition text
   verbatim (system functional design, logic flow, algorithms, application
   programs, operating systems, support software) plus the note that software
   not specifically enumerated on USML is licensed under Part 125. PASS.
4. 22 CFR 120.54 (activities that are not exports...) - observed full text of
   (a)(1)-(7) and (b), (c). Specifically confirmed (a)(5) unclassified +
   end-to-end encryption + FIPS 140-2 (or successors) / AES-128-equivalent,
   not sent to/stored in a 126.1 proscribed country; (b) E2E definition
   (unencrypted only between originator/recipient security boundaries, means of
   decryption not provided to any third party); (c) encrypted access not a
   release. Cross-reference amendment blocks observed: 91 FR 55460 (Aug 28 2026)
   and correction 91 FR 58610 (Sep 16 2026). PASS.
5. 22 CFR 120.55 / 120.56 (access information / release) - observed 120.56 full
   text: release by visual inspection (a)(1), oral/written exchange (a)(2),
   access-information enabling access/view/possession of unencrypted technical
   data (a)(3), causing technical data outside the US to be unencrypted (a)(4);
   provision-of-access-information authorization requirement (b). PASS.
6. 22 CFR Part 121 (USML) - observed structure incl. USML Category XI(d)
   (technical data / defense services directly related to XI(a)-(c) articles
   and classified 3A611/3B611/3C611/3D611 tech data) and Category XI(x)
   (EAR-controlled commodities/software/technical data used in or with defense
   articles, license-app only). PASS.
7. 22 CFR Part 120 order of review - observed 120.11 (order of review) and
   120.12 (commodity jurisdiction determination requests) in the part-level
   table of contents; 120.41/120.42 (specially designed; form/fit/function
   etc.) also observed in the same TOC context. PASS.

## EAR provisions verified on eCFR / BIS (2026-10-15)

8. 15 CFR 734.3 (items subject to the EAR) - observed (b)(3) published /
   fundamental research / patents / non-proprietary system descriptions
   exclusion list and (c) EAR99 designation text (items subject to EAR not
   listed on CCL designated EAR99), plus (e) DDTC-approval interplay. PASS.
9. 15 CFR 734.7 (published) - observed (a) published definition (made available
   without restriction, incl. internet posting), (b) published encryption
   software under 5D002 remains EAR-subject unless publicly available and
   meeting 742.15(b), (c) firearm CAM carve-out (0A501, 0A506, 0A507, 0A509
   AMF/G-code remains EAR-subject). PASS.
10. 15 CFR 734.8 (fundamental research) - observed via BIS Part 734 guidance and
    TAMU EAR Part 734 PDF (research.tamu.edu), matching 734.3(b)(3)(ii)
    reference. PASS.
11. 15 CFR Part 774 (CCL) - observed ECCN related-controls references in part
    774 text, incl. mass-market encryption software that meets eligibility
    released from EI/NS and designated 5D992.c; 0E604/9E619 references showing
    the 600-series technology-control pattern. PASS.
12. 15 CFR 740.17 / 742.15 / 748.3 / 750.2 - cited from eCFR part structure and
    the gingercontrol classification guide (750.2(a): BIS answer commitment
    within 14 calendar days); 740.17 ENC and 742.15(b) criteria cross-referenced
    in 734.7(b). PASS (procedural, industry guide).

## Rulemaking / current-state observations (2026)

13. DDTC Public Portal news feed - observed (a) 2026-09-18 final rule amending
    126.1 (adds Saudi Arabia and Peru to major non-NATO ally list, other 126.1
    clarifications; FR Doc. 2026-19161); (b) 2026-09-30/10-01 final rule
    removing Syria from 126.1 (effective Oct 1, 2026); (c) 2026-08-27 IFR
    removing certain ASE-modified civil aircraft from USML; (d) Data Collection
    Modernization (DCM) deployment Sep 14 2026; (e) 2026-10-01 proposed rule
    (91 FR 62361) further reviewing USML and definitions with new exemption,
    comments due Nov 30 2026. PASS - memo's "current, moving picture" claims
    are grounded, and the memo flags these as requiring re-run before release.
14. Export Control Reform 600-series - observed the 2014 Federal Register rule
    adding ECCNs 3A611/3B611/3D611/3E611 (military electronics transferred off
    USML Cat XI) with the 3E611.a technology-control text; AES reference-table
    bulletin confirming 9D610/9D619/9E610/9E619 600-series software/technology;
    BIS 600-series FAQ confirming the transfer history and USML-to-CCL mapping.
    PASS.
15. 90 FR 29723 (July 7, 2025) - observed the Federal Register final rule
    "Updates to Certain Proscribed Countries and Other Changes" (effective
    Jul 7, 2025) matching the eCFR source note on 120.54; this backs the memo's
    statement that Part 120 was amended in July 2025. PASS.

## Consistency checks

16. Every CFR citation in the memo appears in the source register whitelist
    (enforced by criterion I-4 of the acceptance test). PASS (0 missing).
17. Memo names each of the regulated-object categories it relies on: software
    vs technical data vs commodity (via 120.40 composition terms, 120.33,
    734.3(a)), and states the cargo rule (data classification rides on the
    article) so the reader cannot mistake the tool for the cargo. PASS.
18. No fabricated patent numbers, statute numbers, or FR citations were added:
    all numbers trace to entries in P0-M2/export-control-sources.md observed as
    above. PASS.

## Residual uncertainty (flagged, not resolved)

- 5D002/5D992 ENC classification of a hypothetical closed-binary PDM server is
  not determined (no such binary exists). Flagged [uncertain] in memo section
  5.2 and recommendation 4.
- Specific customer payload classifications are facts of the customer, not of
  OEP; the memo assigns corrective duty via P8 classification attribute and
  contract flow-down (recommendation 6).
- The 2026-10-01 proposed rule (91 FR 62361) is open for comment through
  Nov 30, 2026; its final form may change USML structure/definitions, which is
  why the memo schedules a re-run before first public release.

END OF LOG
