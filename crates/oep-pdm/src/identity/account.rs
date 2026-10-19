//! Local account model (P8-M3-T01).

use serde::{Deserialize, Serialize};

fn utc_rfc3339(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let day_secs = epoch_secs % 86_400;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        day_secs / 3600,
        (day_secs % 3600) / 60,
        day_secs % 60
    )
}

/// Howard Hinnant's `civil_from_days` (public domain), used by the vault
/// store for deterministic RFC 3339 timestamps.
fn civil_from_days(z: i64) -> (i64, u64, u64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    ((if m <= 2 { y + 1 } else { y }), m as u64, d as u64)
}

/// Lifecycle state of a local account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    /// The account may authenticate and be used.
    Active,
    /// The account exists but is inert: sign-in is refused, its data is kept.
    Disabled,
}

/// A local PDM account. Passwords are never stored: only the PHC argon2id
/// hash (see [`crate::identity::password`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    username: String,
    display_name: String,
    status: AccountStatus,
    /// PHC `$argon2id$` string, never the plaintext password.
    password_hash: String,
    created_at: String,
    updated_at: String,
}

impl Account {
    /// Name this account logs in with (unique within the store).
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Human display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Lifecycle state.
    pub fn status(&self) -> AccountStatus {
        self.status
    }

    /// True when the account may authenticate.
    pub fn is_active(&self) -> bool {
        self.status == AccountStatus::Active
    }

    /// The stored PHC hash. Exposed for auditing and for the hash-format
    /// acceptance check; never a plaintext password.
    pub fn secret_material(&self) -> &str {
        &self.password_hash
    }

    /// RFC 3339 UTC creation time.
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    /// RFC 3339 UTC last mutation time.
    pub fn updated_at(&self) -> &str {
        &self.updated_at
    }

    /// Set the lifecycle status (store-internal mutation).
    pub(crate) fn set_status(&mut self, status: AccountStatus) {
        self.status = status;
    }

    /// Replace the password hash (store-internal mutation). If hashing fails
    /// the previous hash is kept (the account stays valid under its old
    /// secret; a failing reset is a rare operational problem, not a reason to
    /// brick the account).
    pub(crate) fn set_password_hash(&mut self, password: &str) {
        if let Ok(hash) = super::password::hash_password(password) {
            self.password_hash = hash;
        }
    }
}

/// Domain errors for account lifecycle operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    /// The username is reserved or invalid (see `validate_username`).
    InvalidUsername(String),
    /// The username already exists.
    DuplicateUsername(String),
    /// No account with this username exists.
    NotFound(String),
    /// The operation requires an active account but it is disabled.
    AccountDisabled(String),
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountError::InvalidUsername(u) => write!(f, "invalid username: {u}"),
            AccountError::DuplicateUsername(u) => write!(f, "duplicate username: {u}"),
            AccountError::NotFound(u) => write!(f, "no such account: {u}"),
            AccountError::AccountDisabled(u) => write!(f, "account disabled: {u}"),
        }
    }
}

impl std::error::Error for AccountError {}

/// Validate a login name: non-empty, at least 3 chars, only `[a-z0-9._-]`.
pub fn validate_username(username: &str) -> Result<(), String> {
    let ok = !username.is_empty()
        && username.len() >= 3
        && username.chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-' || c == '_'
        });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "usernames must be 3+ chars of [a-z0-9._-], got {username:?}"
        ))
    }
}

/// Build a valid [`Account`] from verified fields (only the store constructs
/// these, after running `validate_username` + password policy).
pub(crate) fn new_account(
    username: String,
    display_name: String,
    password_hash: String,
    now: u64,
) -> Account {
    let ts = utc_rfc3339(now);
    Account {
        username,
        display_name,
        status: AccountStatus::Active,
        password_hash,
        created_at: ts.clone(),
        updated_at: ts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn username_validation() {
        assert!(validate_username("alice").is_ok());
        assert!(validate_username("a-b_c.1").is_ok());
        assert!(validate_username("ALICE").is_err());
        assert!(validate_username("ab").is_err());
        assert!(validate_username("").is_err());
        assert!(validate_username("has space").is_err());
    }

    #[test]
    fn civil_from_days_epoch() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(0), (1970, 1, 1));
    }
}
