use crate::{RealizationPolicy, RealizationPreference};
use conduit_core::{
    CapabilityOffer, HostAdvertisement, RealizationAdvertisement, RealizationCharacteristicId,
    RealizationCharacteristicValue,
};
use std::cmp::Ordering;

#[allow(clippy::too_many_arguments)]
pub(crate) fn compare(
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
            RealizationPreference::MaximizeComputeServiceGuarantee(class) => {
                compute_service(right_host, class).cmp(&compute_service(left_host, class))
            }
            RealizationPreference::PreferComputePerformanceClass {
                resource_class_id,
                performance_class_id,
            } => compute_performance_distance(left_host, resource_class_id, performance_class_id)
                .cmp(&compute_performance_distance(
                    right_host,
                    resource_class_id,
                    performance_class_id,
                )),
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

fn compute_service(
    host: &HostAdvertisement,
    class: &conduit_core::ResourceClassId,
) -> Option<conduit_core::ComputeServiceGuarantee> {
    host.resources
        .iter()
        .find(|offer| &offer.class_id == class)
        .and_then(|offer| offer.compute.as_ref())
        .map(|compute| compute.service_guarantee)
}

fn compute_performance_distance(
    host: &HostAdvertisement,
    class: &conduit_core::ResourceClassId,
    performance: &conduit_core::ComputePerformanceClassId,
) -> u8 {
    u8::from(!host.resources.iter().any(|offer| {
        &offer.class_id == class
            && offer.compute.as_ref().is_some_and(|compute| {
                compute
                    .topology_groups
                    .iter()
                    .any(|group| group.performance_class.as_ref() == Some(performance))
            })
    }))
}
