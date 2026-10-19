# Obtaining the Official 3DXML Specification (P1-M1-T02)

Version: 1.0, 2026-10-15
Author: Keelworks default worker
**Purpose:** Document the exact process to obtain the Dassault 3DXML specification
(XSD reference schemas + user's guide) under Dassault's free internal-use license,
per program clean-room doctrine (public specs and licensed materials only, no
vendor binaries, no copied source). This is a process record, not a redistribution
of the spec itself.

## Executive summary

The 3DXML format specification is proprietary to Dassault Systèmes, but Dassault
grants the documentation under a yearly, royalty-free, non-exclusive,
non-transferable license to anyone who requests it, limited to the requester's
own internal business purposes. The request is a self-service download from
Dassault's software-downloads portal: it requires a free 3DEXPERIENCE/3DS Passport
account (a login wall), then selecting "3D XML XSD Schema" from the exchange/
interoperability downloads. This matches public guidance in Dassault's own
3DEXPERIENCE help ("To ask for a documentation, browse the ... internet site then
choose 3D XML XSD Schema") and UTF-8 help mirrors.

Key constraint for Keelworks: the license grants internal use only, so the XSD and
user's guide must NOT be republished. Do not republish the XSD or user's guide in
any public or open repository, bundle them into a shipped product, or quote them
beyond fair use. Keelworks may hold and read the spec internally (private NAS repo)
to build its clean-room 3DXML reader, mirroring how the program treats STEP/AP242
reference material, but the corpus stays license-clean.

## Step 1. Understand what is being licenced

- Material: "3D XML XSD" reference schemas (XSDs) and the "3D XML User's Guide".
- Historical official downloads URL family: https://www.3ds.com/3ds-passport/downloads/
  and the older https://www.3ds.com/3dxml (both now redirect to Dassault's current
  Software Downloads portal, software.3ds.com lineage).
- Announcement: Dassault press release "Dassault Systemes Delivers 3D XML
  Specifications and Player" (2005-06-15) noting free availability of the
  Specifications and Player.
  https://www.3ds.com/newsroom/press-releases/dassault-systemes-delivers-3d-xml-specifications-and-player

## Step 2. Create or log in to a free 3DS Passport (3DEXPERIENCE ID)

1. Go to https://www.3ds.com/products-services/3d-xml/downloads (or
   https://www.3ds.com/3ds-passport/downloads/), which routes to the current
   Dassault Software Downloads portal.
2. Create a free "3DEXPERIENCE ID" / "3DS Passport" account (name, email,
   password) or sign in with an existing one. This is a public signup; no Dassault
   product purchase or support contract is required for this download family.
3. Confirm the email to activate the account.

The account step is the actual "request" for the licence: Dassault's portal
distributes the XSDs only to signed-in users, and the act of downloading them
constitutes acceptance of the license agreement.

## Step 3. Locate and download "3D XML XSD Schema"

1. From the Software Downloads portal, navigate to the interoperability /
   format-specification downloads (the "3D XML XSD Schema", "3D XML User's Guide"
   and optionally "3D XML Player" entries).
2. Download the 3D XML XSD archive and the user's guide.
3. Retain the license text and the filenames/versions as provenance for the
   internal spec register (keep in the private corpus, do not commit to a public
   branch).

If the entry is not immediately visible under "3D XML", search the portal for
"3D XML XSD" or "3D XML User's Guide"; the portal layout changes over time, and
the same items may be listed under CATIA or 3DEXPERIENCE interoperability
download categories.

## Step 4. Record the licence terms (do not redistribute)

Per the official 3DXML license (as cited in public references, e.g. Wikipedia's
3DXML article), Dassault Systèmes grants a royalty-free, non-exclusive,
non-transferable and temporary licence to use and copy the materials for the
licensee's own internal business purposes. Implications documented for the program:

- INTERNAL USE ONLY. The XSDs and user's guide must live on the private NAS
  corpus, not in any public repo, package, or publication.
- The format documentation may be READ to implement a clean-room reader/writer,
  which is exactly the P3 workstream plan; it must not be copied into our
  source or shipped.
- Because 3DXML geometry (Gregory patches, surface mesh, tessellated mesh) is
  described in the spec, holding the spec internally removes the "unknown
  schema" uncertainty for the P3 reader, while the corpus samples provide
  real-material verification.

## Alternative / fallback observations (not a substitution for the licence)

- Public discussions and the open-source GLC_Player (GPL) already read ASCII
  3DXML from observed structure, and trimesh ships a threedxml loader tested on
  the MIT sample set, so some capability is observable without the official spec.
  These are lawfully-observed partial references, not a replacement for the
  licensed spec, and they concern only the ASCII branch.
- The ASCII branch being open-tooling-readable today de-risks P3-M1/P3-M2; the
  binary `.3DRep` branch (started in P3-M3) will lean on the licensed spec plus
  the corpus.

## Status (2026-10-15)

- [x] Process documented and source URLs verified live.
- [ ] Account created / XSD downloaded: PENDING, requires a human to complete the
  public 3DS Passport signup (a credentials step Hermes cannot perform for Don).
  Action: Don (or a profile with web credentials) completes Steps 2-3, stores the
  resulting XSD + user's guide under the private corpus (e.g. corpus/spec/3dxml/,
  gitignored or on the private-only branch), and this task's register is updated.

## Related

- Catalog of public 3DXML samples: [3dxml-samples.md](3dxml-samples.md)
- Source register: [sources.md](sources.md)
- Real-material verification: [verification.md](verification.md)
