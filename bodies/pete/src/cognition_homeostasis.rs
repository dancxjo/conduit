//! Translate fresh frame observations through Pete's reviewed homeostasis policy.
use conduit_human::{
    ExperienceAvailability, ExperienceCertainty, ExperienceDomain, ExperienceItem,
    ExperienceOrigin, ExperienceSourceRef, ExperienceTemporalPolicy, SourceAvailability,
    SourceObservation,
};

use crate::{
    reduce_homeostasis, HomeostasisPolicy, HomeostaticInputs, Observed, PeteBodyState,
    PeteCognitionRefusal, PeteSituation, PowerCondition, SafetyCondition, SituationFactState,
    HOMEOSTATIC_STATE_KIND,
};

pub(super) fn homeostatic_evidence(
    situation: &PeteSituation,
    battery: &Observed<conduit_robotics::BatteryObservation>,
    charging: bool,
    temporal_policy: ExperienceTemporalPolicy,
    body: Option<&Observed<PeteBodyState>>,
) -> Result<ExperienceItem, PeteCognitionRefusal> {
    let bound = situation.maximum_age_ticks;
    let power = (situation.battery.state == SituationFactState::Current).then_some(battery);
    let safety = body.filter(|_| situation.body.state == SituationFactState::Current);
    let state = reduce_homeostasis(
        &situation.reference,
        &HomeostasisPolicy::default(),
        &HomeostaticInputs {
            power: source("battery", bound, power, |value| PowerCondition {
                reserve_permille: value.charge_permille(),
                charging,
            }),
            thermal: missing("thermal", bound),
            compute_pressure: missing("compute-pressure", bound),
            storage_pressure: missing("storage-pressure", bound),
            motion_safety: source("body-safety", bound, safety, |value| SafetyCondition {
                motion_available: value.motion_available,
                inhibited: value.safety_inhibited,
            }),
            important_capability: missing("important-capability", bound),
        },
    )
    .map_err(|_| PeteCognitionRefusal::Homeostasis)?;
    let charge_percent = power.map(|item| {
        let permille = item.value.charge_permille();
        format!("{}.{}%", permille / 10, permille % 10)
    });
    let content = serde_json::to_vec(&serde_json::json!({
        "policy_identity": state.policy_identity,
        "energy": state.energy,
        "charging": state.charging,
        "charge_percent": charge_percent,
        "motion": state.motion,
        "thermal": state.thermal,
        "compute_pressure": state.compute_pressure,
        "storage_pressure": state.storage_pressure,
        "important_capability": state.important_capability,
    }))
    .map_err(|_| PeteCognitionRefusal::Evidence)?;
    let mut sources = Vec::with_capacity(2);
    for sign in [
        power.map(|item| &item.sign_id),
        safety.map(|item| &item.sign_id),
    ]
    .into_iter()
    .flatten()
    {
        let source = ExperienceSourceRef::Sign(sign.clone());
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    Ok(ExperienceItem {
        id: "pete/homeostasis".into(),
        domain: ExperienceDomain::BodyState,
        content_kind: HOMEOSTATIC_STATE_KIND.into(),
        encoded_content: content,
        origin: ExperienceOrigin::DeterministicDerived,
        temporal_role: temporal_policy
            .classify(&state.reduced_at, &situation.reference)
            .map_err(|_| PeteCognitionRefusal::Homeostasis)?,
        availability: ExperienceAvailability::Present,
        certainty: ExperienceCertainty::Uncertain,
        observed_at: Some(state.reduced_at),
        recorded_at: None,
        sources,
    })
}

fn source<T, U>(
    identity: &str,
    freshness_limit_ticks: u64,
    observation: Option<&Observed<T>>,
    map: impl FnOnce(&T) -> U,
) -> SourceObservation<U> {
    match observation {
        Some(observation) => SourceObservation {
            availability: SourceAvailability::Present,
            value: Some(map(&observation.value)),
            observation_sign_id: Some(observation.sign_id.clone()),
            observed_at: Some(observation.observed_at.clone()),
            ..missing(identity, freshness_limit_ticks)
        },
        None => missing(identity, freshness_limit_ticks),
    }
}

fn missing<T>(identity: &str, freshness_limit_ticks: u64) -> SourceObservation<T> {
    SourceObservation {
        source_identity: format!("pete/{identity}"),
        subject_identity: format!("pete/{identity}"),
        availability: SourceAvailability::Missing,
        value: None,
        observation_sign_id: None,
        observed_at: None,
        freshness_limit_ticks,
        uncertainty_permille: 1000,
        calibration_profile_identity: None,
    }
}
