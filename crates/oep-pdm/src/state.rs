//! P8-M2-T01 -- Revision lifecycle state machine.
//!
//! Models the lifecycle of a PDM *revision* over the four states adopted
//! verbatim from the wire contract (`workstreams/pdm/api/openapi-v1.yaml`, v1). See
//! `workstreams/pdm/revisions/revision-state-machine.md` for the full spec.
//!
//! ```text
//!   (new) -> working -> under_review -> released -> obsolete   (terminal)
//!               ^            |
//!               |____________|   (return for rework)
//! ```
//!
//! ## Legal hops (the complete table, also asserted by tests)
//!
//! ```text
//!   working      -> under_review    submit for review
//!   working      -> released        direct release (fast path, requires final maturity)
//!   working      -> obsolete        abandoned draft
//!   under_review -> working         returned for rework
//!   under_review -> released        approved (requires final maturity)
//!   under_review -> obsolete        rejected / withdrawn
//!   released     -> obsolete        superseded later in service
//! ```
//!
//! Everything else is illegal and returns a typed [`TransitionError`]. A
//! released revision cannot leave `released` except to `obsolete` (this is
//! the released-state immutability enforcement); `obsolete` is terminal.
//!
//! ## The maturity axis and the maturity transient
//!
//! A revision carries a second, independent axis, [`Maturity`]
//! (`preliminary < detailed < final`). Maturity is a *transient* property for
//! the whole time the revision is not `released`: while `working` or
//! `under_review` it may be set, advanced, or rolled back freely, because
//! nothing is frozen until release. Release is the end of the maturity
//! transient: entering `released` requires maturity already at `Final`
//! ([`TransitionError::MaturityNotFinal`] otherwise) and then maturity is
//! frozen at `Final` permanently.
//!
//! Every transition is atomic and returns a [`TransitionOutcome`] (from / to
//! / maturity before and after); the transient intermediate value is never
//! observable at rest.

use core::fmt;

/// The four lifecycle states of a PDM revision.
///
/// Adopted verbatim from the wire contract. This enum is `Copy` so it can be
/// moved in and out of the store and machine without allocation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RevisionState {
    /// Active engineering. The revision is still edited freely.
    Working,
    /// Submitted for peer/formal review.
    UnderReview,
    /// Approved and frozen. Immutable except for the terminal `obsolete` hop.
    Released,
    /// Superseded or withdrawn. Terminal: no further legal hops.
    Obsolete,
}

impl RevisionState {
    /// The wire identifier used over the REST API and in persisted files.
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::UnderReview => "under_review",
            Self::Released => "released",
            Self::Obsolete => "obsolete",
        }
    }

    /// Parse a wire identifier back into a state; `None` for unknown strings.
    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match s {
            "working" => Self::Working,
            "under_review" => Self::UnderReview,
            "released" => Self::Released,
            "obsolete" => Self::Obsolete,
            _ => return None,
        })
    }

    /// All four states, in wire order. Used by the exhaustive-hop tests.
    pub const ALL: [Self; 4] = [
        Self::Working,
        Self::UnderReview,
        Self::Released,
        Self::Obsolete,
    ];

    /// True for the terminal state [`Obsolete`](Self::Obsolete).
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Obsolete)
    }

    /// True for [`Released`](Self::Released).
    pub const fn is_released(self) -> bool {
        matches!(self, Self::Released)
    }
}

impl fmt::Display for RevisionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// Maturity of a revision, independent of its lifecycle state.
///
/// `Preliminary < Detailed < Final` by declaration order. See the crate docs
/// for the "maturity transient" rules: freely mutable while not released,
/// frozen at `Final` from the moment a revision is released.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Maturity {
    /// Concept / first-pass engineering output.
    Preliminary,
    /// Substantially developed, review-ready engineering.
    Detailed,
    /// Complete and validated; required to release.
    Final,
}

