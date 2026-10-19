---
title: OEP Identity and Passport Concept (P8-M3-T01)
decision: identity_local_accounts_plus_optional_oidc_own_passport
version: 1.0.0
date: 2026-10-17
status: Ratified
owner: Keelworks program, P8 PDM lane (P8-M3 identity)
supersedes: nothing (first identity record)
---

# OEP Identity and Passport Concept

This record is the binding design decision for identity in the Keelworks /
Open Engineering Platform (OEP) PDM. It defines our own account and passport
model, keeps OIDC optional and OFF by default (sovereignty principle from
P0-M5-T02), and documents exactly how far the "passport" idea goes and no
further.

## 1. What we are doing

We adopt the abstract idea behind the incumbent platform's "passport"
identity: *one verified principal identity that follows the user across the
engineering platform* (PDM, modeler, viewer, compute). We implement that idea
ourselves, from our own model, with no vendor product, no vendor code, and no
borrowed semantics.

Accepted:
- A local account store with a real lifecycle: create, disable, enable,
  reset password. Passwords are argon2id-hashed PHC strings, never plaintext.
- After a verified local login, the identity service issues an
  `IdentityPassport` (subject + display name + provider). RBAC (P8-M3-T02)
  converts it into a `RoleSubject` for authorization.
- OpenID Connect (OIDC) federation is available as an explicit opt-in for
  shops that already run an identity provider.

Not done here:
- We do NOT recreate or imitate the vendor's passport-branded product, its
  user flows, its reg card, its sub-process rankings, or its marketing
  semantics. Those are the vendor's product, not the abstract idea.
- We do NOT make OIDC the default. The default configuration has no OIDC
  tenant at all; the local path is the only path that needs no third party.

## 2. Sovereignty defaults (from P0-M5-T02, applied to identity)

The telemetry decision (P0-M5-T02) established the program rule: **defaults
are the product**, and the sovereign default is what ships. Identity follows
the same rule:

1. `IdentityConfig::default()` has `local_accounts_enabled = true` and
   `oidc = None`. Zero OIDC configuration in a default build.
2. OIDC turns on ONLY through an explicit `set_oidc(OidcConfig)` call. There
   is no other path to enable it; a fresh deployment performs no discovery,
   no token exchange, and no outbound traffic for identity.
3. The local authentication path is fully functional with OIDC absent. A
   deployment can run for years on local accounts and never touch a federated
   identity provider.

This is enforced automatically: `crates/oep-pdm/tests/identity_oidc_default_off.rs`
asserts the default has no OIDC provider, that OIDC requires explicit opt-in,
and that local login succeeds with OIDC unconfigured.

## 3. Account lifecycle

The tested lifecycle (identical to the acceptance suite):

1. **create** - new username + display name + password (policy floor 12
   chars). Stored as `$argon2id$` PHC; duplicate and invalid usernames are
   rejected.
2. **disable** - the account exists but is inert: password verification is
   refused even with the correct password. Data is kept.
3. **enable** - reverses disable.
4. **reset password** - replaces the hash; the old password is dead
   immediately. Resetting does NOT re-enable an explicitly disabled account
   (separation of "secret changed" from "account state").

Sign-in never reveals whether a failure was "unknown account" or "wrong
password" (`AuthError::InvalidCredentials` for both) and returns
`AccountDisabled` separately for disabled accounts (so the operator can build
a "your account is locked" screen if desired).

## 4. Why we do not copy the vendor product

Three reasons, matching the program invariants (board.program.json):

1. **Clean-room invariant.** No vendor binaries, no copied source, no
   adoption of vendor code. A "passport" built from our own model keeps the
   identity layer clean-room like every other component.
2. **Trademark and RE posture.** The vendor's passport is a branded product
   (see P0-M2-T03 trademark posture: no public use of vendor marks). Reusing
   its name or semantics would be brand confusion, not interop.
3. **Sovereignty.** The point of the program is that small and sovereign shops
   control their identity and their network. Tying identity to a vendor's
   semantics or a third-party-by-default SSO would recreate the dependence we
   exist to remove.

## 5. The boundary in one sentence

We build OUR OWN identity layer with the abstract idea of a platform
passport; OIDC is an optional, explicit, off-by-default federation point; we
adopt no vendor product, name, flow, or semantics.

## 6. Change log

- 1.0.0 (2026-10-17): initial record.
