use conduit_core::{SignId, TemporalInstant, TemporalScale};
use conduit_human::*;
use patchbay_model::inspect_current_experience_item;

fn at(ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/patchbay-homeostasis".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    }
}

fn unavailable<T>(source: &str) -> SourceObservation<T> {
    SourceObservation {
        source_identity: source.into(),
        availability: SourceAvailability::Unavailable,
        value: None,
        observation_sign_id: None,
        observed_at: None,
        freshness_limit_ticks: 10,
        uncertainty_permille: 0,
        calibration_profile_identity: None,
    }
}

fn observed<T>(source: &str, value: T, sign: &str) -> SourceObservation<T> {
    SourceObservation {
        source_identity: source.into(),
        availability: SourceAvailability::Present,
        value: Some(value),
        observation_sign_id: Some(SignId::from(sign)),
        observed_at: Some(at(20)),
        freshness_limit_ticks: 10,
        uncertainty_permille: 0,
        calibration_profile_identity: Some(format!("profile/{source}@1")),
    }
}

#[test]
fn patchbay_traces_self_state_to_exact_source_observations() {
    let state = reduce_homeostasis(
        &at(20),
        &HomeostasisPolicy::default(),
        &HomeostaticInputs {
            power: observed(
                "create/battery",
                PowerCondition {
                    reserve_permille: 200,
                    charging: false,
                },
                "sign/create-battery/20",
            ),
            thermal: unavailable("motherbrain/thermal"),
            compute_pressure: unavailable("motherbrain/compute"),
            storage_pressure: unavailable("motherbrain/storage"),
            motion_safety: observed(
                "brainstem/safety",
                SafetyCondition {
                    motion_available: false,
                    inhibited: true,
                },
                "sign/brainstem-safety/20",
            ),
            important_capability: unavailable("provider/model"),
        },
    )
    .unwrap();
    let observation = state
        .as_body_self_observation(SignId::from("sign/homeostasis/20"), at(20))
        .unwrap();
    let item = body_self_experience(
        "self/homeostasis",
        &observation,
        ExperienceTemporalRole::Current,
    )
    .unwrap();
    let mut experience = CurrentExperience::new(
        ExperienceLimits {
            maximum_items: 1,
            maximum_current_items: 1,
            maximum_recent_items: 1,
            maximum_stale_items: 1,
            maximum_historical_items: 1,
            maximum_items_per_domain: 1,
            maximum_model_derived_items: 1,
            maximum_selected_memory_items: 1,
            maximum_source_refs: 16,
            maximum_relationships: 1,
            maximum_item_bytes: MAXIMUM_BODY_SELF_STATE_BYTES,
            maximum_encoded_bytes: MAXIMUM_BODY_SELF_STATE_BYTES,
            maximum_conflict_alternatives: 2,
            maximum_identity_bytes: 128,
        },
        at(20),
        ExperienceTemporalPolicy {
            maximum_current_age_ticks: 10,
            maximum_recent_age_ticks: 20,
        },
    )
    .unwrap();
    experience.try_admit(item).unwrap();

    let trace = inspect_current_experience_item(&experience, "self/homeostasis").unwrap();
    for sign in ["sign/create-battery/20", "sign/brainstem-safety/20"] {
        assert!(trace
            .item
            .sources
            .contains(&ExperienceSourceRef::Sign(SignId::from(sign))));
    }
    assert!(trace.item.sources.contains(&ExperienceSourceRef::Source {
        source_id: "create/battery".into(),
    }));
}
