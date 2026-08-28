use crate::fact_policy::{compare_preference, CandidateFacts};
use crate::{PlannerError, RealizationPolicy};
use conduit_core::{
    CapabilityOffer, HostAdvertisement, RealizationAdvertisement, ResourceObservation,
};
use core::cmp::Ordering;

#[allow(clippy::too_many_arguments)]
pub(crate) fn compare(
    left_host: &HostAdvertisement,
    left_offer: &CapabilityOffer,
    left: Option<&RealizationAdvertisement>,
    right_host: &HostAdvertisement,
    right_offer: &CapabilityOffer,
    right: Option<&RealizationAdvertisement>,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Ordering {
    compare_with_clause(
        left_host,
        left_offer,
        left,
        right_host,
        right_offer,
        right,
        observations,
        policy,
    )
    .0
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compare_with_clause(
    left_host: &HostAdvertisement,
    left_offer: &CapabilityOffer,
    left: Option<&RealizationAdvertisement>,
    right_host: &HostAdvertisement,
    right_offer: &CapabilityOffer,
    right: Option<&RealizationAdvertisement>,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> (Ordering, Option<u16>) {
    let left = CandidateFacts {
        host: left_host,
        offer: left_offer,
        realization: left,
        observations,
    };
    let right = CandidateFacts {
        host: right_host,
        offer: right_offer,
        realization: right,
        observations,
    };
    for (index, preference) in policy.preferences.iter().enumerate() {
        let ordering = compare_preference(&left, &right, &preference.lower())
            .expect("preferences are validated before sorting");
        if ordering != Ordering::Equal {
            return (ordering, u16::try_from(index).ok());
        }
    }
    (
        left_host
            .host_id
            .cmp(&right_host.host_id)
            .then_with(|| left_offer.capability_id.cmp(&right_offer.capability_id)),
        None,
    )
}

pub(crate) fn validate_preferences(
    candidates: &[(
        &HostAdvertisement,
        &CapabilityOffer,
        Option<&RealizationAdvertisement>,
    )],
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<(), PlannerError> {
    for preference in &policy.preferences {
        let preference = preference.lower();
        crate::fact_policy::validate_preference(&preference).map_err(invalid_policy)?;
        for (host, offer, realization) in candidates {
            let candidate = CandidateFacts {
                host,
                offer,
                realization: *realization,
                observations,
            };
            compare_preference(&candidate, &candidate, &preference).map_err(invalid_policy)?;
        }
    }
    Ok(())
}

fn invalid_policy(error: crate::fact_policy::FactPolicyError) -> PlannerError {
    PlannerError::InvalidRealizationPolicy(format!("invalid generic preference: {error:?}"))
}
