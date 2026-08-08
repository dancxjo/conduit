use crate::observations::{observations_admit, validate_resource_observations};
use crate::policy::validate_policy;
use crate::realization::{consume_selected_capacity, reject_unknown_operation_inputs};
use crate::requirements::{
    hard_requirement_failure, validate_requirement_identities, HardRealizationRequirements,
};
use crate::{
    PlacementChoice, PlacementChoices, PlannerError, RealizationPolicy, RealizationPreference,
};
use conduit_core::{
    seal_plan, CapabilityOffer, ConnectionProvider, FormIdentity, HostAdvertisement, OperationId,
    Plan, RealizationAdvertisement, RealizationCharacteristicId, RealizationCharacteristicValue,
    ResourceObservation,
};
use conduit_form::{CheckedForm, CheckedOperation};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub fn select_realization_with_characteristics(
    operation: &CheckedOperation,
    realm: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    requirements: &HardRealizationRequirements,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<PlacementChoice, PlannerError> {
    validate_requirement_identities(requirements)?;
    validate_policy(policy)?;
    validate_resource_observations(realm, observations)?;
    validate_advertisements(realm, advertisements)?;

    let mut face_candidates = realm
        .iter()
        .flat_map(|host| host.capabilities.iter().map(move |offer| (host, offer)))
        .filter(|(_, offer)| offer.checked_face() == operation.checked_face())
        .collect::<Vec<_>>();
    if face_candidates.is_empty() {
        return Err(PlannerError::UnknownCapability(
            operation.kind_id.as_str().to_string(),
        ));
    }
    face_candidates.retain(|(host, offer)| {
        let facts = advertisement_for(host, offer, advertisements);
        hard_requirement_failure(offer, requirements).is_none()
            && characteristics_satisfy(facts, requirements)
    });
    if face_candidates.is_empty() {
        return Err(PlannerError::HardRealizationRequirementUnsatisfied(
            format!(
                "operation '{}' has no hard-admissible realization",
                operation.operation_id.as_str()
            ),
        ));
    }
    face_candidates.retain(|(host, offer)| observations_admit(host, offer, observations));
    if face_candidates.is_empty() {
        return Err(PlannerError::CurrentResourceObservationUnavailable(
            format!(
                "operation '{}' has no realization with current observed resources",
                operation.operation_id.as_str()
            ),
        ));
    }
    face_candidates.sort_by(|(left_host, left_offer), (right_host, right_offer)| {
        compare(
            left_host,
            left_offer,
            advertisement_for(left_host, left_offer, advertisements),
            right_host,
            right_offer,
            advertisement_for(right_host, right_offer, advertisements),
            policy,
        )
    });
    Ok(PlacementChoice {
        host_id: face_candidates[0].0.host_id.clone(),
        capability_id: face_candidates[0].1.capability_id.clone(),
    })
}

pub fn plan_selected_realizations_with_characteristics(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    providers: &[ConnectionProvider],
    requirements: &BTreeMap<OperationId, HardRealizationRequirements>,
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
    policies: &BTreeMap<OperationId, RealizationPolicy>,
) -> Result<Plan, PlannerError> {
    reject_unknown_operation_inputs(form, requirements, policies)?;
    validate_resource_observations(realm, observations)?;
    validate_advertisements(realm, advertisements)?;
    let mut remaining = observations.to_vec();
    let mut by_operation = BTreeMap::new();
    for operation in &form.operations {
        let choice = select_realization_with_characteristics(
            operation,
            realm,
            advertisements,
            requirements
                .get(&operation.operation_id)
                .unwrap_or(&HardRealizationRequirements::default()),
            &remaining,
            policies
                .get(&operation.operation_id)
                .unwrap_or(&RealizationPolicy::default()),
        )?;
        consume_selected_capacity(realm, &choice, &mut remaining)?;
        by_operation.insert(operation.operation_id.clone(), choice);
    }
    let mut plain_requirements = requirements.clone();
    for requirement in plain_requirements.values_mut() {
        requirement.minimum_characteristic_counts.clear();
        requirement.maximum_characteristic_counts.clear();
        requirement.required_characteristic_flags.clear();
        requirement.required_characteristic_labels.clear();
    }
    let plan = crate::plan_with_hard_requirements(
        form,
        realm,
        &PlacementChoices { by_operation },
        providers,
        &plain_requirements,
    )?;
    seal_characteristics(plan, advertisements)
}

fn validate_advertisements(
    realm: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
) -> Result<(), PlannerError> {
    let mut scopes = BTreeSet::new();
    for advertisement in advertisements {
        let Some(host) = realm
            .iter()
            .find(|host| host.host_id == advertisement.host_id)
        else {
            return invalid("realization advertisement host is absent from realm");
        };
        if host.boot_id != advertisement.boot_id
            || host.offer_generation != advertisement.offer_generation
            || !host
                .capabilities
                .iter()
                .any(|offer| offer.capability_id == advertisement.capability_id)
        {
            return invalid("realization advertisement scope is stale or unknown");
        }
        if !scopes.insert((
            advertisement.host_id.clone(),
            advertisement.capability_id.clone(),
        )) {
            return invalid("realization advertisement scope must be unique");
        }
        let mut ids = BTreeSet::new();
        for characteristic in &advertisement.characteristics {
            if characteristic.characteristic_id.as_str().is_empty()
                || !ids.insert(characteristic.characteristic_id.clone())
            {
                return invalid(
                    "realization characteristic identities must be non-empty and unique",
                );
            }
        }
    }
    Ok(())
}

fn characteristics_satisfy(
    advertisement: Option<&RealizationAdvertisement>,
    requirements: &HardRealizationRequirements,
) -> bool {
    requirements
        .minimum_characteristic_counts
        .iter()
        .all(|(id, minimum)| count(advertisement, id).is_some_and(|value| value >= *minimum))
        && requirements
            .maximum_characteristic_counts
            .iter()
            .all(|(id, maximum)| count(advertisement, id).is_some_and(|value| value <= *maximum))
        && requirements
            .required_characteristic_flags
            .iter()
            .all(|(id, required)| {
                value(advertisement, id) == Some(&RealizationCharacteristicValue::Flag(*required))
            })
        && requirements
            .required_characteristic_labels
            .iter()
            .all(|(id, required)| {
                value(advertisement, id)
                    == Some(&RealizationCharacteristicValue::Label(required.clone()))
            })
}

#[allow(clippy::too_many_arguments)]
fn compare(
    left_host: &HostAdvertisement,
    left_offer: &CapabilityOffer,
    left: Option<&RealizationAdvertisement>,
    right_host: &HostAdvertisement,
    right_offer: &CapabilityOffer,
    right: Option<&RealizationAdvertisement>,
    policy: &RealizationPolicy,
) -> Ordering {
    for preference in &policy.preferences {
        let ordering = match preference {
            RealizationPreference::MinimizeResourceUnits(class) => {
                resource_units(left_offer, class).cmp(&resource_units(right_offer, class))
            }
            RealizationPreference::MaximizeQueueItems => right_offer
                .limits
                .max_queue_items
                .cmp(&left_offer.limits.max_queue_items),
            RealizationPreference::MaximizeQueueBytes => right_offer
                .limits
                .max_queue_bytes
                .cmp(&left_offer.limits.max_queue_bytes),
            RealizationPreference::PreferWithoutHostOperation(contract) => {
                has_host_operation(left_offer, contract)
                    .cmp(&has_host_operation(right_offer, contract))
            }
            RealizationPreference::PreferWithoutAuthority(contract) => {
                has_authority(left_offer, contract).cmp(&has_authority(right_offer, contract))
            }
            RealizationPreference::MinimizeCharacteristicCount(id) => {
                count(left, id).cmp(&count(right, id))
            }
            RealizationPreference::MaximizeCharacteristicCount(id) => {
                count(right, id).cmp(&count(left, id))
            }
            RealizationPreference::PreferCharacteristicFlag {
                characteristic_id,
                value: preferred,
            } => flag_distance(left, characteristic_id, *preferred).cmp(&flag_distance(
                right,
                characteristic_id,
                *preferred,
            )),
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left_host
        .host_id
        .cmp(&right_host.host_id)
        .then_with(|| left_offer.capability_id.cmp(&right_offer.capability_id))
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

fn value<'a>(
    advertisement: Option<&'a RealizationAdvertisement>,
    id: &RealizationCharacteristicId,
) -> Option<&'a RealizationCharacteristicValue> {
    advertisement?
        .characteristics
        .iter()
        .find(|item| &item.characteristic_id == id)
        .map(|item| &item.value)
}

fn count(
    advertisement: Option<&RealizationAdvertisement>,
    id: &RealizationCharacteristicId,
) -> Option<u64> {
    match value(advertisement, id) {
        Some(RealizationCharacteristicValue::Count(value)) => Some(*value),
        _ => None,
    }
}

fn flag_distance(
    advertisement: Option<&RealizationAdvertisement>,
    id: &RealizationCharacteristicId,
    preferred: bool,
) -> u8 {
    match value(advertisement, id) {
        Some(RealizationCharacteristicValue::Flag(value)) if *value == preferred => 0,
        _ => 1,
    }
}

fn resource_units(offer: &CapabilityOffer, class: &conduit_core::ResourceClassId) -> u64 {
    offer
        .resource_requirements
        .iter()
        .filter(|item| &item.class_id == class)
        .map(|item| u64::from(item.units))
        .sum()
}
fn has_host_operation(
    offer: &CapabilityOffer,
    contract: &conduit_core::HostOperationContractId,
) -> bool {
    offer
        .host_operations
        .iter()
        .any(|item| &item.contract_id == contract)
}
fn has_authority(offer: &CapabilityOffer, contract: &conduit_core::AuthorityContractId) -> bool {
    offer
        .authority_requirements
        .iter()
        .any(|item| &item.contract_id == contract)
}

fn seal_characteristics(
    mut plan: Plan,
    advertisements: &[RealizationAdvertisement],
) -> Result<Plan, PlannerError> {
    for fragment in &mut plan.fragments {
        for operation in &mut fragment.placements {
            if let Some(advertisement) = advertisements.iter().find(|item| {
                item.host_id == operation.host_id
                    && item.boot_id == operation.boot_id
                    && item.offer_generation == operation.offer_generation
                    && item.capability_id == operation.capability_id
            }) {
                operation.realization_characteristics = advertisement.characteristics.clone();
                operation.realization_characteristics.sort();
            }
        }
    }
    Ok(seal_plan(
        FormIdentity {
            source_document_id: plan.source_document_id,
            checked_form_id: plan.checked_form_id,
            expanded_form_id: plan.expanded_form_id,
        },
        plan.fragments,
    ))
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidHardRealizationRequirement(
        detail.to_string(),
    ))
}
