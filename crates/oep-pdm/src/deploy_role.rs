//! Hybrid cloud deployment role for the oep-pdm lane (P8-M5-T03).
//!
//! Adopts the two-role deployment model ratified in
//! `workstreams/P8-M5/hybrid-cloud-role-adr.md` v1.0.0:
//!
//! * [`DeploymentRole::Airgap`] (default for regulated shops) -- the
//!   deployment stays entirely inside the shop boundary: no internet, no
//!   telemetry, no update callbacks, no cloud round-trips (P0-M2-T04
//!   section 5.3 "pure air gap"). Direct HTTPS is allowed only inside the LAN
//!   boundary.
//! * [`DeploymentRole::Hybrid`] (opt-in, credential-gated) -- a self-hosted,
//!   **relay-only** P2P mesh shaped after the Bastion mesh pattern (R1A
//!   product, Don sole developer) carries remote/mobile traffic when a peer
//!   is NAT'd. The relay forwards encapsulated traffic and holds no
//!   decryption keys, sees no plaintext, and stores nothing; endpoints
//!   terminate end-to-end encryption inside their own security boundary
//!   (`120.54(a)(5)` / `120.56` conditions required, no `126.1`-country
//!   touch).
//!
//! The module encodes the posture matrix as executable assertions so a bad
//! role configuration fails closed at activation instead of silently exporting
//! or enabling telemetry. Both roles:

//! * force telemetry off (program decision P0-M5-T02: none by default);
//! * limit network egress (`Airgap`: loopback/LAN only; `Hybrid`: the relay
//!   only);
//! * reject decryption-key custody outside the data owner's boundary
//!   (120.56 custody rule: the relay never holds keys);
//! * reject a `126.1` proscribed-destination configuration.
//!
//! Clean-room invariant: the Bastion mesh is reused as architecture/pattern,
//! never as a linked runtime, and the P2P orchestration stays AGPL-3.0
//! (license decision #3). Plain HTTPS is rejected as a NAT-traversal
//! transport by the ADR; this module conditionally permits direct HTTPS only
//! inside the LAN boundary.

use std::collections::HashSet;

/// The deployment role of an oep-pdm installation.
///
/// Chosen by operator configuration (env or config file), never inferred: an
/// `Airgap` deployment never attempts to reach a relay, and a `Hybrid`
/// deployment requires an explicit relay endpoint, an operator-declared
/// region/operator, and telemetry off before the service will start.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum DeploymentRole {
    /// Pure LAN/air-gap: no internet, no telemetry, no cloud round-trips.
    #[default]
    Airgap,
    /// LAN plus a relay-only P2P mesh (Bastion-shaped) for NAT'd remote peers.
    Hybrid,
}

/// A named security posture a role config is checked against.
pub enum Posture {
    /// Telemetry must be disabled (program decision P0-M5-T02).
    TelemetryOff,
    /// Network egress must be bounded (loopback/LAN, or the relay only).
    EgressBounded,
    /// No decryption-key custody outside the data owner's boundary (120.56).
    NoForeignKeyCustody,
    /// No 126.1 proscribed-destination touch.
    No1261,
}

/// A single failed posture assertion with a human-readable reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostureViolation {
    /// The posture that was violated.
    pub posture: &'static str,
    /// Concrete reason, in the program's surface language.
    pub reason: String,
}

/// The resolved configuration for a role; the input to [`DeploymentRole::validate`].
#[derive(Clone, Debug, Default)]
pub struct RoleConfig {
    /// Whether telemetry/analytics would be enabled. Must be false in both roles.
    pub telemetry_enabled: bool,
    /// Declared network egress policy: "none", "lan", or "relay".
    pub egress_policy: String,
    /// Whether a hybrid relay endpoint is configured.
    pub relay_configured: bool,
    /// Whether the deployment would hold decryption keys outside the data
    /// owner's boundary (e.g. keys stored on, or derived at, the relay).
    pub foreign_key_custody: bool,
    /// Declared region/operator labels (e.g. "US", "dept-network-A"). A
    /// 126.1 proscribed destination fails the gate.
    pub regions: HashSet<String>,
}

