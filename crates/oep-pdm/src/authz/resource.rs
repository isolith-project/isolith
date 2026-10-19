//! Vault-object tenant resolution (P11-T03).
//!
//! The gate tenant-scopes every object-level request to the tenant that owns
//! the target item. The vault model has two free-form identity tags on an
//! item: `owner` (P8-M1-T02 "owner/principal tag") and `context` (the
//! classification/context tag). This module defines the boundary convention:
//! the owning tenant is `owner` when present, falling back to `context`.

use crate::api::Item;
use crate::store::{StoreError, VaultStore};

/// The tenant a vault item belongs to, per the boundary convention.
///
/// `None` for an unscoped item: an unscoped object has no tenant boundary,
/// so any authenticated reader may see it (the cross-tenant rule only fires
/// when BOTH the subject and the resource carry a tenant that differs).
pub fn item_tenant(item: &Item) -> Option<String> {
    item.owner
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| item.context.as_deref().filter(|s| !s.is_empty()))
        .map(str::to_string)
}

/// Resolve the tenant an item id owns, if the item exists.
///
/// `StoreError::NotFound` is returned verbatim so the gate can decide whether
/// the object truly does not exist (defer to the handler's `404`) versus
/// exists-but-cross-tenant (the gate's own IDOR-masked `404`).
pub fn lookup_item_tenant(store: &VaultStore, id: &str) -> Result<Option<String>, StoreError> {
    match store.get_item(id) {
        Ok(item) => Ok(item_tenant(&item)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(owner: Option<&str>, context: Option<&str>) -> Item {
        use crate::api::Item;
        Item {
            id: "it".into(),
            name: "Box".into(),
            kind: "part".into(),
            description: None,
            owner: owner.map(str::to_string),
            context: context.map(str::to_string),
            created_at: "2026-10-18T00:00:00Z".into(),
            latest_revision: None,
            revision_count: 0,
        }
    }

    #[test]
    fn owner_wins_over_context() {
        assert_eq!(
            item_tenant(&item(Some("Tenant-A"), Some("Tenant-B"))),
            Some("Tenant-A".into())
        );
    }

    #[test]
    fn context_is_the_fallback() {
        assert_eq!(
            item_tenant(&item(None, Some("Tenant-C"))),
            Some("Tenant-C".into())
        );
    }

    #[test]
    fn unscoped_is_none() {
        assert_eq!(item_tenant(&item(None, None)), None);
        assert_eq!(item_tenant(&item(Some(""), None)), None);
    }
}
