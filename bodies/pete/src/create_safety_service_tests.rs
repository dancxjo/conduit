use super::*;
use crate::{IndependentWatchdogObservation, SafetyInputObservation, SafetyInputs};

fn host() -> HostId {
    HostId::from("host/pico-pete")
}

fn boot() -> BootId {
    BootId::from("boot/pico-pete-1")
}

fn binding<'a>(host: &'a HostId, boot: &'a BootId) -> CreateSafetyServiceBinding<'a> {
    CreateSafetyServiceBinding {
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(5),
        implementation_id: CREATE_SAFETY_ENVELOPE_IMPLEMENTATION,
        robot_identity: "create/physical-1",
        envelope_id: "safety/create-1",
    }
}

fn authority<'a>(host: &'a HostId, boot: &'a BootId) -> CreateSafetyClearAuthority<'a> {
    CreateSafetyClearAuthority {
        grant_id: CREATE_SAFETY_CLEAR_AUTHORITY,
        host_id: host,
        boot_id: boot,
        offer_generation: OfferGeneration(5),
        implementation_id: CREATE_SAFETY_ENVELOPE_IMPLEMENTATION,
        robot_identity: "create/physical-1",
        envelope_id: "safety/create-1",
        valid_until_tick: 200,
    }
}

fn inputs(generation: u32, tick: u64) -> SafetyInputs {
    SafetyInputs {
        generation,
        observed_at_tick: tick,
        maximum_age_ticks: 20,
        emergency_stop: SafetyInputObservation::Clear,
        wheel_drop: false,
        cliff: false,
        contact: false,
        tilt: SafetyInputObservation::Clear,
        impact: SafetyInputObservation::Clear,
        charging: false,
        control_alive: true,
        body_link_alive: true,
        independent_watchdog: IndependentWatchdogObservation::Healthy,
    }
}

fn request(hazard: LocalHazard, latch: u32, observation: u32) -> CreateSafetyClearRequest<'static> {
    CreateSafetyClearRequest {
        request_id: "service/safety-clear-1",
        hazard,
        expected_latch_generation: latch,
        expected_observation_generation: observation,
        deadline_tick: 180,
    }
}

#[test]
fn exact_authority_clears_only_inactive_matching_generation() {
    let host = host();
    let boot = boot();
    let mut envelope = LocalSafetyEnvelope::new();
    let mut contact = inputs(1, 100);
    contact.contact = true;
    envelope.observe(contact, 100).unwrap();
    envelope.observe(inputs(2, 110), 110).unwrap();

    let sign = clear_create_safety_latch(
        &mut envelope,
        binding(&host, &boot),
        request(LocalHazard::Contact, 2, 2),
        Some(authority(&host, &boot)),
        111,
    )
    .unwrap();
    assert_eq!(sign.prior_latch_generation, 2);
    assert_eq!(sign.latch_generation, 3);
    assert!(sign.remaining_hazards.is_empty());
}

#[test]
fn active_stale_and_wrong_generation_clear_attempts_are_inert() {
    let host = host();
    let boot = boot();
    let mut envelope = LocalSafetyEnvelope::new();
    let mut wheel_drop = inputs(1, 100);
    wheel_drop.wheel_drop = true;
    envelope.observe(wheel_drop, 100).unwrap();
    assert_eq!(
        clear_create_safety_latch(
            &mut envelope,
            binding(&host, &boot),
            request(LocalHazard::WheelDrop, 2, 1),
            Some(authority(&host, &boot)),
            101
        ),
        Err(CreateSafetyServiceRefusal::Envelope(
            SafetyEnvelopeRefusal::HazardStillActive
        ))
    );
    assert!(envelope
        .snapshot()
        .unwrap()
        .latched_hazards
        .contains(LocalHazard::WheelDrop));

    envelope.observe(inputs(2, 102), 102).unwrap();
    assert_eq!(
        clear_create_safety_latch(
            &mut envelope,
            binding(&host, &boot),
            request(LocalHazard::WheelDrop, 1, 2),
            Some(authority(&host, &boot)),
            103
        ),
        Err(CreateSafetyServiceRefusal::Envelope(
            SafetyEnvelopeRefusal::LatchGenerationMismatch
        ))
    );
}

#[test]
fn emergency_stop_needs_no_motion_authority_but_clear_does() {
    let host = host();
    let boot = boot();
    let mut envelope = LocalSafetyEnvelope::new();
    envelope.observe(inputs(1, 100), 100).unwrap();
    let sign = assert_create_emergency_stop(
        &mut envelope,
        binding(&host, &boot),
        CreateEmergencyStopRequest {
            request_id: "safety/estop-1",
        },
        101,
    )
    .unwrap();
    assert!(sign.latched_hazards.contains(LocalHazard::EmergencyStop));
    assert_eq!(
        clear_create_safety_latch(
            &mut envelope,
            binding(&host, &boot),
            request(LocalHazard::EmergencyStop, sign.latch_generation, 1),
            None,
            102
        ),
        Err(CreateSafetyServiceRefusal::MissingAuthority)
    );
}

#[test]
fn cross_boot_authority_cannot_clear_the_current_envelope() {
    let host = host();
    let boot = boot();
    let other_boot = BootId::from("boot/stale");
    let mut envelope = LocalSafetyEnvelope::new();
    let mut contact = inputs(1, 100);
    contact.contact = true;
    envelope.observe(contact, 100).unwrap();
    envelope.observe(inputs(2, 101), 101).unwrap();
    assert_eq!(
        clear_create_safety_latch(
            &mut envelope,
            binding(&host, &boot),
            request(LocalHazard::Contact, 2, 2),
            Some(authority(&host, &other_boot)),
            102
        ),
        Err(CreateSafetyServiceRefusal::BootMismatch)
    );
}
