//! Numerical certificates gate the existing realization selector before ranking.
use crate::prelude::*;
use crate::{PlannerError, MAXIMUM_REALIZATION_DECISION_RECORDS};
use conduit_core::{
    CapabilityOffer, ExactNumericWork, HostAdvertisement, KindSemanticLaw,
    NumericalRealizationRefusal, NumericalRealizationRequest, PreparedNumericalRealization,
};
use conduit_plot::CheckedGear;

pub(crate) const VERIFICATION_WORK: u64 = 100_000_000;
pub(crate) fn is_profile(profile: &conduit_core::KindId) -> bool {
    *profile == conduit_core::numerical_realization_profile()
}
fn requests(gear: &CheckedGear) -> Result<Vec<NumericalRealizationRequest>, PlannerError> {
    let mut result = Vec::new();
    for law in &gear.semantic_contract.laws {
        let KindSemanticLaw::RealizationRequirement {
            property_profile,
            configuration_key,
        } = law
        else {
            continue;
        };
        if !is_profile(property_profile) {
            continue;
        }
        let request = gear
            .configuration
            .iter()
            .find(|entry| entry.key == *configuration_key)
            .and_then(|entry| match &entry.value {
                conduit_core::ConfigurationValue::Structured(v) => Some(v),
                _ => None,
            })
            .ok_or_else(|| {
                PlannerError::InvalidHardRealizationRequirement(format!(
                    "gear '{}' lacks its typed numerical request",
                    gear.gear_id.as_str()
                ))
            })?;
        let request = NumericalRealizationRequest::from_configuration(
            request,
            &mut ExactNumericWork::new(VERIFICATION_WORK),
        )
        .map_err(|why| {
            PlannerError::InvalidHardRealizationRequirement(format!(
                "gear '{}' has invalid numerical requirements: {why:?}",
                gear.gear_id.as_str()
            ))
        })?;
        result.push(request);
    }
    Ok(result)
}
pub(crate) fn candidate_check(
    gear: &CheckedGear,
    offer: &CapabilityOffer,
) -> Result<Result<(), NumericalRealizationRefusal>, PlannerError> {
    let requests = requests(gear)?;
    if !conduit_core::valid_realization_properties(&offer.realization_properties) {
        return Err(PlannerError::InvalidRealizationProperties(
            offer.capability_id.clone(),
        ));
    }
    if requests.is_empty() {
        return Ok(Ok(()));
    }
    let Some(property) = offer
        .realization_properties
        .iter()
        .find(|property| is_profile(property.profile()))
    else {
        return Ok(Err(NumericalRealizationRefusal::InvalidCertificate));
    };
    let mut work = ExactNumericWork::new(VERIFICATION_WORK);
    let prepared = match PreparedNumericalRealization::from_property(property, &mut work) {
        Ok(prepared) => prepared,
        Err(why) => return Ok(Err(why)),
    };
    for request in &requests {
        if let Err(why) = prepared.admits(request, &mut work) {
            return Ok(Err(why));
        }
    }
    Ok(Ok(()))
}
pub(crate) fn filter_candidates<T, F>(
    gear: &CheckedGear,
    candidates: Vec<T>,
    fields: F,
) -> Result<Vec<T>, PlannerError>
where
    F: for<'a> Fn(&'a T) -> (&'a HostAdvertisement, &'a CapabilityOffer),
{
    if requests(gear)?.is_empty() {
        return Ok(candidates);
    }
    if candidates.len() > MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(PlannerError::PlannerLimitExceeded(
            "numeric candidate evidence bound".into(),
        ));
    }
    let mut admitted = Vec::new();
    let mut reasons = Vec::new();
    for candidate in candidates {
        let (_, offer) = fields(&candidate);
        match candidate_check(gear, offer)? {
            Ok(()) => admitted.push(candidate),
            Err(why) => reasons.push((offer.capability_id.clone(), why)),
        }
    }
    if admitted.is_empty() {
        return Err(PlannerError::HardRealizationRequirementUnsatisfied(
            format!(
                "gear '{}' has no precision-safe numerical realization: {reasons:?}",
                gear.gear_id.as_str()
            ),
        ));
    }
    Ok(admitted)
}

#[cfg(test)]
#[path = "numerical_realization_tests.rs"]
mod tests;
