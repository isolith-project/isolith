# Isolith: Name and Public Product Brand for Open Engineering Platform (OEP)

Status: Selected and trademark-checked by P13-T08 (2026-10-18). Formal legal memo P0-M2-T03 owns the final clearance and any attorney review. On 2026-10-20 Don approved the full name change: the internal codename is retired and Isolith is the single program name (internal and public). On 2026-10-27 Don directed the public identity rollout under the Isolith name (card NAME-AND-ASSET-RESERVATIONS t_a15e108f); the public-handle decision and asset map are recorded below pending his pick.
Date: 2026-10-18 (name record v1.0), rename record v1.1 2026-10-20, public asset map v1.2 2026-10-28
Version: 1.3
Owner: Don (final), P13-T08 worker (selection, design, and collision check)

## Decision

The program name and public product brand is **Isolith** (pronounced eye-so-lith;
two syllables stressed on the first). It is the name used for the program, the
product, on the docs site, in release notes, and on packaging. The former
internal codename, Keelworks, is retired: it collided with a registered mark and
is no longer used on any surface.

Fixed naming map, re-stated so it cannot drift:

| Name | Use | Public? |
|---|---|---|
| Isolith | Program name and public product brand. Wordmark "isolith". | Yes |
| Open Engineering Platform (OEP) | Descriptive working title and expanded name ("Isolith: Open Engineering Platform"). Never the registrable brand. | Yes |
| Keelworks | Former internal codename. Retired 2026-10-20 (decision record 0001, rename record v1.1) because it collides with the registered KEELWORKS mark (KeelWorks Ltd UK, classes 9/41/42). Not used on any surface. | No |

## Trademark check (same method as P0-M1-T02, live registries, 2026-10-18)

Search method: live queries of USPTO Trademark Search (tmsearch.uspto.gov), the
WIPO Global Brand Database / TMview aggregate (branddb.wipo.int, 89 data sources
including EUIPO), and public-web common-law checks. Full machine-readable
results are in evidence/trademark-evidence-2026-10-18.json, and the note in
evidence/trademark-collision-check-note.md.

Finding for "Isolith":

- USPTO wordmark search returns zero results for "isolith" in any class. No US
  filing or registration exists under the identical wordmark.
- WIPO Global Brand Database / TMview returns 5 records, all in non-software
  classes: class 2 (paints/coatings), class 5 (dental/pharmaceutical products),
  class 19 (building and insulation materials). No record in classes 9 and 42
  (software, and software design and development services), which are the
  classes a CAD/CAE/PDM platform uses.
- Common-law users are small and outside the goods: an Austrian wood-wool
  insulation maker (class 19 field), a UK business-services firm with no
  software or trademark registration, and a tiny MIT licensed isometric video
  game from 2026. The game is the only software-space use; it is an
  entertainment product with no registered mark, distinct goods from an
  engineering suite. It is recorded in the evidence for the P0-M2-T03 memo to
  weigh; it does not block a preliminary clearance.

Finding for the rejected runners-up (all checked in the same registries;
details in the evidence file):

- Keelworks: the control. Still registered (US 7153038 / serial 79353104,
  classes 9/41/42), confirming the method catches a real collision. This is the
  mark the retired codename collided with.
- Geolith: live US registration 8147704 covering class 42. Rejected.
- Stratolith: live US application 50149416 (classes 25/35), plus a weak name.
  Rejected.
- Orthoplex, Isoplex, Keyline: heavier prior art (live US/EU marks, 20+150+
  WIPO records). Rejected.

## Why this name

- Distinctive, coined, and registrable in prospect: "iso" (standards, equal) +
  "-lith" (stone; a printing plate, the root of lithography, and by extension a
  blueprint image). An isometric line drawing is an isometric drawing; the name
  points at engineering drawing and at solid, dependable tooling.
- Short, one invented word, low collision surface, works as a wordmark and as a
  monogram (mark + "isolith").
- No nautical association with the old codename, no part of the rejected class
  the codename occupied, no "CATIA", no descriptor that reads generic.
- Carries the program's honesty posture: the mark is a solid engineering block,
  not an overclaim.

