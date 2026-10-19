//! Identity layer for the P8 PDM service (P8-M3-T01).
//!
//! Our own take on the platform-identity concept. The incumbent CAD-PDM
//! platform ships a proprietary "passport" identity product; we adopt the
//! abstract idea (one verified principal identity used across the engineering
//! platform) and build it ourselves, cleanly, with sovereignty defaults:
//!
//! * **Local accounts** are the default and the only path that requires no
//!   third party. They implement the full lifecycle tested by
//!   `tests/identity_account_lifecycle.rs`: create, disable, enable, reset,
//!   with passwords stored as PHC `$argon2id$` strings.
//! * **OIDC is optional and OFF by default** (sovereignty principle from
//!   P0-M5-T02, applied to identity): federated sign-in only appears when an
//!   operator explicitly constructs an [`OidcConfig`] and sets it on
//!   [`IdentityConfig`] (`tests/identity_oidc_default_off.rs`).
//! * **Passport**: after a successful local login the service issues an
//!   [`IdentityPassport`] - the verified principal that RBAC (P8-M3-T02)
//!   turns into a [`RoleSubject`](crate::rbac::RoleSubject) once wired.
//!
//! Nothing here depends on a vendor's semantics or product; it is our own
//! identity model built to the program's sovereignty doctrine.
//!
//! Integration points: `P8-M3/p8m3t01-identity-and-passport.md`.

pub mod account;
pub mod config;
pub mod passport;
pub mod password;
pub mod store;

pub use account::{Account, AccountError, AccountStatus};
pub use config::{IdentityConfig, OidcConfig};
pub use passport::{IdentityPassport, PassportProvider};
pub use password::{hash_password, PasswordError, PasswordPolicy};
pub use store::{AccountStore, AuthError, IdentityService};
