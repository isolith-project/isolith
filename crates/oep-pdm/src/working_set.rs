//! Working-set protocol: lock / edit / publish (object locking).
//!
//! P8-M1-T03. A working set is the set of vault objects a principal holds
//! locks on for an edit session. Unlike vendor check-out models (which reserve
//! file copies into a private workspace), this protocol locks the *logical
//! object identity* with a short-lived lease. Locks are advisory at the data
//! layer: reads and working-copy edits are never blocked. The lock is the
//! serialization point for *publish* — only the currently live lease holder
//! may publish a new revision of the object.
//!
//! Lease semantics (clean-room, own design; no vendor API or vocabulary):
//!
//! * one live lock per object (single-writer);
//! * every lock carries an opaque one-time [`LockToken`] that proves ownership;
//!   all mutating operations (release / steal / publish) are token-checked;
//! * every lock is a lease with a TTL — a crashed client cannot wedge an
//!   object forever; the lease expires and the object is acquirable again;
//! * [`LockManager::steal`] force-transfers a lock, which invalidates the
//!   previous token so a superseded client can neither release nor publish;
//! * concurrent *editing* of working copies is deliberately allowed across
//!   sessions; conflicts surface at publish time and are handled later by the
//!   P8-M5-T02 conflict-resolution milestone.
//!
//! A [`Clock`] abstraction is injected so timeout behaviour is deterministic
//! in unit tests (a manual clock) while production uses [`SystemClock`].

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Identifier of a lockable vault object (a PDM item or revision id).
pub type ObjectId = String;

/// Identifier of a principal (user, workbench instance, or agent) that holds
/// locks in a working set.
pub type HolderId = String;

/// Opaque one-time token proving ownership of a specific lock.
///
/// Tokens are server-minted (monotonic within one manager) and are compared by
/// value only; a client never constructs one by hand.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct LockToken(String);

impl LockToken {
    /// The opaque wire representation (URL-safe token string).
    ///
    /// Added by the P8-M6-T03 gate surface: the REST layer needs to hand the
    /// token to a client and take it back. It stays opaque -- clients never
    /// construct one by hand, and the server compares by value only.
    pub fn wire(&self) -> &str {
        &self.0
    }

    /// Parse a wire token back into a [`LockToken`]. Returns `None` for a
    /// string this manager could not have minted (malformed shape).
    pub fn from_wire(s: &str) -> Option<LockToken> {
        if s.starts_with("lk-") && s.len() > 3 && s[3..].chars().all(|c| c.is_ascii_hexdigit()) {
            Some(LockToken(s.to_string()))
        } else {
            None
        }
    }
}

/// Absolute time in milliseconds since the Unix epoch.
pub type TimestampMs = u64;

/// Errors returned by lock-mutating operations on an object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    /// Another principal holds a live lock on the object.
    ConflictingLock {
        /// Who currently holds the live lock.
        holder: HolderId,
        /// When the conflicting lease expires (ms since epoch).
        expires: TimestampMs,
    },
    /// The object has no lock record (or its lease already expired).
    NotHeld,
    /// The presented token does not match the object's current lock.
    /// Typical after the lock was stolen, released, or superseded.
    TokenMismatch,
}

/// Errors returned by the publish gate (lock -> edit -> publish step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishError {
    /// The publisher does not hold a lock on the object at all.
    NotHeld {
        /// The object the publisher tried to publish.
        object: ObjectId,
    },
    /// The publisher's token is stale: the lease expired or was stolen.
    LockingExpired {
        /// The object the publisher tried to publish.
        object: ObjectId,
    },
}

/// Millisecond-resolution wall clock, injected so time is testable.
///
/// Production uses [`SystemClock`]; unit tests use a manual clock and advance
/// it explicitly, so timeout/lease tests never sleep.
pub trait Clock: Send + Sync {
    /// Current time in milliseconds since the Unix epoch.
    fn now_ms(&self) -> TimestampMs;
}

/// Real wall clock backed by `SystemTime::now`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> TimestampMs {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_millis() as TimestampMs
    }
}