impl Maturity {
    /// The wire identifier used over the REST API and in persisted files.
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Preliminary => "preliminary",
            Self::Detailed => "detailed",
            Self::Final => "final",
        }
    }

    /// Parse a wire identifier back into a maturity; `None` for unknown.
    pub fn from_wire(s: &str) -> Option<Self> {
        Some(match s {
            "preliminary" => Self::Preliminary,
            "detailed" => Self::Detailed,
            "final" => Self::Final,
            _ => return None,
        })
    }

    /// The rank of this maturity on the 0..=2 ladder.
    pub const fn rank(self) -> u8 {
        match self {
            Self::Preliminary => 0,
            Self::Detailed => 1,
            Self::Final => 2,
        }
    }

    /// The next rung up the ladder, or `None` when already [`Final`](Self::Final).
    pub const fn advance(self) -> Option<Self> {
        match self {
            Self::Preliminary => Some(Self::Detailed),
            Self::Detailed => Some(Self::Final),
            Self::Final => None,
        }
    }

    /// The previous rung down the ladder, or `None` at [`Preliminary`](Self::Preliminary).
    pub const fn retreat(self) -> Option<Self> {
        match self {
            Self::Detailed => Some(Self::Preliminary),
            Self::Final => Some(Self::Detailed),
            Self::Preliminary => None,
        }
    }
}

impl fmt::Display for Maturity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.wire())
    }
}

/// The exhaustive legal-hop table, in declaration order.
///
/// This is the single authority the machine consults. Tests assert it is
/// exactly the expected set (no missing, no extras) and that every other one
/// of the 16 ordered pairs is rejected.
pub const LEGAL_TRANSITIONS: [(RevisionState, RevisionState); 7] = [
    (RevisionState::Working, RevisionState::UnderReview),
    (RevisionState::Working, RevisionState::Released),
    (RevisionState::Working, RevisionState::Obsolete),
    (RevisionState::UnderReview, RevisionState::Working),
    (RevisionState::UnderReview, RevisionState::Released),
    (RevisionState::UnderReview, RevisionState::Obsolete),
    (RevisionState::Released, RevisionState::Obsolete),
];

/// Why a transition or mutation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionError {
    /// The ordered (from, to) pair is not in [`LEGAL_TRANSITIONS`] and the
    /// source is not terminal. Covers self-hops and released -> working /
    /// released -> under_review (the immutability hop, kept distinct below).
    IllegalTransition {
        /// The state the revision is coming from.
        from: RevisionState,
        /// The state the revision was asked to move to.
        to: RevisionState,
    },
    /// The source state is [`Obsolete`](RevisionState::Obsolete): terminal,
    /// no further hops of any kind are permitted.
    Terminal(RevisionState),
    /// A hop into [`Released`](RevisionState::Released) is only legal once the
    /// maturity transient has completed, i.e. maturity is at
    /// [`Final`](Maturity::Final).
    MaturityNotFinal {
        /// The source state of the attempted release.
        from: RevisionState,
        /// Always [`Released`](RevisionState::Released).
        to: RevisionState,
        /// The maturity observed at the time of the attempted hop.
        current: Option<Maturity>,
    },
    /// A mutation of a frozen revision: a released revision is immutable
    /// except for the terminal `obsolete` hop, and an obsolete revision is
    /// immutable, period.
    Immutable {
        /// What was being changed (`"state"`, `"maturity"`).
        op: &'static str,
        /// The frozen state that refused the mutation.
        state: RevisionState,
    },
}

impl fmt::Display for TransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IllegalTransition { from, to } => {
                write!(f, "illegal transition: {from} -> {to}")
            }
            Self::Terminal(s) => write!(f, "state {s} is terminal; no further transitions"),
            Self::MaturityNotFinal { from, to, current } => match current {
                Some(c) => write!(
                    f,
                    "cannot {from} -> {to}: maturity must be final, found {c}"
                ),
                None => write!(f, "cannot {from} -> {to}: maturity is not assigned"),
            },
            Self::Immutable { op, state } => {
                write!(f, "cannot mutate {op} of {state}: revision is frozen")
            }
        }
    }
}

impl std::error::Error for TransitionError {}

/// Decide whether the ordered hop `from -> to` is one of the legal ones,
/// returning the typed error that explains the refusal.
pub fn check_hop(from: RevisionState, to: RevisionState) -> Result<(), TransitionError> {
    // Terminal wins over everything: an obsolete revision accepts zero
    // transitions, not even a self-hop.
    if from.is_terminal() {
        return Err(TransitionError::Terminal(from));
    }
    if from == to {
        return Err(TransitionError::IllegalTransition { from, to });
    }
    if from == RevisionState::Released && to != RevisionState::Obsolete {
        return Err(TransitionError::Immutable {
            op: "state",
            state: from,
        });
    }
    if !LEGAL_TRANSITIONS.contains(&(from, to)) {
        return Err(TransitionError::IllegalTransition { from, to });
    }
    Ok(())
}

