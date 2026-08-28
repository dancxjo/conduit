use crate::fact_policy::{predicate_matches, CandidateFacts};
use crate::prelude::*;
use crate::{
    HardRealizationRequirements, PlannerError, RealizationPolicy, RealizationRejection,
    MAXIMUM_PLANNER_POLICY_CLAUSES,
};
use conduit_core::{
    CapabilityOffer, HostAdvertisement, RealizationAdvertisement, ResourceObservation,
};

pub(crate) fn predicate_rejection(
    host: &HostAdvertisement,
    offer: &CapabilityOffer,
    realization: Option<&RealizationAdvertisement>,
    observations: &[ResourceObservation],
    predicates: &[crate::PlannerPredicate],
) -> Result<Option<RealizationRejection>, PlannerError> {
    let candidate = CandidateFacts {
        host,
        offer,
        realization,
        observations,
    };
    for (index, predicate) in predicates.iter().enumerate() {
        let matches = predicate_matches(&candidate, predicate).map_err(|error| {
            PlannerError::InvalidHardRealizationRequirement(format!(
                "invalid generic predicate at clause {index}: {error:?}"
            ))
        })?;
        if !matches {
            return Ok(Some(RealizationRejection::HardPredicate {
                clause_index: u16::try_from(index).map_err(|_| {
                    PlannerError::PlannerLimitExceeded(
                        "hard predicate clause index exceeds u16 evidence".into(),
                    )
                })?,
                fact: predicate.fact().clone(),
            }));
        }
    }
    Ok(None)
}

pub(crate) fn validate_inputs(
    requirements: &HardRealizationRequirements,
    policy: &RealizationPolicy,
    advertisements: &[RealizationAdvertisement],
) -> Result<(), PlannerError> {
    if requirements.predicates.len() > MAXIMUM_PLANNER_POLICY_CLAUSES
        || policy.preferences.len() > MAXIMUM_PLANNER_POLICY_CLAUSES
    {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "hard or soft policy exceeds the {} clause bound",
            MAXIMUM_PLANNER_POLICY_CLAUSES
        )));
    }
    for reference in requirements
        .predicates
        .iter()
        .map(crate::PlannerPredicate::fact)
    {
        if !known_characteristic_reference(reference, advertisements) {
            return Err(PlannerError::InvalidHardRealizationRequirement(format!(
                "generic policy references unknown characteristic '{}'",
                characteristic_id(reference).expect("unknown reference is characteristic")
            )));
        }
    }
    for preference in &policy.preferences {
        let lowered = preference.lower();
        if !known_characteristic_reference(lowered.fact(), advertisements) {
            return Err(PlannerError::InvalidRealizationPolicy(format!(
                "generic policy references unknown characteristic '{}'",
                characteristic_id(lowered.fact()).expect("unknown reference is characteristic")
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_preferences(
    candidates: &[(&HostAdvertisement, &CapabilityOffer)],
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<(), PlannerError> {
    let candidates = candidates
        .iter()
        .map(|(host, offer)| {
            (
                *host,
                *offer,
                advertisement_for(host, offer, advertisements),
            )
        })
        .collect::<Vec<_>>();
    crate::characteristic_policy::validate_preferences(&candidates, observations, policy)
}

pub(crate) fn decisive_clause(
    candidates: &[(&HostAdvertisement, &CapabilityOffer)],
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Option<u16> {
    let winner = candidates.first()?;
    let runner = candidates.get(1)?;
    crate::characteristic_policy::compare_with_clause(
        winner.0,
        winner.1,
        advertisement_for(winner.0, winner.1, advertisements),
        runner.0,
        runner.1,
        advertisement_for(runner.0, runner.1, advertisements),
        observations,
        policy,
    )
    .1
}

fn known_characteristic_reference(
    reference: &crate::PlannerFactRef,
    advertisements: &[RealizationAdvertisement],
) -> bool {
    let Some(id) = characteristic_id(reference) else {
        return true;
    };
    if crate::style::reviewed_style_fact(&conduit_core::CharacteristicId::from(id)) {
        return true;
    }
    advertisements.iter().any(|advertisement| {
        advertisement
            .characteristics
            .iter()
            .any(|item| item.definition.characteristic_id.as_str() == id)
    })
}

fn characteristic_id(reference: &crate::PlannerFactRef) -> Option<&str> {
    match reference {
        crate::PlannerFactRef::RealizationCharacteristic(id) => Some(id.as_str()),
        _ => None,
    }
}

fn advertisement_for<'a>(
    host: &HostAdvertisement,
    offer: &CapabilityOffer,
    advertisements: &'a [RealizationAdvertisement],
) -> Option<&'a RealizationAdvertisement> {
    advertisements.iter().find(|item| {
        item.host_id == host.host_id
            && item.boot_id == host.boot_id
            && item.offer_generation == host.offer_generation
            && item.capability_id == offer.capability_id
    })
}