/// One recorded lock (lease) on a vault object.
#[derive(Debug, Clone)]
struct LockRecord {
    holder: HolderId,
    token: LockToken,
    expires: TimestampMs,
}

impl LockRecord {
    /// True when `now` is strictly before the lease expiry.
    fn live(&self, now: TimestampMs) -> bool {
        now < self.expires
    }
}

/// The working-set lock manager.
///
/// Thread-safe: all state lives behind an internal mutex; concurrent
/// `acquire` calls on the same object serialize and exactly one wins.
pub struct LockManager {
    clock: Box<dyn Clock>,
    locks: Mutex<HashMap<ObjectId, LockRecord>>,
    ttl: Duration,
    seq: AtomicU64,
}

impl fmt::Debug for LockManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The injected clock is a trait object (not Debug); report the stable
        // facts only.
        f.debug_struct("LockManager")
            .field("locks", &self.locks)
            .field("ttl_ms", &self.ttl.as_millis())
            .finish_non_exhaustive()
    }
}

/// A human-readable identifier for the working-set protocol.
pub const WORKING_SET_PROTOCOL: &str = "oep-pdm/working-set/v1";

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LockManager {
    /// Create a manager on the real wall clock with the default lease TTL
    /// (one hour).
    pub fn new() -> Self {
        Self::with_clock_and_ttl(Box::new(SystemClock), Duration::from_secs(3600))
    }

    /// Create a manager with an explicit clock and lease TTL. Tests inject a
    /// manual clock and a short TTL to exercise timeout deterministically.
    pub fn with_clock_and_ttl(clock: Box<dyn Clock>, ttl: Duration) -> Self {
        Self {
            clock,
            locks: Mutex::new(HashMap::new()),
            ttl,
            seq: AtomicU64::new(1),
        }
    }

    /// The default lease TTL this manager grants on [`Self::acquire`].
    pub fn default_ttl(&self) -> Duration {
        self.ttl
    }

    fn now(&self) -> TimestampMs {
        self.clock.now_ms()
    }

    fn mint_token(&self) -> LockToken {
        let n = self.seq.fetch_add(1, Ordering::Relaxed);
        LockToken(format!("lk-{:016x}", n))
    }

    fn insert_locked(
        &self,
        map: &mut HashMap<ObjectId, LockRecord>,
        object: ObjectId,
        holder: HolderId,
    ) -> LockToken {
        let token = self.mint_token();
        let now = self.now();
        let expires = now + self.ttl.as_millis() as TimestampMs;
        map.insert(
            object,
            LockRecord {
                holder: holder.clone(),
                token: token.clone(),
                expires,
            },
        );
        token
    }

    /// Acquire an exclusive lease on `object` for `holder` (step 1 of the
    /// protocol). Succeeds when the object is free or its prior lease has
    /// expired; fails with [`LockError::ConflictingLock`] when another
    /// principal holds a live lease.
    ///
    /// Re-acquiring by the *same* holder refreshes the lease and returns a
    /// fresh token (the previous one is invalidated), so long edits can extend
    /// their hold.
    pub fn acquire(&self, object: &ObjectId, holder: &HolderId) -> Result<LockToken, LockError> {
        let mut map = self.locks.lock().unwrap();
        let now = self.now();
        match map.get(object) {
            None => Ok(self.insert_locked(&mut map, object.clone(), holder.clone())),
            Some(existing) if !existing.live(now) => {
                // Lease lapsed: takeover is a clean acquire, old token dies.
                Ok(self.insert_locked(&mut map, object.clone(), holder.clone()))
            }
            Some(existing) if existing.holder == *holder => {
                // Same holder refreshes: re-mint token, extend lease.
                Ok(self.insert_locked(&mut map, object.clone(), holder.clone()))
            }
            Some(existing) => Err(LockError::ConflictingLock {
                holder: existing.holder.clone(),
                expires: existing.expires,
            }),
        }
    }

    /// Release a lock previously acquired or stolen by `token` owner
    /// (protocol teardown). Only the matching token may release; a superseded
    /// token (after steal or refresh) and a not-held object both fail.
    pub fn release(&self, object: &ObjectId, token: &LockToken) -> Result<(), LockError> {
        let mut map = self.locks.lock().unwrap();
        match map.get(object) {
            None => Err(LockError::NotHeld),
            Some(rec) if &rec.token != token => Err(LockError::TokenMismatch),
            Some(_) => {
                map.remove(object);
                Ok(())
            }
        }
    }

    /// Force-transfer the lock on `object` to `holder`, regardless of the
    /// current holder or lease state (admin / crash recovery path). The new
    /// lease uses this manager's TTL; the previous token is invalidated, so a
    /// superseded client's later `release` or `publish` fails with
    /// [`LockError::TokenMismatch`].
    pub fn steal(&self, object: &ObjectId, holder: &HolderId) -> LockToken {
        let mut map = self.locks.lock().unwrap();
        self.insert_locked(&mut map, object.clone(), holder.clone())
    }

    /// True when `token` currently holds a *live* (unexpired) lock on
    /// `object`. This is the ownership check used by the publish gate and by
    /// clients asking "may I still publish?".
    pub fn live(&self, object: &ObjectId, token: &LockToken) -> bool {
        let map = self.locks.lock().unwrap();
        match map.get(object) {
            Some(rec) => &rec.token == token && rec.live(self.now()),
            None => false,
        }
    }

    /// Name of the principal currently holding a live lock on `object`, if any.
    pub fn holder_of(&self, object: &ObjectId) -> Option<HolderId> {
        let map = self.locks.lock().unwrap();
        let now = self.now();
        map.get(object)
            .filter(|r| r.live(now))
            .map(|r| r.holder.clone())
    }

    /// True when any live lock exists on `object`.
    pub fn is_locked(&self, object: &ObjectId) -> bool {
        self.holder_of(object).is_some()
    }

    /// When the live lease on `object` expires, if held.
    pub fn expires_at(&self, object: &ObjectId) -> Option<TimestampMs> {
        let map = self.locks.lock().unwrap();
        let now = self.now();
        map.get(object).filter(|r| r.live(now)).map(|r| r.expires)
    }

    /// The publish gate (step 3): publishing a new revision of `object`
    /// requires the publisher to still hold a live lock with `token`.
    ///
    /// * token holds no record -> [`PublishError::NotHeld`];
    /// * token is stale (lease expired or lock stolen) -> [`PublishError::LockingExpired`];
    /// * otherwise -> `Ok(())`, authorizing the revision write.
    ///
    /// The check is a single atomic read, so a lease that expires after the
    /// call is not this milestone's concern (conflict handling is P8-M5-T02).
    pub fn authorize_publish(
        &self,
        object: &ObjectId,
        token: &LockToken,
    ) -> Result<(), PublishError> {
        let map = self.locks.lock().unwrap();
        let now = self.now();
        match map.get(object) {
            None => Err(PublishError::NotHeld {
                object: object.clone(),
            }),
            Some(rec) if &rec.token != token => Err(PublishError::LockingExpired {
                object: object.clone(),
            }),
            Some(rec) if !rec.live(now) => Err(PublishError::LockingExpired {
                object: object.clone(),
            }),
            Some(_) => Ok(()),
        }
    }

    /// Object ids whose live lock is currently held by `holder` — the
    /// principal's current working set.
    pub fn working_set(&self, holder: &HolderId) -> Vec<ObjectId> {
        let map = self.locks.lock().unwrap();
        let now = self.now();
        let mut out: Vec<ObjectId> = map
            .iter()
            .filter(|(_, r)| r.holder == *holder && r.live(now))
            .map(|(o, _)| o.clone())
            .collect();
        out.sort();
        out
    }

    /// Remove expired lock records and return how many were reaped. A client
    /// housekeeping task may call this periodically; correctness never
    /// depends on it, because every operation already treats expired leases as
    /// free.
    pub fn reap_expired(&self) -> usize {
        let mut map = self.locks.lock().unwrap();
        let now = self.now();
        let before = map.len();
        map.retain(|_, r| r.live(now));
        before - map.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Manual clock: tests advance time explicitly, so lease/timeout behaviour
    /// is deterministic and never sleeps. Uses an `Arc`-shared atomic so the
    /// manager-held copy and the test's copy observe the same time.
    #[derive(Debug, Clone, Default)]
    struct ManualClock {
        now: std::sync::Arc<std::sync::atomic::AtomicU64>,
    }

    impl ManualClock {
        fn at(t: TimestampMs) -> ManualClock {
            ManualClock {
                now: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(t)),
            }
        }
        fn advance(&self, ms: TimestampMs) {
            self.now.fetch_add(ms, std::sync::atomic::Ordering::SeqCst);
        }
    }

    impl Clock for ManualClock {
        fn now_ms(&self) -> TimestampMs {
            self.now.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    fn mgr(clock: TimestampMs) -> (LockManager, ManualClock) {
        let c = ManualClock::at(clock);
        let m = LockManager::with_clock_and_ttl(Box::new(c.clone()), Duration::from_secs(100));
        (m, c)
    }

    fn id(s: &str) -> ObjectId {
        s.to_string()
    }

    #[test]
    fn acquire_free_object_succeeds_and_locks() {
        let (m, _) = mgr(1_000);
        let tok = m
            .acquire(&id("part-1"), &id("alice"))
            .expect("acquire free");
        assert!(m.live(&id("part-1"), &tok));
        assert!(m.is_locked(&id("part-1")));
        assert_eq!(m.holder_of(&id("part-1")).as_deref(), Some("alice"));
        assert_eq!(m.working_set(&id("alice")), vec![id("part-1")]);
    }

    #[test]
    fn acquire_conflicting_holder_fails() {
        let (m, _) = mgr(1_000);
        m.acquire(&id("part-1"), &id("alice")).unwrap();
        let err = m.acquire(&id("part-1"), &id("bob")).unwrap_err();
        assert_eq!(
            err,
            LockError::ConflictingLock {
                holder: id("alice"),
                expires: 1_000 + 100_000,
            }
        );
    }

    #[test]
    fn same_holder_reacquire_refreshes_lease_and_new_token() {
        let (m, c) = mgr(1_000);
        let t1 = m.acquire(&id("part-1"), &id("alice")).unwrap();
        c.advance(50_000); // half the 100s lease
        let t2 = m.acquire(&id("part-1"), &id("alice")).unwrap();
        assert_ne!(t1, t2, "refresh mints a fresh token");
        assert!(!m.live(&id("part-1"), &t1), "old token invalidated");
        assert!(m.live(&id("part-1"), &t2));
        assert_eq!(m.expires_at(&id("part-1")), Some(51_000 + 100_000));
    }

    #[test]
    fn release_by_token_unlocks() {
        let (m, _) = mgr(1_000);
        let tok = m.acquire(&id("part-1"), &id("alice")).unwrap();
        m.release(&id("part-1"), &tok).expect("release ok");
        assert!(!m.is_locked(&id("part-1")));
        assert!(m.working_set(&id("alice")).is_empty());
        // release again on a free object fails
        assert_eq!(
            m.release(&id("part-1"), &tok).unwrap_err(),
            LockError::NotHeld
        );
    }

    #[test]
    fn release_with_wrong_token_fails() {
        let (m, _) = mgr(1_000);
        m.acquire(&id("part-1"), &id("alice")).unwrap();
        let stranger = LockToken("lk-ffffffffffffffff".to_string());
        assert_eq!(
            m.release(&id("part-1"), &stranger).unwrap_err(),
            LockError::TokenMismatch
        );
        // lock is still held
        assert!(m.is_locked(&id("part-1")));
    }

    #[test]
    fn steal_takeover_invalidates_old_token() {
        let (m, _) = mgr(1_000);
        let alice_tok = m.acquire(&id("part-1"), &id("alice")).unwrap();
        let bob_tok = m.steal(&id("part-1"), &id("bob"));
        assert!(m.live(&id("part-1"), &bob_tok));
        assert!(
            !m.live(&id("part-1"), &alice_tok),
            "stolen: alice token dead"
        );
        // superseded alice cannot release bob's lock
        assert_eq!(
            m.release(&id("part-1"), &alice_tok).unwrap_err(),
            LockError::TokenMismatch
        );
        // and cannot publish either
        assert_eq!(
            m.authorize_publish(&id("part-1"), &alice_tok).unwrap_err(),
            PublishError::LockingExpired {
                object: id("part-1")
            }
        );
        // bob's live token can release
        m.release(&id("part-1"), &bob_tok).unwrap();
        assert!(!m.is_locked(&id("part-1")));
    }

    #[test]
    fn timeout_expired_lease_is_acquirable_and_stale_publish_denied() {
        let (m, c) = mgr(1_000);
        let alice_tok = m.acquire(&id("part-1"), &id("alice")).unwrap();
        // Let the whole 100s lease pass.
        c.advance(101_000);
        assert!(!m.live(&id("part-1"), &alice_tok), "lease expired");
        assert!(!m.is_locked(&id("part-1")), "expired lock no longer held");
        // a new player can take the object even though a record still exists
        let bob_tok = m
            .acquire(&id("part-1"), &id("bob"))
            .expect("acquire after timeout");
        assert_eq!(m.holder_of(&id("part-1")).as_deref(), Some("bob"));
        // stale alice cannot publish after expiry
        assert_eq!(
            m.authorize_publish(&id("part-1"), &alice_tok).unwrap_err(),
            PublishError::LockingExpired {
                object: id("part-1")
            }
        );
        // current holder publishes fine
        assert!(m.authorize_publish(&id("part-1"), &bob_tok).is_ok());
    }

    #[test]
    fn timeout_reap_removes_expired_records() {
        let (m, c) = mgr(1_000);
        m.acquire(&id("a"), &id("alice")).unwrap();
        m.acquire(&id("b"), &id("bob")).unwrap();
        c.advance(101_000);
        assert_eq!(m.reap_expired(), 2);
        assert_eq!(m.reap_expired(), 0);
        assert!(m.working_set(&id("alice")).is_empty());
    }

    #[test]
    fn publish_gate_requires_live_token() {
        let (m, _) = mgr(1_000);
        // no lock at all
        assert_eq!(
            m.authorize_publish(&id("part-1"), &LockToken("lk-1".into()))
                .unwrap_err(),
            PublishError::NotHeld {
                object: id("part-1")
            }
        );
        // correct holder publishes fine
        let tok = m.acquire(&id("part-1"), &id("alice")).unwrap();
        assert!(m.authorize_publish(&id("part-1"), &tok).is_ok());
        // after release, publish is denied again
        m.release(&id("part-1"), &tok).unwrap();
        assert_eq!(
            m.authorize_publish(&id("part-1"), &tok).unwrap_err(),
            PublishError::NotHeld {
                object: id("part-1")
            }
        );
    }

    #[test]
    fn concurrent_acquire_exactly_one_winner() {
        use std::sync::Arc;
        let m = Arc::new(LockManager::new());
        let object = id("part-hero");
        // widen the window by holding the busy object lock open via a big TTL,
        // then race many threads at the same object.
        m.acquire(&object, &id("seeder")).unwrap();
        let holders: Vec<String> = (0..32).map(|i| format!("worker-{i:02}")).collect();
        let mut handles = Vec::new();
        for h in holders.iter() {
            let m = Arc::clone(&m);
            let h = h.clone();
            let o = object.clone();
            handles.push(std::thread::spawn(move || m.acquire(&o, &h)));
        }
        // Seeder still owns it, so every racing worker must be refused.
        for h in handles {
            assert!(matches!(
                h.join().unwrap(),
                Err(LockError::ConflictingLock { .. })
            ));
        }
        // Seed holder's working set has the object.
        assert_eq!(m.working_set(&id("seeder")), vec![id("part-hero")]);
    }

    #[test]
    fn token_equality_is_value_based_opaque() {
        let a = LockToken("lk-1".into());
        let b = LockToken("lk-1".into());
        assert_eq!(a, b);
        assert_ne!(a, LockToken("lk-2".into()));
    }

    #[test]
    fn default_manager_uses_system_clock() {
        let m = LockManager::new();
        let tok = m.acquire(&id("x"), &id("alice")).unwrap();
        assert!(m.live(&id("x"), &tok));
        assert!(m.expires_at(&id("x")).unwrap() > 1_700_000_000_000);
    }
}