/// What one atomic transition changed: the from/to states and the maturity
/// before and after. The transient maturity value in between is never exposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionOutcome {
    /// The state the revision left.
    pub from: RevisionState,
    /// The state the revision entered.
    pub to: RevisionState,
    /// Maturity observed at the start of the hop.
    pub maturity_before: Option<Maturity>,
    /// Maturity after the hop (freezes to [`Some`] [`Maturity::Final`] on release).
    pub maturity_after: Option<Maturity>,
}

/// The (state, maturity) pair of a single revision, with the machine logic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lifecycle {
    state: RevisionState,
    maturity: Option<Maturity>,
}

impl Lifecycle {
    /// A fresh revision: `working` with no maturity assigned yet.
    pub fn new() -> Self {
        Self {
            state: RevisionState::Working,
            maturity: None,
        }
    }

    /// Build a lifecycle from a persisted pair. Intended for store reloads;
    /// values are trusted as already validated (see [`Self::new`]).
    pub fn from_parts(state: RevisionState, maturity: Option<Maturity>) -> Self {
        Self { state, maturity }
    }

    /// The current lifecycle state.
    pub fn state(&self) -> RevisionState {
        self.state
    }

    /// The current maturity, or `None` when no maturity has been assigned.
    pub fn maturity(&self) -> Option<Maturity> {
        self.maturity
    }

    /// True when the revision has reached [`Released`](RevisionState::Released).
    pub fn is_released(&self) -> bool {
        self.state.is_released()
    }

