use crate::observations::{observations_admit, validate_resource_observations};
use crate::policy::validate_policy;
use crate::prelude::*;
use crate::realization::{consume_selected_capacity, reject_unknown_operation_inputs};
use crate::requirements::{
    hard_requirement_failure, validate_requirement_identities, HardRealizationRequirements,
};
use crate::{PlacementChoice, PlacementChoices, PlannerError, RealizationPolicy};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    seal_plan, ArtifactId, BootId, CapabilityId, CapabilityOffer, ConnectionBase, FormIdentity,
    GearId, HostAdvertisement, HostId, ImplementationId, OfferGeneration, Plan,
    RealizationAdvertisement, RealizationCharacteristicId, RealizationCharacteristicValue,
    ResourceObservation,
};
use conduit_form::{CheckedForm, CheckedGear};

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

/// Bounded, prompt-free planning signs for one equal-face candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizationDecisionRecord {
    pub gear_id: GearId,
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
    pub signs: Vec<RealizationDecisionRecord>,
}

pub fn select_realization_with_characteristics(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    requirements: &HardRealizationRequirements,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<PlacementChoice, PlannerError> {
    select_realization_with_characteristics_and_signs(
        gear,
        hosts,
        advertisements,
        requirements,
        observations,
        policy,
    )
    .map(|selection| selection.choice)
}

pub fn select_realization_with_characteristics_and_signs(
    gear: &CheckedGear,
    hosts: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
    requirements: &HardRealizationRequirements,
    observations: &[ResourceObservation],
    policy: &RealizationPolicy,
) -> Result<RealizationSelection, PlannerError> {
    validate_requirement_identities(requirements)?;
    validate_policy(policy)?;
    validate_resource_observations(hosts, observations)?;
    validate_advertisements(hosts, advertisements)?;

    let face_candidates = hosts
        .iter()
        .flat_map(|host| host.capabilities.iter().map(move |offer| (host, offer)))
        .filter(|(_, offer)| offer.checked_face() == gear.checked_face())
        .collect::<Vec<_>>();
    if face_candidates.is_empty() {
        return Err(PlannerError::UnknownCapability(
            gear.kind_id.as_str().to_string(),
        ));
    }
    if face_candidates.len() > MAXIMUM_REALIZATION_DECISION_RECORDS {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "gear '{}' has {} equal-face candidates above the signs bound of {}",
            gear.gear_id.as_str(),
            face_candidates.len(),
            MAXIMUM_REALIZATION_DECISION_RECORDS
        )));
    }
    let mut signs = Vec::with_capacity(face_candidates.len());
    let mut hard_admitted = Vec::with_capacity(face_candidates.len());
    for (host, offer) in face_candidates {
        let facts = advertisement_for(host, offer, advertisements);
        let rejection = hard_requirement_failure(offer, requirements)
            .map(base_rejection)
            .or_else(|| characteristic_rejection(facts, requirements));
        if rejection.is_none() {
            hard_admitted.push((host, offer));
        }
        signs.push(decision_record(
            gear,
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
                "gear '{}' has no hard-admissible realization",
                gear.gear_id.as_str()
            ),
        ));
    }
    let mut observed_admitted = Vec::with_capacity(hard_admitted.len());
    for (host, offer) in hard_admitted {
        if observations_admit(host, offer, observations) {
            observed_admitted.push((host, offer));
        } else if let Some(record) = signs.iter_mut().find(|record| {
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
                "gear '{}' has no realization with current observed resources",
                gear.gear_id.as_str()
            ),
        ));
    }
    observed_admitted.sort_by(|(left_host, left_offer), (right_host, right_offer)| {
        crate::characteristic_policy::compare(
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
    for record in &mut signs {
        if record.host_id == choice.host_id && record.capability_id == choice.capability_id {
            record.disposition = RealizationDecisionDisposition::Selected;
        }
    }
    signs.sort_by(|left, right| {
        left.host_id
            .cmp(&right.host_id)
            .then_with(|| left.capability_id.cmp(&right.capability_id))
    });
    Ok(RealizationSelection { choice, signs })
}

fn decision_record(
    gear: &CheckedGear,
    host: &HostAdvertisement,
    offer: &CapabilityOffer,
    disposition: RealizationDecisionDisposition,
) -> RealizationDecisionRecord {
    RealizationDecisionRecord {
        gear_id: gear.gear_id.clone(),
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
    hosts: &[HostAdvertisement],
    bases: &[ConnectionBase],
    requirements: &BTreeMap<GearId, HardRealizationRequirements>,
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
    policies: &BTreeMap<GearId, RealizationPolicy>,
) -> Result<Plan, PlannerError> {
    reject_unknown_operation_inputs(form, requirements, policies)?;
    validate_resource_observations(hosts, observations)?;
    validate_advertisements(hosts, advertisements)?;
    let mut remaining = observations.to_vec();
    let mut by_gear = BTreeMap::new();
    for gear in &form.gears {
        let choice = select_realization_with_characteristics(
            gear,
            hosts,
            advertisements,
            requirements
                .get(&gear.gear_id)
                .unwrap_or(&HardRealizationRequirements::default()),
            &remaining,
            policies
                .get(&gear.gear_id)
                .unwrap_or(&RealizationPolicy::default()),
        )?;
        consume_selected_capacity(hosts, &choice, &mut remaining)?;
        by_gear.insert(gear.gear_id.clone(), choice);
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
        hosts,
        &PlacementChoices { by_gear },
        bases,
        &plain_requirements,
    )?;
    seal_characteristics(plan, advertisements)
}

fn validate_advertisements(
    hosts: &[HostAdvertisement],
    advertisements: &[RealizationAdvertisement],
) -> Result<(), PlannerError> {
    let mut scopes = BTreeSet::new();
    for advertisement in advertisements {
        let Some(host) = hosts
            .iter()
            .find(|host| host.host_id == advertisement.host_id)
        else {
            return invalid("realization advertisement host is absent from hosts");
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

pub(crate) fn seal_characteristics(
    mut plan: Plan,
    advertisements: &[RealizationAdvertisement],
) -> Result<Plan, PlannerError> {
    for fragment in &mut plan.fragments {
        for gear in &mut fragment.placements {
            if let Some(advertisement) = advertisements.iter().find(|item| {
                item.host_id == gear.host_id
                    && item.boot_id == gear.boot_id
                    && item.offer_generation == gear.offer_generation
                    && item.capability_id == gear.capability_id
            }) {
                gear.realization_characteristics = advertisement.characteristics.clone();
                gear.realization_characteristics.sort();
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
