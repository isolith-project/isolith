//! P8-M5-T03 acceptance (Rust side): the hybrid cloud role model.
//!
//! Mirrors the Python acceptance test `tests/test_p8m5t03_hybrid_cloud_role.py`
//! and the ADR `P8-M5/hybrid-cloud-role-adr.md` v1.0.0 on real material: the
//! `DeploymentRole` type, the two-role deployment model, and the posture matrix
//! (telemetry off, egress bounded, no foreign key custody, no 126.1 touch).

use oep_pdm::deploy_role::{DeploymentRole, RoleConfig};
use std::collections::HashSet;

fn airgap_cfg() -> RoleConfig {
    RoleConfig {
        telemetry_enabled: false,
        egress_policy: "none".to_string(),
        relay_configured: false,
        foreign_key_custody: false,
        regions: HashSet::from(["us".to_string()]),
    }
}

fn hybrid_cfg() -> RoleConfig {
    RoleConfig {
        telemetry_enabled: false,
        egress_policy: "relay".to_string(),
        relay_configured: true,
        foreign_key_custody: false,
        regions: HashSet::from(["us".to_string()]),
    }
}

#[test]
fn airgap_is_the_default_role() {
    assert_eq!(DeploymentRole::default(), DeploymentRole::Airgap);
    assert_eq!(DeploymentRole::Airgap.as_str(), "airgap");
}

#[test]
fn airgap_install_path_is_clean_by_default() {
    // A LAN/air-gap deployment is the default and validates with no setup.
    assert_eq!(DeploymentRole::Airgap.validate(&airgap_cfg()), vec![]);
}

#[test]
fn hybrid_provides_relay_only_nat_access() {
    // A hybrid deployment is opt-in (explicit relay + relay-only egress) and
    // still keeps telemetry off and keys with the data owner.
    assert_eq!(DeploymentRole::Hybrid.validate(&hybrid_cfg()), vec![]);
}

#[test]
fn hybrid_rejects_missing_relay_or_internet_egress() {
    let mut no_relay = hybrid_cfg();
    no_relay.relay_configured = false;
    assert!(!DeploymentRole::Hybrid.validate(&no_relay).is_empty());

    let mut lan_egress = hybrid_cfg();
    lan_egress.egress_policy = "lan".to_string();
    assert!(!DeploymentRole::Hybrid.validate(&lan_egress).is_empty());
}

#[test]
fn telemetry_off_is_enforced_in_both_roles() {
    for role in [DeploymentRole::Airgap, DeploymentRole::Hybrid] {
        let mut cfg = airgap_cfg();
        cfg.telemetry_enabled = true;
        assert!(!role.validate(&cfg).is_empty());
    }
}

#[test]
fn key_custody_stays_with_the_data_owner() {
    for role in [DeploymentRole::Airgap, DeploymentRole::Hybrid] {
        let mut cfg = hybrid_cfg();
        cfg.foreign_key_custody = true;
        assert!(!role.validate(&cfg).is_empty());
    }
}

#[test]
fn proscribed_1261_regions_fail_closed() {
    for role in [DeploymentRole::Airgap, DeploymentRole::Hybrid] {
        let mut cfg = airgap_cfg();
        cfg.regions.insert("kp".to_string());
        assert!(!role.validate(&cfg).is_empty());
    }
}