    /// True when the revision is terminal ([`Obsolete`](RevisionState::Obsolete)).
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }

    /// Convenience: is `from -> to` one of the 7 legal hops?
    ///
    /// Note this only checks the hop table, not the maturity gate of the
    /// current lifecycle; a transition into `released` still needs
    /// [`Self::transition_to`] to succeed with maturity at `Final`.
    pub fn is_legal(ctx_from: RevisionState, to: RevisionState) -> bool {
        check_hop(ctx_from, to).is_ok()
    }

    /// Every hop in the legal table, in declaration order (7 entries).
    pub fn legal_hops() -> Vec<(RevisionState, RevisionState)> {
        LEGAL_TRANSITIONS.to_vec()
    }

    /// Apply one atomic transition. Validates the hop, enforces the maturity
    /// gate for release, freezes maturity on release, and returns an outcome
    /// recording exactly what changed.
    pub fn transition_to(
        &mut self,
        to: RevisionState,
    ) -> Result<TransitionOutcome, TransitionError> {
        let from = self.state;
        check_hop(from, to)?;
        // Maturity gate: a release ends the maturity transient, so it may only
        // happen once maturity has already reached Final.
        if to == RevisionState::Released {
            match self.maturity {
                Some(Maturity::Final) => {}
                m => {
                    return Err(TransitionError::MaturityNotFinal {
                        from,
                        to,
                        current: m,
                    })
                }
            }
        }
        let maturity_before = self.maturity;
        self.state = to;
        if to == RevisionState::Released {
            self.maturity = Some(Maturity::Final);
        }
        let maturity_after = self.maturity;
        Ok(TransitionOutcome {
            from,
            to,
            maturity_before,
            maturity_after,
        })
    }

    /// Set the revision's maturity. Only legal while `working` or
    /// `under_review` (the transient, pre-release phase); a released or
    /// obsolete revision is immutable.
    ///
    /// Maturity may move in either direction here: before release nothing is
    /// frozen, so a rollback is permitted.
    pub fn set_maturity(&mut self, m: Maturity) -> Result<(), TransitionError> {
        match self.state {
            RevisionState::Working | RevisionState::UnderReview => {
                self.maturity = Some(m);
                Ok(())
            }
            s => Err(TransitionError::Immutable {
                op: "maturity",
                state: s,
            }),
        }
    }

    /// Advance maturity one rung up the ladder.
    ///
    /// A revision with no assigned maturity starts at
    /// [`Preliminary`](Maturity::Preliminary). Advancing already-`Final`
    /// maturity is a no-op that keeps `Final`. Refused on released/obsolete.
    pub fn advance_maturity(&mut self) -> Result<(), TransitionError> {
        match self.state {
            RevisionState::Working | RevisionState::UnderReview => {
                let next = match self.maturity {
                    Some(m) => m.advance().unwrap_or(m),
                    None => Maturity::Preliminary,
                };
                self.maturity = Some(next);
                Ok(())
            }
            s => Err(TransitionError::Immutable {
                op: "maturity",
                state: s,
            }),
        }
    }
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TransitionError as E;

    fn states() -> impl Iterator<Item = RevisionState> {
        RevisionState::ALL.into_iter()
    }

    fn pair_combinations() -> Vec<(RevisionState, RevisionState)> {
        let mut out = Vec::new();
        for a in states() {
            for b in states() {
                out.push((a, b));
            }
        }
        out
    }

    /// The exact expected legal set, mirroring the spec.
    fn expected_legal() -> Vec<(RevisionState, RevisionState)> {
        let w = RevisionState::Working;
        let u = RevisionState::UnderReview;
        let r = RevisionState::Released;
        let o = RevisionState::Obsolete;
        vec![(w, u), (w, r), (w, o), (u, w), (u, r), (u, o), (r, o)]
    }

    #[test]
    fn legal_hops_table_is_exact() {
        let actual: std::collections::BTreeSet<_> = LEGAL_TRANSITIONS.iter().copied().collect();
        let expected: std::collections::BTreeSet<_> = expected_legal().into_iter().collect();
        assert_eq!(actual, expected, "legal table must be exactly the spec set");
        assert_eq!(LEGAL_TRANSITIONS.len(), 7);
    }

    #[test]
    fn all_hops_are_enumerated_and_classified() {
        let all = pair_combinations();
        assert_eq!(all.len(), 16, "4 states x 4 states = 16 ordered pairs");
        let legal: std::collections::BTreeSet<_> = expected_legal().into_iter().collect();
        let mut legal_hits = 0;
        for (from, to) in &all {
            if legal.contains(&(*from, *to)) {
                assert!(
                    Lifecycle::is_legal(*from, *to),
                    "{from} -> {to} must be legal"
                );
                legal_hits += 1;
            }
        }
        assert_eq!(legal_hits, 7);
    }

    #[test]
    fn every_illegal_hop_is_rejected() {
        let legal: std::collections::BTreeSet<_> = expected_legal().into_iter().collect();
        for (from, to) in pair_combinations() {
            if legal.contains(&(from, to)) {
                continue;
            }
            let mut lc = Lifecycle::from_parts(from, None);
            let err = lc.transition_to(to).unwrap_err();
            // Every illegal hop is refused and leaves the state untouched.
            assert_eq!(lc.state(), from, "failed hop must not move the state");
            match from {
                RevisionState::Obsolete => assert_eq!(err, E::Terminal(from)),
                RevisionState::Released => assert!(
                    matches!(err, E::IllegalTransition { .. } | E::Immutable { .. }),
                    "released source reflexes as illegal or immutable, got {err:?}"
                ),
                _ => assert!(matches!(err, E::IllegalTransition { .. })),
            }
        }
    }

    #[test]
    fn self_hops_are_illegal_for_every_state() {
        for s in states() {
            let mut lc = Lifecycle::from_parts(s, None);
            let err = lc.transition_to(s).unwrap_err();
            if s == RevisionState::Obsolete {
                assert_eq!(err, E::Terminal(RevisionState::Obsolete));
            } else {
                assert_eq!(err, E::IllegalTransition { from: s, to: s });
            }
            assert_eq!(lc.state(), s);
        }
    }

    #[test]
    fn obsolete_is_terminal() {
        let mut lc = Lifecycle::new();
        lc.transition_to(RevisionState::Obsolete).unwrap();
        assert!(lc.is_terminal());
        for to in states() {
            let err = lc.transition_to(to).unwrap_err();
            assert_eq!(err, E::Terminal(RevisionState::Obsolete));
        }
    }

    #[test]
    fn release_requires_final_maturity() {
        for src in [RevisionState::Working, RevisionState::UnderReview] {
            for m in [None, Some(Maturity::Preliminary), Some(Maturity::Detailed)] {
                let mut lc = Lifecycle::from_parts(src, m);
                let err = lc.transition_to(RevisionState::Released).unwrap_err();
                assert_eq!(
                    err,
                    E::MaturityNotFinal {
                        from: src,
                        to: RevisionState::Released,
                        current: m,
                    }
                );
                assert_eq!(lc.state(), src, "gate refusal must not move the state");
                assert_eq!(lc.maturity(), m);
            }
        }
    }

    #[test]
    fn release_with_final_maturity_succeeds_and_freezes() {
        for src in [RevisionState::Working, RevisionState::UnderReview] {
            let mut lc = Lifecycle::from_parts(src, Some(Maturity::Final));
            let out = lc.transition_to(RevisionState::Released).unwrap();
            assert_eq!(out.from, src);
            assert_eq!(out.to, RevisionState::Released);
            assert_eq!(out.maturity_before, Some(Maturity::Final));
            assert_eq!(out.maturity_after, Some(Maturity::Final));
            assert!(lc.is_released());
            assert_eq!(lc.maturity(), Some(Maturity::Final));
            // Released immutability: no state change except obsolete.
            let err = lc.transition_to(RevisionState::Working).unwrap_err();
            assert_eq!(
                err,
                E::Immutable {
                    op: "state",
                    state: RevisionState::Released
                }
            );
            let err = lc.transition_to(RevisionState::UnderReview).unwrap_err();
            assert_eq!(
                err,
                E::Immutable {
                    op: "state",
                    state: RevisionState::Released
                }
            );
            // Maturity is frozen too.
            assert_eq!(
                lc.set_maturity(Maturity::Preliminary).unwrap_err(),
                E::Immutable {
                    op: "maturity",
                    state: RevisionState::Released
                }
            );
            assert_eq!(
                lc.advance_maturity().unwrap_err(),
                E::Immutable {
                    op: "maturity",
                    state: RevisionState::Released
                }
            );
            // ... but released -> obsolete remains the one legal hop.
            let out = lc.transition_to(RevisionState::Obsolete).unwrap();
            assert_eq!(out.from, RevisionState::Released);
            assert_eq!(out.to, RevisionState::Obsolete);
            assert!(lc.is_terminal());
        }
    }

    #[test]
    fn maturity_is_transient_until_release() {
        let mut lc = Lifecycle::new();
        // Not assigned yet.
        assert_eq!(lc.maturity(), None);
        // Advance through the ladder, transient (revocable) while working.
        lc.advance_maturity().unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Preliminary));
        lc.advance_maturity().unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Detailed));
        lc.advance_maturity().unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Final));
        lc.advance_maturity().unwrap(); // no-op above the top
        assert_eq!(lc.maturity(), Some(Maturity::Final));
        // Set directly and roll back: pre-release maturity is transient.
        lc.set_maturity(Maturity::Detailed).unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Detailed));
        lc.set_maturity(Maturity::Preliminary).unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Preliminary));
        // Still transient in under_review.
        lc.transition_to(RevisionState::UnderReview).unwrap();
        lc.set_maturity(Maturity::Detailed).unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Detailed));
        lc.advance_maturity().unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Final));
        // Release and the transient ends.
        lc.transition_to(RevisionState::Released).unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Final));
        assert_eq!(
            lc.set_maturity(Maturity::Preliminary).unwrap_err(),
            E::Immutable {
                op: "maturity",
                state: RevisionState::Released
            }
        );
    }

    #[test]
    fn full_lifecycle_records_outcomes() {
        let mut lc = Lifecycle::new();
        lc.set_maturity(Maturity::Detailed).unwrap();
        // returned for rework keeps maturity.
        let out = lc.transition_to(RevisionState::UnderReview).unwrap();
        assert_eq!(
            out,
            TransitionOutcome {
                from: RevisionState::Working,
                to: RevisionState::UnderReview,
                maturity_before: Some(Maturity::Detailed),
                maturity_after: Some(Maturity::Detailed),
            }
        );
        let out = lc.transition_to(RevisionState::Working).unwrap();
        assert_eq!(out.to, RevisionState::Working);
        lc.advance_maturity().unwrap();
        assert_eq!(lc.maturity(), Some(Maturity::Final));
        lc.transition_to(RevisionState::UnderReview).unwrap();
        lc.transition_to(RevisionState::Released).unwrap();
        lc.transition_to(RevisionState::Obsolete).unwrap();
        assert!(lc.is_terminal());
    }

    #[test]
    fn wire_names_match_contract_snake_case() {
        assert_eq!(RevisionState::UnderReview.wire(), "under_review");
        assert_eq!(Maturity::Final.wire(), "final");
        assert_eq!(
            RevisionState::from_wire("working"),
            Some(RevisionState::Working)
        );
        assert_eq!(RevisionState::from_wire("bogus"), None);
        assert_eq!(
            Maturity::from_wire("preliminary"),
            Some(Maturity::Preliminary)
        );
        assert_eq!(Maturity::from_wire(""), None);
    }

    #[test]
    fn serde_round_trip_uses_wire_names() {
        let s = serde_json::to_string(&RevisionState::UnderReview).unwrap();
        assert_eq!(s, "\"under_review\"");
        let back: RevisionState = serde_json::from_str(&s).unwrap();
        assert_eq!(back, RevisionState::UnderReview);
        let m = serde_json::to_string(&Maturity::Final).unwrap();
        assert_eq!(m, "\"final\"");
    }
}
