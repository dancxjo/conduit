use super::*;
use crate::{IndependentWatchdogObservation, SafetyInputObservation, CREATE_DRIVE_IMPLEMENTATION};
use conduit_core::{BootId, HostId, OfferGeneration};

pub(super) fn reduced_safety(generation: u32, observed_at_tick: u64) -> SafetyObservation {
    SafetyObservation {
        generation,
        latch_generation: 1,
        latched_hazards: crate::SafetyHazardSet::EMPTY,
        observed_at_tick,
        maximum_age_ticks: 1_000,
        emergency_stop: SafetyInputObservation::Unavailable,
        wheel_drop: false,
        cliff: false,
        contact: false,
        tilt: SafetyInputObservation::Unavailable,
        impact: SafetyInputObservation::Unavailable,
        charging: false,
        control_alive: true,
        body_link_alive: true,
        independent_watchdog: IndependentWatchdogObservation::Absent,
    }
}

pub(super) fn observation() -> CreateDockObservation {
    CreateDockObservation {
        host_id: HostId::from("std/create-dock"),
        boot_id: BootId::from("std/create-dock-boot"),
        offer_generation: OfferGeneration(3),
        serial_base_id: "std/create-uart/0".into(),
        robot_identity: "robot/create1/0".into(),
        robot_identity_verified: true,
        dock_resource_id: "robot/create1/0/dock".into(),
        timer_resource_id: "std/timer/create-dock".into(),
        mode: OiMode::Safe,
        safety: reduced_safety(7, 100),
    }
}

#[test]
fn canonical_dock_form_is_mechanism_free_and_finitely_timed() {
    for forbidden in ["create", "uart", "serial", "gpio", "netherwick", "opcode"] {
        assert!(!CREATE_DOCK_FORM.to_ascii_lowercase().contains(forbidden));
    }
    assert!(CREATE_DOCK_FORM.contains("timeout-ms = 30000"));
    let (_, profile) = crate::catalogs().unwrap();
    conduit_form::parse(CREATE_DOCK_FORM, &profile).unwrap();
}

#[test]
fn dock_offer_requires_verified_fresh_safe_truth_and_is_honestly_reduced() {
    let value = observation();
    let host = live_create_dock_advertisement(&value, 100).unwrap();
    assert_eq!(host.profile.as_str(), CREATE_DOCK_REDUCED_SAFETY_PROFILE);
    let dock = host
        .capabilities
        .iter()
        .find(|offer| offer.capability_id.as_str() == CREATE_DOCK_CAPABILITY)
        .unwrap();
    assert_eq!(
        dock.authority_requirements[0].contract_id.as_str(),
        CREATE_DOCK_REDUCED_SAFETY_AUTHORITY
    );

    let mut unverified = value.clone();
    unverified.robot_identity_verified = false;
    assert_eq!(
        live_create_dock_advertisement(&unverified, 100),
        Err(CreateDockOfferRefusal::UnverifiedIdentity)
    );
    let mut hazardous = value;
    hazardous.safety.wheel_drop = true;
    assert_eq!(
        live_create_dock_advertisement(&hazardous, 100),
        Err(CreateDockOfferRefusal::SafetyStaleOrInhibited(
            LocalHazard::WheelDrop
        ))
    );
}

#[test]
fn dock_offer_requires_the_non_bypassable_latch_envelope() {
    let mut value = observation();
    value.safety.latch_generation = 0;
    assert_eq!(
        live_create_dock_advertisement(&value, 100),
        Err(CreateDockOfferRefusal::MissingSafetyEnvelope)
    );
}

#[test]
fn exact_dock_authority_is_required_and_plan_seals_no_drive_realization() {
    assert!(matches!(
        create_dock_plan(&observation(), 100, false),
        Err(PlannerError::AuthorityGrantMissing(_))
    ));
    let plan = create_dock_plan(&observation(), 100, true).unwrap();
    let encoded = serde_json::to_string(&plan).unwrap();
    assert!(encoded.contains(CREATE_DOCK_IMPLEMENTATION));
    assert!(encoded.contains(CREATE_DOCK_REDUCED_SAFETY_AUTHORITY));
    assert!(!encoded.contains(CREATE_DRIVE_IMPLEMENTATION));
    let dock = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.implementation_id.as_str() == CREATE_DOCK_IMPLEMENTATION)
        .unwrap();
    assert_eq!(dock.resources.len(), 4);
    assert_eq!(dock.authority.len(), 1);
}
