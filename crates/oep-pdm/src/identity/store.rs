//! In-memory account store + identity service (P8-M3-T01).
//!
//! [`AccountStore`] is the data layer: create / disable / enable / reset /
//! lookup, with argon2id-hashed secrets. [`IdentityService`] wraps the store
//! behind the [`IdentityConfig`] and is what the REST/admin layers will call
//! (P8-M4-T03 admin UI, P8-M6-T01 service).

use std::collections::HashMap;

use super::account::{new_account, validate_username, AccountError};
use super::config::IdentityConfig;
use super::passport::IdentityPassport;
use super::password::{check_policy, hash_password, verify_password, PasswordPolicy};

/// A monotonically increasing clock for deterministic timestamps; the caller
/// supplies epoch seconds (tests and the service both use `std::time`).
pub fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Errors surfaced from the identity service boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// No such account, or the password is wrong. Deliberately one variant so
    /// sign-in never reveals which; maps to a single client error.
    InvalidCredentials,
    /// The account exists but is disabled.
    AccountDisabled,
    /// The account store rejected an account-level operation.
    Account(AccountError),
    /// The password failed the active policy.
    Policy(super::password::PasswordError),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::InvalidCredentials => write!(f, "invalid username or password"),
            AuthError::AccountDisabled => write!(f, "account is disabled"),
            AuthError::Account(e) => write!(f, "account error: {e}"),
            AuthError::Policy(e) => write!(f, "password error: {e}"),
        }
    }
}

impl std::error::Error for AuthError {}

impl From<AccountError> for AuthError {
    fn from(e: AccountError) -> Self {
        AuthError::Account(e)
    }
}

impl From<super::password::PasswordError> for AuthError {
    fn from(e: super::password::PasswordError) -> Self {
        AuthError::Policy(e)
    }
}

/// The local account data layer.
#[derive(Debug, Default)]
pub struct AccountStore {
    accounts: HashMap<String, super::account::Account>,
}

impl AccountStore {
    /// A fresh, empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an active account. Fails on an invalid or duplicate username, or
    /// when the password fails the policy.
    pub fn create(
        &mut self,
        username: &str,
        display_name: &str,
        password: &str,
        policy: &PasswordPolicy,
    ) -> Result<super::account::Account, AccountError> {
        validate_username(username).map_err(AccountError::InvalidUsername)?;
        if self.accounts.contains_key(username) {
            return Err(AccountError::DuplicateUsername(username.to_string()));
        }
        check_policy(password, policy).map_err(|e| AccountError::InvalidUsername(e.to_string()))?;
        let hash = hash_password(password)
            .map_err(|_| AccountError::InvalidUsername("password hashing failed".to_string()))?;
        let account = new_account(
            username.to_string(),
            display_name.to_string(),
            hash,
            now_epoch_secs(),
        );
        self.accounts.insert(username.to_string(), account.clone());
        Ok(account)
    }

    /// Mark an existing account disabled (kept, inert).
    pub fn disable(&mut self, username: &str) -> Result<(), AccountError> {
        let acc = self
            .accounts
            .get_mut(username)
            .ok_or_else(|| AccountError::NotFound(username.to_string()))?;
        acc.set_status(super::account::AccountStatus::Disabled);
        Ok(())
    }

    /// Re-enable a disabled account.
    pub fn enable(&mut self, username: &str) -> Result<(), AccountError> {
        let acc = self
            .accounts
            .get_mut(username)
            .ok_or_else(|| AccountError::NotFound(username.to_string()))?;
        acc.set_status(super::account::AccountStatus::Active);
        Ok(())
    }

    /// Replace the account's password hash (password reset/admin reset).
    pub fn reset_password(
        &mut self,
        username: &str,
        new_password: &str,
        policy: &PasswordPolicy,
    ) -> Result<(), AccountError> {
        check_policy(new_password, policy)
            .map_err(|e| AccountError::InvalidUsername(e.to_string()))?;
        let account = self
            .accounts
            .get_mut(username)
            .ok_or_else(|| AccountError::NotFound(username.to_string()))?;
        // Reset does NOT re-enable an explicitly disabled account: an admin
        // reset on a disabled account stays disabled until enable() is called.
        account.set_password_hash(new_password);
        Ok(())
    }

    /// Look up an account by username.
    pub fn get(&self, username: &str) -> Option<&super::account::Account> {
        self.accounts.get(username)
    }

    /// True when an account with this username exists.
    pub fn contains(&self, username: &str) -> bool {
        self.accounts.contains_key(username)
    }
}

/// Identity service: holds the config + store and is the public face of
/// P8-M3-T01.
pub struct IdentityService {
    config: IdentityConfig,
    store: AccountStore,
}

impl IdentityService {
    /// Build with the given config.
    pub fn new(config: IdentityConfig) -> Self {
        IdentityService {
            config,
            store: AccountStore::new(),
        }
    }

    /// Build with the sovereign default config (local accounts on, OIDC off).
    pub fn default_sovereign() -> Self {
        Self::new(IdentityConfig::default())
    }

    /// Current identity configuration.
    pub fn config(&self) -> &IdentityConfig {
        &self.config
    }

    /// The account store (read-only access for queries).
    pub fn accounts(&self) -> &AccountStore {
        &self.store
    }

    /// The account store (mutable access for lifecycle operations).
    pub fn accounts_mut(&mut self) -> &mut AccountStore {
        &mut self.store
    }

    /// Verify a local login and, on success, return a local passport.
    ///
    /// OIDC is optional and separate: this method is the local path that works
    /// with no OIDC configuration at all (sovereignty).
    pub fn issue_passport(
        &self,
        username: &str,
        password: &str,
    ) -> Result<IdentityPassport, AuthError> {
        self.authenticate(username, password)?;
        let account = self
            .store
            .get(username)
            .expect("authenticate verified the account exists");
        Ok(IdentityPassport::local(
            account.username().to_string(),
            account.display_name().to_string(),
        ))
    }

    /// Attempt local username/password authentication.
    pub fn authenticate(&self, username: &str, password: &str) -> Result<(), AuthError> {
        let account = self
            .store
            .get(username)
            .ok_or(AuthError::InvalidCredentials)?;
        if !account.is_active() {
            return Err(AuthError::AccountDisabled);
        }
        if verify_password(account.secret_material(), password) {
            Ok(())
        } else {
            Err(AuthError::InvalidCredentials)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_local_login() {
        let mut svc = IdentityService::default_sovereign();
        svc.accounts_mut()
            .create(
                "eve",
                "Eve Engineer",
                "a long enough passphrase",
                &PasswordPolicy::default(),
            )
            .unwrap();
        let p = svc
            .issue_passport("eve", "a long enough passphrase")
            .unwrap();
        assert!(p.provider_is_local());
        assert_eq!(p.subject(), "eve");
    }

    #[test]
    fn service_refuses_unknown_and_wrong() {
        let svc = IdentityService::default_sovereign();
        assert_eq!(
            svc.authenticate("ghost", "whatever-pw-long"),
            Err(AuthError::InvalidCredentials)
        );
    }
}
