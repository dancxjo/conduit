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
    seal_plan, ArtifactId, BootId, CapabilityId, CapabilityOffer, ConnectionProvider, FormIdentity,
    HostAdvertisement, HostId, ImplementationId, OfferGeneration, OperationId, Plan,
    RealizationAdvertisement, RealizationCharacteristicId, RealizationCharacteristicValue,
    ResourceObservation,
};
use conduit_form::{CheckedForm, CheckedOperation};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

pub const MAXIMUM_REALIZATION_DECISION_RECORDS: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationRejection {
    QueueItemBound,
    QueueByteBound,
    ResourceUnitCeiling,
    HostOperationAllowlist,
    AuthorityContractAllowlist,
    MinimumCharacteristicCount(RealizationCharacteristicId),
    MaximumCharacteristicCount(RealizationCharacteristicId),
    RequiredCharacteristicFlag(RealizationCharacteristicId),
    RequiredCharacteristicLabel(RealizationCharacteristicId),
    CurrentResourceObservation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationDecisionDisposition {
    Rejected(RealizationRejection),
    Admitted,
    Selected,
}

/// Bounded, prompt-free planning evidence for one equal-face candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationDecisionRecord {
    pub operation_id: OperationId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub disposition: RealizationDecisionDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationSelection {
    pub choice: PlacementChoice,
    pub evidence: Vec<RealizationDecisionRecord>,
}

pub fn select_realization_with_characteristics(
    operation: &CheckedOperation,
    realm: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    requirements: &HardRealizationRequirements,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<PlacementChoice, PlannerError> {
    select_realization_with_characteristics_and_evidence(
        operation,
        realm,
        advertisements,
        requirements,
        observations,
        policy,
    )
    .map(|selection| selection.choice)
}

pub fn select_realization_with_characteristics_and_evidence(
    operation: &CheckedOperation,
    realm: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    requirements: &HardRealizationRequirements,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<RealizationSelection, PlannerError> {
    validate_requirement_identities(requirements)?;
    validate_policy(policy)?;
    validate_resource_observations(realm, observations)?;
    validate_advertisements(realm, advertisements)?;

    let face_candidates = realm
        .iter()
        .flat_map(|host| host.capabilities.iter().map(move |offer| (host, offer)))
        .filter(|(_, offer)| offer.checked_face() == operation.checked_face())
        .collect::<Vec<_>>();
    if face_candidates.is_empty() {
        return Err(PlannerError::UnknownCapability(
            operation.kind_id.as_str().to_string(),
        ));
    }
    if face_candidates.len() > MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "operation '{}' has {} equal-face candidates above the evidence bound of {}",
            operation.operation_id.as_str(),
            face_candidates.len(),
            MAXIMUM_REALIZATION_DECISION_RECORDS
        )));
    }
    let mut evidence = Vec::with_capacity(face_candidates.len());
    let mut hard_admitted = Vec::with_capacity(face_candidates.len());
    for (host, offer) in face_candidates {
        let facts = advertisement_for(host, offer, advertisements);
        let rejection = hard_requirement_failure(offer, requirements)
            .map(base_rejection)
            .or_else(|| characteristic_rejection(facts, requirements));
        if rejection.is_none() {
            hard_admitted.push((host, offer));
        }
        evidence.push(decision_record(
            operation,
            host,
            offer,
            rejection.map_or(
                RealizationDecisionDisposition::Admitted,
                RealizationDecisionDisposition::Rejected,
            ),
        ));
    }
    if hard_admitted.is_empty() {
        return Err(PlannerError::HardRealizationRequirementUnsatisfied(
            format!(
                "operation '{}' has no hard-admissible realization",
                operation.operation_id.as_str()
            ),
        ));
    }
    let mut observed_admitted = Vec::with_capacity(hard_admitted.len());
    for (host, offer) in hard_admitted {
        if observations_admit(host, offer, observations) {
            observed_admitted.push((host, offer));
        } else if let Some(record) = evidence.iter_mut().find(|record| {
            record.host_id == host.host_id && record.capability_id == offer.capability_id
        }) {
            record.disposition = RealizationDecisionDisposition::Rejected(
                RealizationRejection::CurrentResourceObservation,
            );
        }
    }
    if observed_admitted.is_empty() {
        return Err(PlannerError::CurrentResourceObservationUnavailable(
            format!(
                "operation '{}' has no realization with current observed resources",
                operation.operation_id.as_str()
            ),
        ));
    }
    observed_admitted.sort_by(|(left_host, left_offer), (right_host, right_offer)| {
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
    let choice = PlacementChoice {
        host_id: observed_admitted[0].0.host_id.clone(),
        capability_id: observed_admitted[0].1.capability_id.clone(),
    };
    for record in &mut evidence {
        if record.host_id == choice.host_id && record.capability_id == choice.capability_id {
            record.disposition = RealizationDecisionDisposition::Selected;
        }
    }
    evidence.sort_by(|left, right| {
        left.host_id
            .cmp(&right.host_id)
            .then_with(|| left.capability_id.cmp(&right.capability_id))
    });
    Ok(RealizationSelection { choice, evidence })
}

fn decision_record(
    operation: &CheckedOperation,
    host: &HostAdvertisement,
    offer: &CapabilityOffer,
    disposition: RealizationDecisionDisposition,
) -> RealizationDecisionRecord {
    RealizationDecisionRecord {
        operation_id: operation.operation_id.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        capability_id: offer.capability_id.clone(),
        implementation_id: offer.implementation.implementation_id.clone(),
        artifact_id: offer.implementation.artifact_id.clone(),
        disposition,
    }
}

fn base_rejection(dimension: &'static str) -> RealizationRejection {
    match dimension {
        "queue item bound" => RealizationRejection::QueueItemBound,
        "queue byte bound" => RealizationRejection::QueueByteBound,
        "resource-unit ceiling" => RealizationRejection::ResourceUnitCeiling,
        "host-operation allowlist" => RealizationRejection::HostOperationAllowlist,
        "authority-contract allowlist" => RealizationRejection::AuthorityContractAllowlist,
        _ => unreachable!("hard requirement failures have a closed vocabulary"),
    }
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

fn characteristic_rejection(
    advertisement: Option<&RealizationAdvertisement>,
    requirements: &HardRealizationRequirements,
) -> Option<RealizationRejection> {
    if let Some((id, _)) = requirements
        .minimum_characteristic_counts
        .iter()
        .find(|(id, minimum)| count(advertisement, id).is_none_or(|value| value < **minimum))
    {
        return Some(RealizationRejection::MinimumCharacteristicCount(id.clone()));
    }
    if let Some((id, _)) = requirements
        .maximum_characteristic_counts
        .iter()
        .find(|(id, maximum)| count(advertisement, id).is_none_or(|value| value > **maximum))
    {
        return Some(RealizationRejection::MaximumCharacteristicCount(id.clone()));
    }
    if let Some((id, _)) =
        requirements
            .required_characteristic_flags
            .iter()
            .find(|(id, required)| {
                value(advertisement, id) != Some(&RealizationCharacteristicValue::Flag(**required))
            })
    {
        return Some(RealizationRejection::RequiredCharacteristicFlag(id.clone()));
    }
    requirements
        .required_characteristic_labels
        .iter()
        .find(|(id, required)| {
            value(advertisement, id)
                != Some(&RealizationCharacteristicValue::Label((*required).clone()))
        })
        .map(|(id, _)| RealizationRejection::RequiredCharacteristicLabel(id.clone()))
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