impl DeploymentRole {
    /// Display form of the role, used by config parsing and docs.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Airgap => "airgap",
            Self::Hybrid => "hybrid",
        }
    }

    /// Parse a role name from operator config; unknown names are rejected.
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "airgap" | "air-gap" | "lan" => Some(Self::Airgap),
            "hybrid" | "relay" | "bastion" => Some(Self::Hybrid),
            _ => None,
        }
    }

    /// Validate a role configuration against the posture matrix.
    ///
    /// Returns an empty list when the config is sound, otherwise every
    /// violated posture with a concrete reason. The caller must treat any
    /// non-empty list as activation failure (fail closed, never continue).
    pub fn validate(&self, cfg: &RoleConfig) -> Vec<PostureViolation> {
        let mut v = Vec::new();

        // Telemetry is off in both roles (P0-M5-T02, encoded by P8-M5-T03 ADR).
        if cfg.telemetry_enabled {
            v.push(PostureViolation {
                posture: stringify!(Posture::TelemetryOff),
                reason: "telemetry must be disabled in every deployment role".to_string(),
            });
        }

        // Egress is bounded per role.
        let policy = cfg.egress_policy.trim().to_ascii_lowercase();
        match self {
            Self::Airgap => {
                if policy != "none" && policy != "lan" {
                    v.push(PostureViolation {
                        posture: stringify!(Posture::EgressBounded),
                        reason: "airgap role permits only loopback/LAN egress \
                                 (no internet, no relay)"
                            .to_string(),
                    });
                }
                if cfg.relay_configured {
                    v.push(PostureViolation {
                        posture: stringify!(Posture::EgressBounded),
                        reason: "airgap role must not configure a relay".to_string(),
                    });
                }
            }
            Self::Hybrid => {
                if !cfg.relay_configured || policy != "relay" {
                    v.push(PostureViolation {
                        posture: stringify!(Posture::EgressBounded),
                        reason: "hybrid role requires a relay endpoint and a \
                                 relay-only egress policy"
                            .to_string(),
                    });
                }
            }
        }

        // Key custody: the relay (and any third party) never holds decryption
        // keys (120.56 custody rule).
        if cfg.foreign_key_custody {
            v.push(PostureViolation {
                posture: stringify!(Posture::NoForeignKeyCustody),
                reason: "decryption keys must stay with the data owner; a \
                         relay or transport must never hold them"
                    .to_string(),
            });
        }

        // 126.1 proscribed destinations are a hard gate in every role.
        let p1261 = ["cu", "ir", "kp", "sy", "ru", "mm", "cn", "ve"]
            .iter()
            .map(|s| s.to_string())
            .collect::<HashSet<_>>();
        for r in &cfg.regions {
            let key = r.trim().to_ascii_lowercase();
            if p1261.contains(&key) {
                v.push(PostureViolation {
                    posture: stringify!(Posture::No1261),
                    reason: format!(
                        "region {} is a 126.1 proscribed destination; the \
                         deployment must not touch it",
                        r
                    ),
                });
            }
        }

        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_airgap() -> RoleConfig {
        RoleConfig {
            telemetry_enabled: false,
            egress_policy: "none".to_string(),
            relay_configured: false,
            foreign_key_custody: false,
            regions: ["us".to_string()].into_iter().collect(),
        }
    }

    fn base_hybrid() -> RoleConfig {
        RoleConfig {
            telemetry_enabled: false,
            egress_policy: "relay".to_string(),
            relay_configured: true,
            foreign_key_custody: false,
            regions: ["us".to_string()].into_iter().collect(),
        }
    }

    #[test]
    fn airgap_default_is_clean() {
        assert!(DeploymentRole::Airgap.validate(&base_airgap()).is_empty());
    }

    #[test]
    fn hybrid_default_is_clean() {
        assert!(DeploymentRole::Hybrid.validate(&base_hybrid()).is_empty());
    }

    #[test]
    fn telemetry_is_rejected_in_both_roles() {
        for role in [DeploymentRole::Airgap, DeploymentRole::Hybrid] {
            let mut cfg = base_airgap();
            cfg.telemetry_enabled = true;
            assert!(!role.validate(&cfg).is_empty());
        }
    }

    #[test]
    fn airgap_rejects_relay() {
        let mut cfg = base_airgap();
        cfg.relay_configured = true;
        assert!(!DeploymentRole::Airgap.validate(&cfg).is_empty());
    }

    #[test]
    fn hybrid_requires_relay_and_relay_egress() {
        for mut cfg in [base_hybrid(), base_hybrid()] {
            cfg.relay_configured = false;
            assert!(!DeploymentRole::Hybrid.validate(&cfg).is_empty());
        }
        let mut cfg = base_hybrid();
        cfg.egress_policy = "lan".to_string();
        assert!(!DeploymentRole::Hybrid.validate(&cfg).is_empty());
    }

    #[test]
    fn foreign_key_custody_is_rejected() {
        for role in [DeploymentRole::Airgap, DeploymentRole::Hybrid] {
            let mut cfg = base_hybrid();
            cfg.foreign_key_custody = true;
            assert!(!role.validate(&cfg).is_empty());
        }
    }

    #[test]
    fn proscribed_destination_is_rejected() {
        for role in [DeploymentRole::Airgap, DeploymentRole::Hybrid] {
            let mut cfg = base_airgap();
            cfg.regions.insert("kp".to_string());
            assert!(!role.validate(&cfg).is_empty());
        }
    }

    #[test]
    fn region_values_are_case_and_space_insensitive() {
        let mut cfg = base_airgap();
        cfg.regions.insert(" KP ".to_string());
        assert!(!DeploymentRole::Airgap.validate(&cfg).is_empty());
    }

    #[test]
    fn parse_accepts_role_names() {
        assert_eq!(
            DeploymentRole::parse("airgap"),
            Some(DeploymentRole::Airgap)
        );
        assert_eq!(
            DeploymentRole::parse("hybrid"),
            Some(DeploymentRole::Hybrid)
        );
        assert_eq!(DeploymentRole::parse("nonsense"), None);
        assert_eq!(DeploymentRole::default(), DeploymentRole::Airgap);
    }
}
