use crate::prelude::*;
use crate::{plan, PlacementChoices, PlannerError, PlannerPredicate};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    AuthorityContractId, BaseImplementationId, CharacteristicId, CharacteristicQuantity, GearId,
    HostAdvertisement, HostOperationContractId, Plan, ResourceClassId,
};
use conduit_form::CheckedForm;

/// Hard admissibility constraints for one semantic gear realization.
///
/// These constraints are boolean gates. They do not rank candidates and do
/// not contain host-supplied desirability scores.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HardRealizationRequirements {
    /// Generic subject-owned hard predicates. Legacy fields below lower to the
    /// same hard-before-policy selection phase during migration.
    pub predicates: Vec<PlannerPredicate>,
    pub minimum_queue_items: u16,
    pub minimum_queue_bytes: u32,
    /// Maximum units the realization may require from each named class.
    /// A zero ceiling forbids use of that resource class.
    pub maximum_resource_units: BTreeMap<ResourceClassId, u32>,
    /// `None` permits any declared gear; `Some` is an exact allowlist.
    pub permitted_host_operations: Option<BTreeSet<HostOperationContractId>>,
    /// `None` permits any declared authority; `Some` is an exact allowlist.
    pub permitted_authority_contracts: Option<BTreeSet<AuthorityContractId>>,
    pub minimum_characteristic_counts: BTreeMap<CharacteristicId, CharacteristicQuantity>,
    pub maximum_characteristic_counts: BTreeMap<CharacteristicId, CharacteristicQuantity>,
    pub required_characteristic_flags: BTreeMap<CharacteristicId, bool>,
    pub required_characteristic_labels: BTreeMap<CharacteristicId, String>,
}

impl HardRealizationRequirements {
    /// Lowers retained characteristic-specific R2 fields into the common typed
    /// predicate vocabulary. Generic predicates already supplied by the caller
    /// remain first and therefore retain their evidence clause identities.
    pub fn lower_characteristic_predicates(&self) -> Vec<PlannerPredicate> {
        let mut predicates = self.predicates.clone();
        predicates.extend(
            self.minimum_characteristic_counts
                .iter()
                .map(|(id, quantity)| PlannerPredicate::AtLeast {
                    fact: crate::PlannerFactRef::RealizationCharacteristic(id.clone()),
                    value: crate::PlannerFactValue::Quantity {
                        value: quantity.value,
                        unit: quantity.unit,
                    },
                }),
        );
        predicates.extend(
            self.maximum_characteristic_counts
                .iter()
                .map(|(id, quantity)| PlannerPredicate::AtMost {
                    fact: crate::PlannerFactRef::RealizationCharacteristic(id.clone()),
                    value: crate::PlannerFactValue::Quantity {
                        value: quantity.value,
                        unit: quantity.unit,
                    },
                }),
        );
        predicates.extend(
            self.required_characteristic_flags
                .iter()
                .map(|(id, value)| PlannerPredicate::Equal {
                    fact: crate::PlannerFactRef::RealizationCharacteristic(id.clone()),
                    value: crate::PlannerFactValue::Boolean(*value),
                }),
        );
        predicates.extend(
            self.required_characteristic_labels
                .iter()
                .map(|(id, value)| PlannerPredicate::Equal {
                    fact: crate::PlannerFactRef::RealizationCharacteristic(id.clone()),
                    value: crate::PlannerFactValue::Category(value.clone()),
                }),
        );
        predicates
    }
}

pub fn plan_with_hard_requirements(
    form: &CheckedForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
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
    if requirements.values().any(|requirement| {
        has_characteristic_requirements(requirement) || !requirement.predicates.is_empty()
    }) {
        return Err(PlannerError::InvalidHardRealizationRequirement(
            "generic or characteristic requirements require exact planner fact inputs".to_string(),
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
    requirement.predicates.iter().any(|predicate| {
        matches!(
            predicate.fact(),
            crate::PlannerFactRef::RealizationCharacteristic(_)
        )
    }) || !requirement.minimum_characteristic_counts.is_empty()
        || !requirement.maximum_characteristic_counts.is_empty()
        || !requirement.required_characteristic_flags.is_empty()
        || !requirement.required_characteristic_labels.is_empty()
}

pub(crate) fn validate_requirement_identities(
    requirement: &HardRealizationRequirements,
) -> Result<(), PlannerError> {
    if requirement.predicates.len() > crate::MAXIMUM_PLANNER_POLICY_CLAUSES {
        return Err(PlannerError::PlannerLimitExceeded(format!(
            "hard policy exceeds the {} clause bound",
            crate::MAXIMUM_PLANNER_POLICY_CLAUSES
        )));
    }
    crate::fact_policy::validate_predicates(&requirement.predicates).map_err(|error| {
        PlannerError::InvalidHardRealizationRequirement(format!(
            "invalid generic predicate: {error:?}"
        ))
    })?;
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