No CATIA in public naming; no "CATIA" ever appears in a name, mark, or
branding for this product (program rule, repeatedly recorded since decision
record 0001).

## Hand-off

- The docs site (P13-T03) uses Isolith: wordmark, mark, palette tokens and
  typography from this kit (assets/brand/). See README.md in this folder for the
  exact consumption contract.
- P0-M2-T03 (formal trademark memo) uses this evidence file as its primary
  material and adds the legal analysis (likelihood of confusion against the
  class 2/5/19 marks and the common-law users, THP, attorney review
  recommendation).

## Decision record

- Program name and public product brand: Isolith.
- Public working title: Open Engineering Platform (OEP).
- Former internal codename: Keelworks (retired 2026-10-20; superseded by
  Isolith on every surface).
- Collision check date: 2026-10-18 (evidence file linked above).
- Registries searched: uspto, euipo, wipo, tmview.
- Next owner: P0-M2-T03 trademark memo (formal).

## Public identity and asset map (v1.3, 2026-11-20)

One table. This is the single source for the public identity assets below;
no document or workflow invents a second name, handle, or path. Availability
evidence (per-platform probes with timestamp) is in
`evidence/name-handle-availability-2026-10-28.json`.

| Asset | Value | Status / owner |
|---|---|---|
| Product name | Isolith | Decided (v1.0). Wordmark "isolith". |
| Domain | isolith.dev (recommended primary), isolith.org (defensive) | Recommended by the release/community plan; registration is a Don item (card SITE-DOMAIN-ORIGIN t_ad10a5f5). isolith.com/.net are held by others; RDAP records in the plan. |
| GitHub org or account | `isolith-project` (org id 358919476, owned by Don's personal account `dmltallen`) | **Decided 2026-11-19 and created.** Canonical clone URL `https://github.com/isolith-project/isolith`. The R1A account `redonealpha-don` never hosts Isolith. |
| crates.io prefix | `oep-` (all published crates keep the existing `oep-` prefix, e.g. `oep-stack`, `oep-archive`) | Decided by namespace ownership: crates.io `isolith` is claimed only as a placeholder 0.0.0 crate if Don approves the squatting policy; otherwise no crates.io reservation. |
| Image path (public) | `ghcr.io/isolith-project/oep-stack` | Follows the org; Docker Hub namespace is free but GHCR is preferred. |
| npm package | `isolith` package (404 = free), claimed with a placeholder README once the handle is picked (needs Don's npm account/token) | Pending pick + credential. |
| PyPI | Held only if a Python package will ever ship; otherwise not squatted | Policy call (this card). |

Crate/image naming map (how the same artifact is referenced today, so the sweep
below has exact targets):

| Today's identifier | File types | Becomes (once handle + domain exist) |
|---|---|---|
| `http://192.168.0.101:3200/don/isolith` | Cargo.toml workspace `repository` | `https://github.com/isolith-project/isolith` (swept 2026-11-20) |
| `http://192.168.0.101:3200/isolith/isolith.git` | Gitea CI clone refs `.gitea/workflows/*.yaml` | Public GitHub clone URL (workflows already port to `.github/` per PUBLIC-REPO-HARDENING) |
| `http://gitea/isolith/isolith.git` | docs-site clone instructions | Public GitHub clone URL |
| `isolith/isolith` (repo path) | tests, python tooling, workstream docs | `github.com/<handle>/isolith` or keep Gitea-internal ref where the tool talks to Gitea |
| `isolith/oep-stack:0.1.0` (image) | deploy compose, Dockerfile label, tests, docs-site rows, proofs, waivers, update-checker | `ghcr.io/<handle>/oep-stack:<tag>` |
| `SITE-DOMAIN-PLACEHOLDER` / `SITE-DOMAIN-ORIGIN` | CODE_OF_CONDUCT, SECURITY, GOVERNANCE | `isolith.dev` once SITE-DOMAIN-ORIGIN wires it |

Sweep inventory (deliverable 4 output, machine-complete): see
`docs/name-handle-sweep-2026-10-28.md`.
