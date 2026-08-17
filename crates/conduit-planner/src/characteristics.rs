use crate::decision_evidence::{
    RealizationDecisionDisposition, RealizationDecisionRecord, RealizationRejection,
    RealizationSelection, MAXIMUM_REALIZATION_DECISION_RECORDS,
};
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
    CapabilityOffer, CharacteristicId, CharacteristicQuantity, CharacteristicValue, ConnectionBase,
    GearId, HostAdvertisement, Plan, RealizationAdvertisement, ResourceObservation,
};
use conduit_form::{CheckedForm, CheckedGear};

pub const MAXIMUM_PLANNER_POLICY_CLAUSES: usize = 64;

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
    crate::generic_selection::validate_inputs(requirements, policy, advertisements)?;

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
        let rejection = match rejection {
            Some(rejection) => Some(rejection),
            None => crate::generic_selection::predicate_rejection(
                host,
                offer,
                facts,
                observations,
                &requirements.predicates,
            )?,
        };
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
    crate::generic_selection::validate_preferences(
        &observed_admitted,
        advertisements,
        observations,
        policy,
    )?;
    observed_admitted.sort_by(|(left_host, left_offer), (right_host, right_offer)| {
        crate::characteristic_policy::compare(
            left_host,
            left_offer,
            advertisement_for(left_host, left_offer, advertisements),
            right_host,
            right_offer,
            advertisement_for(right_host, right_offer, advertisements),
            observations,
            policy,
        )
    });
    let choice = PlacementChoice {
        host_id: observed_admitted[0].0.host_id.clone(),
        capability_id: observed_admitted[0].1.capability_id.clone(),
    };
    let decisive_preference_clause = crate::generic_selection::decisive_clause(
        &observed_admitted,
        advertisements,
        observations,
        policy,
    );
    for record in &mut signs {
        if record.host_id == choice.host_id && record.capability_id == choice.capability_id {
            record.disposition = RealizationDecisionDisposition::Selected;
            record.decisive_preference_clause = decisive_preference_clause;
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
        decisive_preference_clause: None,
        clause_source: None,
        decisive_preference_source: None,
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
    plan_selected_realizations_with_characteristics_and_authority(
        form,
        SelectedRealizationPlanning {
            hosts,
            bases,
            requirements,
            advertisements,
            observations,
            policies,
            connection_item_capacity: conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[],
        },
    )
}

pub struct SelectedRealizationPlanning<'a> {
    pub hosts: &'a [HostAdvertisement],
    pub bases: &'a [ConnectionBase],
    pub requirements: &'a BTreeMap<GearId, HardRealizationRequirements>,
    pub advertisements: &'a [RealizationAdvertisement],
    pub observations: &'a [ResourceObservation],
    pub policies: &'a BTreeMap<GearId, RealizationPolicy>,
    pub connection_item_capacity: u16,
    pub connection_byte_capacity: u32,
    pub authority_grants: &'a [conduit_core::AuthorityGrant],
}

/// Selects against fresh resource observations and exact realization facts,
/// then seals independently supplied authority grants into the ordinary Plan.
pub fn plan_selected_realizations_with_characteristics_and_authority(
    form: &CheckedForm,
    options: SelectedRealizationPlanning<'_>,
) -> Result<Plan, PlannerError> {
    let SelectedRealizationPlanning {
        hosts,
        bases,
        requirements,
        advertisements,
        observations,
        policies,
        connection_item_capacity,
        connection_byte_capacity,
        authority_grants,
    } = options;
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    plan_selected_realizations_with_characteristics_and_options(
        form,
        hosts,
        bases,
        requirements,
        advertisements,
        observations,
        policies,
        crate::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity,
            connection_byte_capacity,
            authority_grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
}

/// Selects through the generic policy language and seals the result through
/// the ordinary Line, resource, and authority Plan machinery.
#[allow(clippy::too_many_arguments)]
pub fn plan_selected_realizations_with_characteristics_and_options(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    bases: &[ConnectionBase],
    requirements: &BTreeMap<GearId, HardRealizationRequirements>,
    advertisements: &[RealizationAdvertisement],
    observations: &[ResourceObservation],
    policies: &BTreeMap<GearId, RealizationPolicy>,
    planning_options: crate::PlanningOptions<'_>,
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
        requirement.predicates.clear();
        requirement.minimum_characteristic_counts.clear();
        requirement.maximum_characteristic_counts.clear();
        requirement.required_characteristic_flags.clear();
        requirement.required_characteristic_labels.clear();
    }
    let placements = PlacementChoices { by_gear };
    crate::requirements::validate_hard_requirements(form, hosts, &placements, &plain_requirements)?;
    let plan = crate::plan_with_options(form, hosts, &placements, bases, planning_options)?;
    crate::characteristic_sealing::seal_characteristics(plan, advertisements)
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
            if characteristic
                .definition
                .characteristic_id
                .as_str()
                .is_empty()
                || !ids.insert(characteristic.definition.characteristic_id.clone())
            {
                return invalid(
                    "realization characteristic identities must be non-empty and unique",
                );
            }
            characteristic
                .definition
                .validate_realization_value(&characteristic.value)
                .map_err(|error| {
                    PlannerError::InvalidHardRealizationRequirement(format!(
                        "invalid realization characteristic '{}': {error:?}",
                        characteristic.definition.characteristic_id.as_str()
                    ))
                })?;
        }
    }
    Ok(())
}

fn characteristic_rejection(
    advertisement: Option<&RealizationAdvertisement>,
    requirements: &HardRealizationRequirements,
) -> Option<RealizationRejection> {
    if let Some((id, _)) =
        requirements
            .minimum_characteristic_counts
            .iter()
            .find(|(id, minimum)| {
                count(advertisement, id)
                    .is_none_or(|value| value.unit != minimum.unit || value.value < minimum.value)
            })
    {
        return Some(RealizationRejection::MinimumCharacteristicCount(id.clone()));
    }
    if let Some((id, _)) =
        requirements
            .maximum_characteristic_counts
            .iter()
            .find(|(id, maximum)| {
                count(advertisement, id)
                    .is_none_or(|value| value.unit != maximum.unit || value.value > maximum.value)
            })
    {
        return Some(RealizationRejection::MaximumCharacteristicCount(id.clone()));
    }
    if let Some((id, _)) =
        requirements
            .required_characteristic_flags
            .iter()
            .find(|(id, required)| {
                value(advertisement, id) != Some(&CharacteristicValue::Boolean(**required))
            })
    {
        return Some(RealizationRejection::RequiredCharacteristicFlag(id.clone()));
    }
    requirements
        .required_characteristic_labels
        .iter()
        .find(|(id, required)| {
            value(advertisement, id) != Some(&CharacteristicValue::Categorical((*required).clone()))
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
    id: &CharacteristicId,
) -> Option<&'a CharacteristicValue> {
    advertisement?
        .characteristics
        .iter()
        .find(|item| &item.definition.characteristic_id == id)
        .map(|item| &item.value)
}

fn count(
    advertisement: Option<&RealizationAdvertisement>,
    id: &CharacteristicId,
) -> Option<CharacteristicQuantity> {
    match value(advertisement, id) {
        Some(CharacteristicValue::UnsignedQuantity { value, unit }) => {
            Some(CharacteristicQuantity {
                value: *value,
                unit: *unit,
            })
        }
        _ => None,
    }
}

fn invalid<T>(detail: &str) -> Result<T, PlannerError> {
    Err(PlannerError::InvalidHardRealizationRequirement(
        detail.to_string(),
    ))
}
