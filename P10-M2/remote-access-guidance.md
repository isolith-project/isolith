# Remote access guidance: your network, your choice

Version: 1.0.0 (2026-10-17)
Status: Ratified (program decision #10, 2026-10-17)
Owner: Open Engineering Platform (OEP) program governance, P10-M2 self-hosted delivery
Applies to: customer deployments of the Open Engineering Platform (oep-studio, oep-pdm, oep-worker)
Cross-references: P8-M5/hybrid-cloud-role-adr.md v1.0.0, P8-M5/p8m5t03-lan-airgap-install-path.md, P0-M2-T04 (export-control memo), P10-M2-T02 (offline/air-gap install bundle)

## 1. The short version

Whether your users reach the platform from outside your network is YOUR decision,
made by YOUR administrators with YOUR tools, inside YOUR network boundary.
Remote access is not a product feature of the Open Engineering Platform (OEP),
and it is not a gateway for us to build.

OEP ships self-hosted, inside your network. There is no vendor cloud to reach,
no vendor-operated remote-access endpoint to connect to, and no requirement
that any part of the platform touch the internet. This document is the guidance
for choosing how (or whether) out-of-network access works for your shop. Read
the default posture in section 2 first; it is the answer for most customers.

## 2. The default posture: everything inside your network

A normal OEP deployment is fully in-network. Every layer runs on hardware you
operate:

- oep-studio, the modeling client, runs on your engineers' workstations.
- oep-pdm, the server (vault, revisions, RBAC, audit, and the web UI), runs on
  an on-premise server or NAS inside your boundary.
- oep-worker compute nodes, when you use them, run inside the same boundary.

This is deliberate. Self-hosting inside your own network is the default and is
the ITAR/export-control posture the program is built around (memo P0-M2-T04,
"pure air gap" in section 5.3): no internet, no telemetry, no phone-home, no
cloud round-trip, no update callback that blocks anything. When the platform has
an update available it flags a status at most; it never fails closed and never
requires reach-back (decision #11). If you deploy it on a machine with no
network access at all, it installs, runs, and passes the same health checks as
an online one, from a signed offline bundle (P10-M2-T02, a companion delivery).

On this default posture, remote access means: users are on your LAN or trusted
VLAN, and they open the PDM web UI in a browser. No VPN, no proxy, no public
endpoint, nothing extra to operate.

## 3. Patterns for out-of-network (roaming) access

All three patterns below run on infrastructure you own and control. OEP neither
provides nor requires any out-of-network endpoint; these are the ways your
administrators can wire roaming access with standard, self-hosted components.
Pick one, or none. There is no correct answer that applies to every shop.

### Pattern A: On-network access (default; zero extra components)

Users on your LAN, VPN, or trusted VLAN reach the PDM web UI directly in a
browser. Nothing is exposed to the public internet. This is the air-gap posture
and the default configuration. It is the right answer for most departments.

### Pattern B: Your own TLS reverse proxy (for example Caddy) on your edge or VPS

If you want a persistent HTTPS endpoint, run a TLS reverse proxy on a host you
control. A small VPS or an edge server in your DMZ both work. Point the proxy
at the PDM web UI and HTTPS at the public host.

What you operate is your call: the TLS certificates, the DNS name, and whatever
authentication sits in front, which can be your single sign-on, client
certificates, IP allowlists, or any combination. Caddy is a convenient default
because it provisions and renews Let's Encrypt certificates for you, but any TLS
reverse proxy you already run works identically, and the guidance does not depend
on any particular proxy product.

### Pattern C: Your own VPN or mesh (for example WireGuard) on your network

Stand up WireGuard, or your existing VPN or mesh, on your own network. Remote
users first join the tunnel, then reach the PDM web UI exactly as if they were
on the LAN. Traffic is end to end encrypted through your tunnel, and no service
is exposed to the public internet at all.

This is the pattern to pick when you want roaming access without any public web
presence. It is also the pattern that maps onto the optional relay role described
in section 4 if you have peers behind NAT.

### Pattern comparison

| Pattern | What it gives you | What you operate |
|---|---|---|
| A. In-network browser access | LAN/VPN users reach the web UI directly | Nothing extra |
| B. TLS reverse proxy (Caddy/VPS) | Persistent public HTTPS URL | Proxy, certs, DNS, front-door auth |
| C. VPN/mesh (WireGuard) | Roaming users without any public endpoint | Tunnel, peers, keys |

## 4. What OEP provides, and what it explicitly does not

Provides:

- A self-hosted platform that runs with zero outbound traffic (section 2), and
  the install, upgrade, and backup stories that keep it that way.
- One optional, opt-in role for a common real problem: peers behind NAT. The
  self-hosted relay role (P8-M5-T03) is relay-only: it forwards encapsulated
  traffic, holds no decryption keys, sees no plaintext, and stores nothing.
  Even that relay runs on your infrastructure, and air-gap remains the default
  for deployments that do not opt in.

Explicitly does not:

- Operates no remote-access endpoint. There is no OEP-operated service to
  connect to from outside your network, and none is required.
- Operates no cloud relay, no vendor VPN, no hosted account, and no phone-home
  path. The platform never fails closed because a vendor server is unreachable,
  because there is no vendor server to reach.
- Builds no gateway and installs no gateway. Remote access for your users is
  composed from your own components (sections 3), not from a shipped gadget.
- Does not place a TLS-terminating intermediary of its own between your users
  and your data. A TLS-terminating proxy that a third party operates would hold
  your decryption keys and see your plaintext; that is rejected as a transport
  (P8-M5-T03). Your own edge proxy in Pattern B is different: it terminates at
  your trust boundary, behind your keys, which is your call to make.

If a deployment has internet access restricted or absent, nothing changes
functionally: the platform is complete without it.

## 5. Export-control and sovereignty notes

For shops that are ITAR-subject or otherwise regulated:

- Start from the pure air-gap posture (P0-M2-T04 section 5.3). No outbound
  traffic by default, and the platform requires none.
- If you enable the relay role, do so only under the memo's stated conditions:
  the export-control custody conditions hold and no 126.1-listed-country touch
  exists at any hop.
- Keep keys, certificates, DNS, and ingress in your own control and, where your
  regulations require, in-country. The platform never routes through a vendor
  selected cloud or a vendor selected country, because there is no vendor
  network in the path at all.

## 6. Companion documents

- P8-M5/hybrid-cloud-role-adr.md v1.0.0: the air-gap default and the opt-in,
  relay-only role record; why plain HTTPS is not the NAT-traversal transport.
- P8-M5/p8m5t03-lan-airgap-install-path.md: the proven LAN/air-gap install path
  for oep-pdm.
- P10-M2-T02: offline/air-gap install bundle (signed USB/ISO, no internet
  required), delivered separately as the sovereignty centerpiece.
- P0-M2-T04: the export-control (ITAR/ECCN) posture memo the above relies on.

## Change log

- 1.0.0 (2026-10-17): initial. Ratifies program decision #10 that remote access
  is the customer's network choice, not a product feature or a gateway to build.
