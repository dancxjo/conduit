//! Exact typed configuration/property equality in the existing candidate path.
//! Domain-specific coverage and numerical refinement retain their own owners.
use crate::prelude::*;
use crate::{PlannerError, MAXIMUM_REALIZATION_DECISION_RECORDS};
use conduit_core::{CapabilityOffer, HostAdvertisement, KindSemanticLaw};
use conduit_plot::CheckedGear;

pub(crate) fn filter_candidates<T, F>(
    gear: &CheckedGear,
    candidates: Vec<T>,
    fields: F,
) -> Result<Vec<T>, PlannerError>
where
    F: for<'a> Fn(&'a T) -> (&'a HostAdvertisement, &'a CapabilityOffer),
{
    let requirements = gear
        .semantic_contract
        .laws
        .iter()
        .filter_map(|law| {
            if let KindSemanticLaw::ExactRealizationProperty(requirement) = law {
                Some(requirement)
            } else {
                None
            }
        })
        .take(conduit_core::MAXIMUM_REALIZATION_PROPERTIES + 1)
        .collect::<Vec<_>>();
    if requirements.len() > conduit_core::MAXIMUM_REALIZATION_PROPERTIES {
        return Err(PlannerError::PlannerLimitExceeded(
            "exact realization requirement bound".into(),
        ));
    }
    if requirements.is_empty() {
        return Ok(candidates);
    }
    if candidates.len() > MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(PlannerError::PlannerLimitExceeded(
            "exact realization candidate bound".into(),
        ));
    }
    let mut admitted = Vec::new();
    let mut rejected = Vec::new();
    for candidate in candidates {
        let (_, offer) = fields(&candidate);
        let mut failure = None;
        for requirement in &requirements {
            let configured = gear
                .configuration
                .iter()
                .find(|entry| entry.key == requirement.configuration_key);
            if let Err(why) = requirement.check(
                configured.map(|entry| &entry.value),
                &offer.realization_properties,
            ) {
                failure = Some((requirement.property_profile.clone(), why));
                break;
            }
        }
        if let Some(why) = failure {
            rejected.push((offer.capability_id.clone(), why));
        } else {
            admitted.push(candidate);
        }
    }
    if admitted.is_empty() {
        return Err(PlannerError::HardRealizationRequirementUnsatisfied(
            format!(
                "gear '{}' has no exact typed realization property: {rejected:?}",
                gear.gear_id.as_str()
            ),
        ));
    }
    Ok(admitted)
}
