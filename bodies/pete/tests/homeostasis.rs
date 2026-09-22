use conduit_core::{SignId, TemporalInstant, TemporalScale};
use conduit_human::*;
use conduit_pete::*;

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/homeostasis".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn observed<T>(source: &str, value: T, sign: &str) -> SourceObservation<T> {
    SourceObservation {
        source_identity: source.into(),
        subject_identity: source.into(),
        availability: SourceAvailability::Present,
        value: Some(value),
        observation_sign_id: Some(SignId::from(sign)),
        observed_at: Some(at(99)),
        freshness_limit_ticks: 5,
        uncertainty_permille: 0,
        calibration_profile_identity: Some(format!("calibration/{source}@1")),
    }
}

fn unavailable<T>(source: &str, sign: &str) -> SourceObservation<T> {
    SourceObservation {
        source_identity: source.into(),
        subject_identity: source.into(),
        availability: SourceAvailability::Unavailable,
        value: None,
        observation_sign_id: Some(SignId::from(sign)),
        observed_at: Some(at(99)),
        freshness_limit_ticks: 5,
        uncertainty_permille: 0,
        calibration_profile_identity: None,
    }
}

fn inputs() -> HomeostaticInputs {
    HomeostaticInputs {
        power: observed(
            "create/battery",
            PowerCondition {
                reserve_permille: 90,
                charging: false,
            },
            "sign/power/7",
        ),
        thermal: observed(
            "motherbrain/thermal",
            ThermalCondition {
                milli_celsius: 80_000,
            },
            "sign/thermal/4",
        ),
        compute_pressure: observed(
            "motherbrain/compute",
            PressureCondition {
                pressure_permille: 850,
            },
            "sign/compute/3",
        ),
        storage_pressure: unavailable("motherbrain/storage", "sign/storage-unavailable/2"),
        motion_safety: observed(
            "brainstem/safety",
            SafetyCondition {
                motion_available: true,
                inhibited: true,
            },
            "sign/safety/8",
        ),
        important_capability: unavailable("provider/model", "sign/provider-unavailable/2"),
    }
}

#[test]
fn reviewed_policy_reduces_facts_without_fabricating_unavailable_health() {
    let state = reduce_homeostasis(&at(100), &HomeostasisPolicy::default(), &inputs()).unwrap();
    assert_eq!(state.energy, EnergyState::Critical);
    assert_eq!(state.charging, Some(false));
    assert_eq!(state.thermal, ThermalState::Constrained);
    assert_eq!(state.compute_pressure, ResourcePressureState::High);
    assert_eq!(state.storage_pressure, ResourcePressureState::Unknown);
    assert_eq!(state.motion, MotionState::Inhibited);
    assert_eq!(state.important_capability, AvailabilityState::Unknown);
    assert_eq!(state.important_capability_identity, "provider/model");
    assert_eq!(state.policy_revision, HOMEOSTASIS_POLICY_REVISION);
    assert_eq!(state.source_observations.len(), 6);
}

#[test]
fn self_observation_feeds_experience_and_retains_every_exact_source_sign() {
    let state = reduce_homeostasis(&at(100), &HomeostasisPolicy::default(), &inputs()).unwrap();
    let self_observation = state
        .as_body_self_observation(SignId::from("sign/homeostasis/1"))
        .unwrap();
    let decoded =
        conduit_core::StructuredInfoValue::from_canonical_bytes(&self_observation.canonical_state)
            .unwrap();
    assert_eq!(decoded.value_type(), &homeostatic_state_type());
    let item = body_self_experience(
        "self/homeostasis",
        &self_observation,
        ExperienceTemporalRole::Current,
    )
    .unwrap();
    assert_eq!(item.content_kind.as_str(), HOMEOSTATIC_STATE_KIND);
    assert_eq!(item.domain, ExperienceDomain::BodyState);
    for sign in [
        "sign/homeostasis/1",
        "sign/power/7",
        "sign/thermal/4",
        "sign/compute/3",
        "sign/safety/8",
        "sign/storage-unavailable/2",
        "sign/provider-unavailable/2",
    ] {
        assert!(item
            .sources
            .contains(&ExperienceSourceRef::Sign(SignId::from(sign))));
    }
}

#[test]
fn unavailable_is_witnessed_and_can_go_stale() {
    let mut values = inputs();
    values.storage_pressure.observed_at = Some(at(80));
    assert_eq!(
        reduce_homeostasis(&at(100), &HomeostasisPolicy::default(), &values),
        Err(HomeostasisRefusal::StaleObservation)
    );
}

#[test]
fn policy_identity_covers_thresholds_not_only_revision() {
    let first = HomeostasisPolicy::default();
    let mut second = first.clone();
    second.energy_low_permille += 1;
    assert_ne!(first.identity().unwrap(), second.identity().unwrap());
}

#[test]
fn stale_present_measurement_refuses_instead_of_becoming_current() {
    let mut values = inputs();
    values.power.observed_at = Some(at(80));
    assert_eq!(
        reduce_homeostasis(&at(100), &HomeostasisPolicy::default(), &values),
        Err(HomeostasisRefusal::StaleObservation)
    );
}

#[test]
fn state_is_observation_only_and_carries_no_action_or_authority() {
    let state = reduce_homeostasis(&at(100), &HomeostasisPolicy::default(), &inputs()).unwrap();
    let debug = format!("{state:?}");
    for forbidden in ["authority", "dock", "execute", "hungry", "anxious"] {
        assert!(!debug.to_lowercase().contains(forbidden));
    }
}
