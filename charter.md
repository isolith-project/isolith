# Open Engineering Platform (OEP) Charter

Program name and working brand: Isolith. It replaced the former internal
codename (Keelworks), which was retired 2026-10-20 because it collided with a
registered mark (see `assets/brand/NAME.md` and decision record 0001).

## Mission

A sovereign, self-hosted, interoperable professional engineering suite: an
open-source replacement for CATIA-class CAD/CAE/PDM, with hybrid-cloud compute
and no per-seat paywall. The winnable shape is data sovereignty and clean
ownership of design data, expressed as sovereignty and resilience rather than
as an attack on any specific company. Read the proprietary exchange formats
primes actually swap (CGR, 3DXML, CATPart) first, cover the derived
deliverables with the documented formats (STEP AP242, QIF, JT), and only then
close on native formats, gated.

## Non-goals

- Feature-for-feature parity with CATIA or any other commercial package.
- Replacing commercial tools in every workflow. The target is the exchange
  format and platform layer first.
- Any public release before a working CGR reader and the legal memo exist.

## License posture

- Apache-2.0 for applications and libraries; AGPL-3.0 for the server/PDM and
  P2P orchestration components. Full texts at `LICENSE-APACHE` and
  `LICENSE-AGPL`, index at `LICENSE`, third-party attribution in `NOTICE`.
- The per-crate boundary is fixed in `docs/licensing-boundary.md` (JSON copy
  at `docs/licensing-boundary.json`) and enforced in CI by
  `scripts/licensing-check.py`. This prose is a summary; the map is the rule.
- No GPL-3.0 in delivered software (defense procurement legal review
  routinely disallows it).

## Working doctrine

- Done means a test passed on real material, merged and green on main. Never
  a worker log line, never an open pull request.
- Clean room only: no vendor binaries, no copied source, no
  Dassault/Autodesk/Siemens libraries. Public specs plus lawful observation
  only. Legal review before first public release.
- One change at a time, enforced at the milestone level.
- Exactly two repositories: the canonical development repo and one canary.
  Per-workstream, per-wave, per-card, mirror, and fork repositories are
  forbidden without a recorded human decision.
- Every wave integrates onto a single `wave/N` branch and closes with one
  documented checkpoint commit onto main.

## Documentation

- Public docs site (getting started, admin guide, honest format matrix):
  `docs-site/index.html`, served by the `oep-docs` service.
- Repo layout map: `docs/layout.md`.
- Identity kit (mark, palette, usage rules, trademark evidence):
  `assets/brand/`.
