use crate::{plan, PlacementChoices, PlannerError};
use conduit_core::{
    AuthorityContractId, ConnectionBase, GearId, HostAdvertisement, HostOperationContractId, Plan,
    RealizationCharacteristicId, ResourceClassId,
};
use conduit_form::CheckedForm;
use std::collections::{BTreeMap, BTreeSet};

/// Hard admissibility constraints for one semantic gear realization.
///
/// These constraints are boolean gates. They do not rank candidates and do
/// not contain host-supplied desirability scores.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HardRealizationRequirements {
    pub minimum_queue_items: u16,
    pub minimum_queue_bytes: u32,
    /// Maximum units the realization may require from each named class.
    /// A zero ceiling forbids use of that resource class.
    pub maximum_resource_units: BTreeMap<ResourceClassId, u32>,
    /// `None` permits any declared gear; `Some` is an exact allowlist.
    pub permitted_host_operations: Option<BTreeSet<HostOperationContractId>>,
    /// `None` permits any declared authority; `Some` is an exact allowlist.
    pub permitted_authority_contracts: Option<BTreeSet<AuthorityContractId>>,
    pub minimum_characteristic_counts: BTreeMap<RealizationCharacteristicId, u64>,
    pub maximum_characteristic_counts: BTreeMap<RealizationCharacteristicId, u64>,
    pub required_characteristic_flags: BTreeMap<RealizationCharacteristicId, bool>,
    pub required_characteristic_labels: BTreeMap<RealizationCharacteristicId, String>,
}

pub fn plan_with_hard_requirements(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[ConnectionBase],
    requirements: &BTreeMap<GearId, HardRealizationRequirements>,
) -> Result<Plan, PlannerError> {
    validate_hard_requirements(form, hosts, placements, requirements)?;
    plan(form, hosts, placements, bases)
}

pub(crate) fn validate_hard_requirements(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    requirements: &BTreeMap<GearId, HardRealizationRequirements>,
) -> Result<(), PlannerError> {
    if requirements.values().any(has_characteristic_requirements) {
        return Err(PlannerError::InvalidHardRealizationRequirement(
            "characteristic requirements require exact realization advertisements".to_string(),
        ));
    }
    for gear_id in requirements.keys() {
        if !form.gears.iter().any(|gear| &gear.gear_id == gear_id) {
            return Err(PlannerError::UnknownGear(gear_id.as_str().to_string()));
        }
    }

    for gear in &form.gears {
        let Some(requirement) = requirements.get(&gear.gear_id) else {
            continue;
        };
        validate_requirement_identities(requirement)?;
        let choice = placements
            .by_gear
            .get(&gear.gear_id)
            .ok_or_else(|| PlannerError::MissingPlacement(gear.gear_id.as_str().to_string()))?;
        let host = hosts
            .iter()
            .find(|host| host.host_id == choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
        let offer = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| {
                PlannerError::UnknownCapability(choice.capability_id.as_str().to_string())
            })?;

        if offer.checked_face() != gear.checked_face() {
            return Err(PlannerError::IncompatibleCheckedFace(format!(
                "gear '{}' face differs from capability '{}' face",
                gear.gear_id.as_str(),
                offer.capability_id.as_str()
            )));
        }
        if let Some(dimension) = hard_requirement_failure(offer, requirement) {
            return unsatisfied(gear_id(gear), dimension);
        }
    }
    Ok(())
}

pub(crate) fn has_characteristic_requirements(requirement: &HardRealizationRequirements) -> bool {
    !requirement.minimum_characteristic_counts.is_empty()
        || !requirement.maximum_characteristic_counts.is_empty()
        || !requirement.required_characteristic_flags.is_empty()
        || !requirement.required_characteristic_labels.is_empty()
}

pub(crate) fn validate_requirement_identities(
    requirement: &HardRealizationRequirements,
) -> Result<(), PlannerError> {
    let empty_resource = requirement
        .maximum_resource_units
        .keys()
        .any(|identity| identity.as_str().is_empty());
    let empty_host_operation = requirement
        .permitted_host_operations
        .iter()
        .flatten()
        .any(|identity| identity.as_str().is_empty());
    let empty_authority = requirement
        .permitted_authority_contracts
        .iter()
        .flatten()
        .any(|identity| identity.as_str().is_empty());
    let empty_characteristic = requirement
        .minimum_characteristic_counts
        .keys()
        .chain(requirement.maximum_characteristic_counts.keys())
        .chain(requirement.required_characteristic_flags.keys())
        .chain(requirement.required_characteristic_labels.keys())
        .any(|identity| identity.as_str().is_empty());
    if empty_resource || empty_host_operation || empty_authority || empty_characteristic {
        return Err(PlannerError::InvalidHardRealizationRequirement(
            "requirement identities must be non-empty".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn hard_requirement_failure(
    offer: &conduit_core::CapabilityOffer,
    requirement: &HardRealizationRequirements,
) -> Option<&'static str> {
    if offer.limits.max_queue_items < requirement.minimum_queue_items {
        return Some("queue item bound");
    }
    if offer.limits.max_queue_bytes < requirement.minimum_queue_bytes {
        return Some("queue byte bound");
    }
    if offer.resource_requirements.iter().any(|resource| {
        requirement
            .maximum_resource_units
            .get(&resource.class_id)
            .is_some_and(|maximum| resource.units > *maximum)
    }) {
        return Some("resource-unit ceiling");
    }
    if requirement
        .permitted_host_operations
        .as_ref()
        .is_some_and(|permitted| {
            offer
                .host_operations
                .iter()
                .any(|required| !permitted.contains(&required.contract_id))
        })
    {
        return Some("host-operation allowlist");
    }
    if requirement
        .permitted_authority_contracts
        .as_ref()
        .is_some_and(|permitted| {
            offer
                .authority_requirements
                .iter()
                .any(|required| !permitted.contains(&required.contract_id))
        })
    {
        return Some("authority-contract allowlist");
    }
    None
}

fn gear_id(gear: &conduit_form::CheckedGear) -> &GearId {
    &gear.gear_id
}

fn unsatisfied<T>(gear_id: &GearId, dimension: &str) -> Result<T, PlannerError> {
    Err(PlannerError::HardRealizationRequirementUnsatisfied(
        format!("gear '{}' failed {dimension}", gear_id.as_str()),
    ))
}
