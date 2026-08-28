use crate::prelude::*;
use conduit_core::{
    AuthorityContractId, CapabilityOffer, CharacteristicId, CharacteristicUnit,
    CharacteristicValue, ComputeServiceGuarantee, ComputeTopologyGroupId, HostAdvertisement,
    HostOperationContractId, RealizationAdvertisement, ResourceClassId, ResourceObservation,
};
use core::cmp::Ordering;

mod validation;
pub(crate) use validation::{validate_predicates, validate_preference};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlannerFactRef {
    /// Exact current Host identity. This is realization truth supplied to the
    /// planner, never authored Form meaning or an aggregate Host score.
    HostIdentity,
    RealizationCharacteristic(CharacteristicId),
    ResourceUnits(ResourceClassId),
    ComputeServiceGuarantee(ResourceClassId),
    ComputeHasPerformanceClass {
        resource_class_id: ResourceClassId,
        performance_class_id: conduit_core::ComputePerformanceClassId,
    },
    ComputePerformanceClass {
        resource_class_id: ResourceClassId,
        topology_group_id: ComputeTopologyGroupId,
    },
    ComputeNominalClockHz {
        resource_class_id: ResourceClassId,
        topology_group_id: ComputeTopologyGroupId,
    },
    OfferQueueItems,
    OfferQueueBytes,
    RequiresAuthority(AuthorityContractId),
    RequiresHostOperation(HostOperationContractId),
    ObservationUnreservedUnits(ResourceClassId),
    ObservationUtilizedUnits(ResourceClassId),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlannerFactValue {
    Boolean(bool),
    Quantity {
        value: u64,
        unit: CharacteristicUnit,
    },
    Category(String),
    ServiceGuarantee(ComputeServiceGuarantee),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerPredicate {
    Equal {
        fact: PlannerFactRef,
        value: PlannerFactValue,
    },
    NotEqual {
        fact: PlannerFactRef,
        value: PlannerFactValue,
    },
    AtLeast {
        fact: PlannerFactRef,
        value: PlannerFactValue,
    },
    AtMost {
        fact: PlannerFactRef,
        value: PlannerFactValue,
    },
    In {
        fact: PlannerFactRef,
        values: Vec<PlannerFactValue>,
    },
    Absent {
        fact: PlannerFactRef,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerPreference {
    PreferEqual {
        fact: PlannerFactRef,
        value: PlannerFactValue,
    },
    Minimize {
        fact: PlannerFactRef,
    },
    Maximize {
        fact: PlannerFactRef,
    },
    PreferOrder {
        fact: PlannerFactRef,
        values: Vec<PlannerFactValue>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FactPolicyError {
    EmptyIdentity,
    EmptyValues,
    DuplicateValue,
    ContradictoryPredicate,
    TypeMismatch,
    UnitMismatch,
    UnorderedMagnitude,
    ObservationUnavailable,
}

pub(crate) struct CandidateFacts<'a> {
    pub host: &'a HostAdvertisement,
    pub offer: &'a CapabilityOffer,
    pub realization: Option<&'a RealizationAdvertisement>,
    pub observations: &'a [ResourceObservation],
}

impl PlannerPredicate {
    pub fn fact(&self) -> &PlannerFactRef {
        match self {
            Self::Equal { fact, .. }
            | Self::NotEqual { fact, .. }
            | Self::AtLeast { fact, .. }
            | Self::AtMost { fact, .. }
            | Self::In { fact, .. }
            | Self::Absent { fact } => fact,
        }
    }
}

impl PlannerPreference {
    pub fn fact(&self) -> &PlannerFactRef {
        match self {
            Self::PreferEqual { fact, .. }
            | Self::Minimize { fact }
            | Self::Maximize { fact }
            | Self::PreferOrder { fact, .. } => fact,
        }
    }
}

pub(crate) fn predicate_matches(
    candidate: &CandidateFacts<'_>,
    predicate: &PlannerPredicate,
) -> Result<bool, FactPolicyError> {
    let actual = fact_value(candidate, predicate.fact())?;
    match predicate {
        PlannerPredicate::Absent { .. } => Ok(actual.is_none()),
        PlannerPredicate::Equal { value, .. } => {
            compare_optional(actual, value, false).map(|ordering| ordering == Some(Ordering::Equal))
        }
        PlannerPredicate::NotEqual { value, .. } => compare_optional(actual, value, false)
            .map(|ordering| ordering.is_some_and(|value| value != Ordering::Equal)),
        PlannerPredicate::AtLeast { value, .. } => compare_optional(actual, value, true)
            .map(|ordering| ordering.is_some_and(|value| value != Ordering::Less)),
        PlannerPredicate::AtMost { value, .. } => compare_optional(actual, value, true)
            .map(|ordering| ordering.is_some_and(|value| value != Ordering::Greater)),
        PlannerPredicate::In { values, .. } => match actual {
            None => Ok(false),
            Some(actual) => {
                for expected in values {
                    ensure_compatible(&actual, expected, false)?;
                }
                Ok(values.contains(&actual))
            }
        },
    }
}

pub(crate) fn compare_preference(
    left: &CandidateFacts<'_>,
    right: &CandidateFacts<'_>,
    preference: &PlannerPreference,
) -> Result<Ordering, FactPolicyError> {
    validate_preference(preference)?;
    let left = fact_value(left, preference.fact())?;
    let right = fact_value(right, preference.fact())?;
    match preference {
        PlannerPreference::PreferEqual { value, .. } => {
            Ok(preference_distance(left, value)?.cmp(&preference_distance(right, value)?))
        }
        PlannerPreference::Minimize { .. } => compare_known(left, right, false),
        PlannerPreference::Maximize { .. } => compare_known(right, left, false),
        PlannerPreference::PreferOrder { values, .. } => {
            Ok(order_distance(left, values)?.cmp(&order_distance(right, values)?))
        }
    }
}

fn preference_distance(
    actual: Option<PlannerFactValue>,
    expected: &PlannerFactValue,
) -> Result<usize, FactPolicyError> {
    match actual {
        None => Ok(2),
        Some(actual) => {
            ensure_compatible(&actual, expected, false)?;
            Ok(usize::from(actual != *expected))
        }
    }
}

fn order_distance(
    actual: Option<PlannerFactValue>,
    values: &[PlannerFactValue],
) -> Result<usize, FactPolicyError> {
    validation::validate_values(values)?;
    match actual {
        None => Ok(values.len() + 1),
        Some(actual) => {
            for expected in values {
                ensure_compatible(&actual, expected, false)?;
            }
            Ok(values
                .iter()
                .position(|expected| expected == &actual)
                .unwrap_or(values.len()))
        }
    }
}

fn compare_optional(
    actual: Option<PlannerFactValue>,
    expected: &PlannerFactValue,
    magnitude: bool,
) -> Result<Option<Ordering>, FactPolicyError> {
    actual
        .map(|actual| {
            ensure_compatible(&actual, expected, magnitude)?;
            Ok(actual.cmp(expected))
        })
        .transpose()
}

fn compare_known(
    left: Option<PlannerFactValue>,
    right: Option<PlannerFactValue>,
    allow_categories: bool,
) -> Result<Ordering, FactPolicyError> {
    match (left, right) {
        (Some(left), Some(right)) => {
            ensure_compatible(&left, &right, !allow_categories)?;
            Ok(left.cmp(&right))
        }
        (Some(_), None) => Ok(Ordering::Less),
        (None, Some(_)) => Ok(Ordering::Greater),
        (None, None) => Ok(Ordering::Equal),
    }
}

pub(super) fn ensure_compatible(
    left: &PlannerFactValue,
    right: &PlannerFactValue,
    magnitude: bool,
) -> Result<(), FactPolicyError> {
    match (left, right) {
        (
            PlannerFactValue::Quantity { unit: left, .. },
            PlannerFactValue::Quantity { unit: right, .. },
        ) if left != right => Err(FactPolicyError::UnitMismatch),
        (PlannerFactValue::Quantity { .. }, PlannerFactValue::Quantity { .. })
        | (PlannerFactValue::ServiceGuarantee(_), PlannerFactValue::ServiceGuarantee(_)) => Ok(()),
        (PlannerFactValue::Boolean(_), PlannerFactValue::Boolean(_))
        | (PlannerFactValue::Category(_), PlannerFactValue::Category(_))
            if !magnitude =>
        {
            Ok(())
        }
        (PlannerFactValue::Boolean(_), PlannerFactValue::Boolean(_))
        | (PlannerFactValue::Category(_), PlannerFactValue::Category(_)) => {
            Err(FactPolicyError::UnorderedMagnitude)
        }
        _ => Err(FactPolicyError::TypeMismatch),
    }
}

fn fact_value(
    candidate: &CandidateFacts<'_>,
    fact: &PlannerFactRef,
) -> Result<Option<PlannerFactValue>, FactPolicyError> {
    let value = match fact {
        PlannerFactRef::HostIdentity => Some(PlannerFactValue::Category(
            candidate.host.host_id.as_str().into(),
        )),
        PlannerFactRef::RealizationCharacteristic(id) => candidate
            .realization
            .and_then(|advertisement| {
                advertisement
                    .characteristics
                    .iter()
                    .find(|item| &item.definition.characteristic_id == id)
            })
            .map(|item| characteristic_value(&item.value)),
        PlannerFactRef::ResourceUnits(class) => Some(PlannerFactValue::Quantity {
            value: candidate
                .offer
                .resource_requirements
                .iter()
                .filter(|item| &item.class_id == class)
                .map(|item| u64::from(item.units))
                .sum(),
            unit: CharacteristicUnit::Items,
        }),
        PlannerFactRef::ComputeServiceGuarantee(class) => compute_contract(candidate, class)
            .map(|contract| PlannerFactValue::ServiceGuarantee(contract.service_guarantee)),
        PlannerFactRef::ComputeHasPerformanceClass {
            resource_class_id,
            performance_class_id,
        } => Some(PlannerFactValue::Boolean(
            compute_contract(candidate, resource_class_id).is_some_and(|compute| {
                compute
                    .topology_groups
                    .iter()
                    .any(|group| group.performance_class.as_ref() == Some(performance_class_id))
            }),
        )),
        PlannerFactRef::ComputePerformanceClass {
            resource_class_id,
            topology_group_id,
        } => compute_group(candidate, resource_class_id, topology_group_id)
            .and_then(|group| group.performance_class.as_ref())
            .map(|id| PlannerFactValue::Category(id.as_str().into())),
        PlannerFactRef::ComputeNominalClockHz {
            resource_class_id,
            topology_group_id,
        } => compute_group(candidate, resource_class_id, topology_group_id)
            .and_then(|group| group.nominal_clock_hz)
            .map(|value| PlannerFactValue::Quantity {
                value,
                unit: CharacteristicUnit::Hertz,
            }),
        PlannerFactRef::OfferQueueItems => Some(PlannerFactValue::Quantity {
            value: u64::from(candidate.offer.limits.max_queue_items),
            unit: CharacteristicUnit::Items,
        }),
        PlannerFactRef::OfferQueueBytes => Some(PlannerFactValue::Quantity {
            value: u64::from(candidate.offer.limits.max_queue_bytes),
            unit: CharacteristicUnit::Bytes,
        }),
        PlannerFactRef::RequiresAuthority(contract) => Some(PlannerFactValue::Boolean(
            candidate
                .offer
                .authority_requirements
                .iter()
                .any(|item| &item.contract_id == contract),
        )),
        PlannerFactRef::RequiresHostOperation(contract) => Some(PlannerFactValue::Boolean(
            candidate
                .offer
                .host_operations
                .iter()
                .any(|item| &item.contract_id == contract),
        )),
        PlannerFactRef::ObservationUnreservedUnits(class) => {
            observation_quantity(candidate, class, |item| item.unreserved_units)?
        }
        PlannerFactRef::ObservationUtilizedUnits(class) => {
            observation_quantity(candidate, class, |item| item.utilized_units)?
        }
    };
    Ok(value)
}

fn characteristic_value(value: &CharacteristicValue) -> PlannerFactValue {
    match value {
        CharacteristicValue::Boolean(value) => PlannerFactValue::Boolean(*value),
        CharacteristicValue::UnsignedQuantity { value, unit } => PlannerFactValue::Quantity {
            value: *value,
            unit: *unit,
        },
        CharacteristicValue::Categorical(value) => PlannerFactValue::Category(value.clone()),
    }
}

fn compute_contract<'a>(
    candidate: &'a CandidateFacts<'_>,
    class: &ResourceClassId,
) -> Option<&'a conduit_core::ComputePoolContract> {
    candidate
        .host
        .resources
        .iter()
        .find(|offer| &offer.class_id == class)
        .and_then(|offer| offer.compute.as_ref())
}

fn compute_group<'a>(
    candidate: &'a CandidateFacts<'_>,
    class: &ResourceClassId,
    group_id: &ComputeTopologyGroupId,
) -> Option<&'a conduit_core::ComputeTopologyGroup> {
    compute_contract(candidate, class)?
        .topology_groups
        .iter()
        .find(|group| &group.group_id == group_id)
}

fn observation_quantity(
    candidate: &CandidateFacts<'_>,
    class: &ResourceClassId,
    read: impl Fn(&ResourceObservation) -> u32,
) -> Result<Option<PlannerFactValue>, FactPolicyError> {
    let mut found = false;
    let value = candidate
        .observations
        .iter()
        .filter(|item| item.host_id == candidate.host.host_id && &item.class_id == class)
        .map(|item| {
            found = true;
            u64::from(read(item))
        })
        .sum();
    if candidate.observations.is_empty() {
        return Err(FactPolicyError::ObservationUnavailable);
    }
    Ok(found.then_some(PlannerFactValue::Quantity {
        value,
        unit: CharacteristicUnit::Items,
    }))
}
