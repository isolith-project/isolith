//! Password policy and hashing (P8-M3-T01).
//!
//! Passwords are hashed with **argon2id** (the `argon2` crate, RustCrypto,
//! pure Rust, no system deps) and stored as PHC strings
//! (`$argon2id$v=19$m=...t=...p=...$salt$hash`) - the standard format the
//! acceptance test re-parses and any conformant verifier can check. The plain
//! password is never stored, logged, or returned.

use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
};

/// Minimum password length (policy floor).
pub const MIN_PASSWORD_LEN: usize = 12;

/// Password strength policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasswordPolicy {
    /// Minimum length.
    pub min_len: usize,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        PasswordPolicy {
            min_len: MIN_PASSWORD_LEN,
        }
    }
}

/// Password-domain errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordError {
    /// Too short for the active policy.
    TooShort {
        /// Minimum length required by the policy.
        min: usize,
        /// Length actually supplied.
        got: usize,
    },
    /// The stored hash is unparsable or corrupt.
    CorruptHash,
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PasswordError::TooShort { min, got } => {
                write!(f, "password too short (min {min}, got {got})")
            }
            PasswordError::CorruptHash => write!(f, "stored password hash is corrupt"),
        }
    }
}

impl std::error::Error for PasswordError {}

/// Check a candidate password against the policy (length floor only; the
/// program bans composition complexity rules, which add no real security).
pub fn check_policy(password: &str, policy: &PasswordPolicy) -> Result<(), PasswordError> {
    if password.chars().count() >= policy.min_len {
        Ok(())
    } else {
        Err(PasswordError::TooShort {
            min: policy.min_len,
            got: password.chars().count(),
        })
    }
}

/// Hash a password with argon2id and a fresh random salt, returning the PHC
/// string for storage. `Err` only on a vetted-KDF failure (never on policy:
/// call [`check_policy`] first).
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let phc = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| PasswordError::CorruptHash)?
        .to_string();
    Ok(phc)
}

/// Constant-time verify a password against a stored PHC string. Returns
/// `true` only on an exact match; malformed stores verify as `false`.
pub fn verify_password(stored: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_floor() {
        assert!(check_policy("123456789012", &PasswordPolicy::default()).is_ok());
        assert!(check_policy("short", &PasswordPolicy::default()).is_err());
    }

    #[test]
    fn hash_round_trip_phc() {
        let pw = "a suitably long passphrase !";
        let hash = hash_password(pw).unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(!hash.contains(pw));
        assert!(verify_password(&hash, pw));
        assert!(!verify_password(&hash, "a different passphrase"));
    }

    #[test]
    fn corrupt_hash_verifies_false() {
        assert!(!verify_password("not-a-phc-string", "anything"));
    }
}
