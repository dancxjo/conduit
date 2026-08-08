use crate::requirements::{
    hard_requirement_failure, has_characteristic_requirements, validate_requirement_identities,
    HardRealizationRequirements,
};
use crate::{PlacementChoice, PlannerError};
use conduit_core::{
    AuthorityContractId, CapabilityOffer, ComputePerformanceClassId, HostAdvertisement,
    HostOperationContractId, RealizationCharacteristicId, ResourceClassId,
};
use conduit_form::CheckedOperation;
use std::cmp::Ordering;

/// One explicit lexicographic comparison dimension.
///
/// Hosts advertise facts; the caller supplies their ordering. No variant is a
/// universal host-provided score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RealizationPreference {
    MinimizeResourceUnits(ResourceClassId),
    MaximizeComputeServiceGuarantee(ResourceClassId),
    PreferComputePerformanceClass {
        resource_class_id: ResourceClassId,
        performance_class_id: ComputePerformanceClassId,
    },
    MaximizeQueueItems,
    MaximizeQueueBytes,
    PreferWithoutHostOperation(HostOperationContractId),
    PreferWithoutAuthority(AuthorityContractId),
    MinimizeCharacteristicCount(RealizationCharacteristicId),
    MaximizeCharacteristicCount(RealizationCharacteristicId),
    PreferCharacteristicFlag {
        characteristic_id: RealizationCharacteristicId,
        value: bool,
    },
}

/// Ordered comparison dimensions. Earlier entries take precedence.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RealizationPolicy {
    pub preferences: Vec<RealizationPreference>,
}

pub fn select_realization_with_policy(
    operation: &CheckedOperation,
    realm: &[HostAdvertisement],
    requirements: &HardRealizationRequirements,
    policy: &RealizationPolicy,
) -> Result<PlacementChoice, PlannerError> {
    select_realization_matching(operation, realm, requirements, policy, |_, _| true, None)
}

pub(crate) fn select_realization_matching(
    operation: &CheckedOperation,
    realm: &[HostAdvertisement],
    requirements: &HardRealizationRequirements,
    policy: &RealizationPolicy,
    mut currently_admissible: impl FnMut(&HostAdvertisement, &CapabilityOffer) -> bool,
    current_refusal: Option<PlannerError>,
) -> Result<PlacementChoice, PlannerError> {
    validate_requirement_identities(requirements)?;
    validate_policy(policy)?;
    if has_characteristic_requirements(requirements) {
        return Err(PlannerError::InvalidHardRealizationRequirement(
            "characteristic requirements require exact realization advertisements".to_string(),
        ));
    }
    if policy.preferences.iter().any(|preference| {
        matches!(
            preference,
            RealizationPreference::MinimizeCharacteristicCount(_)
                | RealizationPreference::MaximizeCharacteristicCount(_)
                | RealizationPreference::PreferCharacteristicFlag { .. }
        )
    }) {
        return Err(PlannerError::InvalidRealizationPolicy(
            "characteristic policy requires exact realization advertisements".to_string(),
        ));
    }

    let mut face_candidates = Vec::new();
    for host in realm {
        for offer in &host.capabilities {
            if offer.checked_face() == operation.checked_face() {
                face_candidates.push(Candidate { host, offer });
            }
        }
    }
    if face_candidates.is_empty() {
        return Err(PlannerError::UnknownCapability(
            operation.kind_id.as_str().to_string(),
        ));
    }

    let mut admitted = face_candidates
        .into_iter()
        .filter(|candidate| hard_requirement_failure(candidate.offer, requirements).is_none())
        .collect::<Vec<_>>();
    if admitted.is_empty() {
        return Err(PlannerError::HardRealizationRequirementUnsatisfied(
            format!(
                "operation '{}' has no hard-admissible realization",
                operation.operation_id.as_str()
            ),
        ));
    }
    admitted.retain(|candidate| currently_admissible(candidate.host, candidate.offer));
    if admitted.is_empty() {
        return Err(current_refusal.unwrap_or_else(|| {
            PlannerError::CurrentResourceObservationUnavailable(format!(
                "operation '{}' has no currently admissible realization",
                operation.operation_id.as_str()
            ))
        }));
    }
    admitted.sort_by(|left, right| compare_candidates(left, right, policy));
    let selected = admitted[0];
    Ok(PlacementChoice {
        host_id: selected.host.host_id.clone(),
        capability_id: selected.offer.capability_id.clone(),
    })
}

#[derive(Clone, Copy)]
struct Candidate<'a> {
    host: &'a HostAdvertisement,
    offer: &'a CapabilityOffer,
}

fn compare_candidates(
    left: &Candidate<'_>,
    right: &Candidate<'_>,
    policy: &RealizationPolicy,
) -> Ordering {
    for preference in &policy.preferences {
        let ordering = match preference {
            RealizationPreference::MinimizeResourceUnits(class_id) => {
                resource_units(left.offer, class_id).cmp(&resource_units(right.offer, class_id))
            }
            RealizationPreference::MaximizeComputeServiceGuarantee(class_id) => {
                compute_service(right.host, class_id).cmp(&compute_service(left.host, class_id))
            }
            RealizationPreference::PreferComputePerformanceClass {
                resource_class_id,
                performance_class_id,
            } => compute_performance_distance(left.host, resource_class_id, performance_class_id)
                .cmp(&compute_performance_distance(
                    right.host,
                    resource_class_id,
                    performance_class_id,
                )),
            RealizationPreference::MaximizeQueueItems => right
                .offer
                .limits
                .max_queue_items
                .cmp(&left.offer.limits.max_queue_items),
            RealizationPreference::MaximizeQueueBytes => right
                .offer
                .limits
                .max_queue_bytes
                .cmp(&left.offer.limits.max_queue_bytes),
            RealizationPreference::PreferWithoutHostOperation(contract_id) => {
                has_host_operation(left.offer, contract_id)
                    .cmp(&has_host_operation(right.offer, contract_id))
            }
            RealizationPreference::PreferWithoutAuthority(contract_id) => {
                has_authority(left.offer, contract_id).cmp(&has_authority(right.offer, contract_id))
            }
            RealizationPreference::MinimizeCharacteristicCount(_)
            | RealizationPreference::MaximizeCharacteristicCount(_)
            | RealizationPreference::PreferCharacteristicFlag { .. } => Ordering::Equal,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
    left.host
        .host_id
        .cmp(&right.host.host_id)
        .then_with(|| left.offer.capability_id.cmp(&right.offer.capability_id))
}

fn resource_units(offer: &CapabilityOffer, class_id: &ResourceClassId) -> u64 {
    offer
        .resource_requirements
        .iter()
        .filter(|requirement| &requirement.class_id == class_id)
        .map(|requirement| u64::from(requirement.units))
        .sum()
}

fn has_host_operation(offer: &CapabilityOffer, contract_id: &HostOperationContractId) -> bool {
    offer
        .host_operations
        .iter()
        .any(|requirement| &requirement.contract_id == contract_id)
}

fn has_authority(offer: &CapabilityOffer, contract_id: &AuthorityContractId) -> bool {
    offer
        .authority_requirements
        .iter()
        .any(|requirement| &requirement.contract_id == contract_id)
}

fn compute_service(
    host: &HostAdvertisement,
    class_id: &ResourceClassId,
) -> Option<conduit_core::ComputeServiceGuarantee> {
    host.resources
        .iter()
        .find(|offer| &offer.class_id == class_id)
        .and_then(|offer| offer.compute.as_ref())
        .map(|compute| compute.service_guarantee)
}

fn compute_performance_distance(
    host: &HostAdvertisement,
    class_id: &ResourceClassId,
    performance_class_id: &ComputePerformanceClassId,
) -> u8 {
    u8::from(!host.resources.iter().any(|offer| {
        &offer.class_id == class_id
            && offer.compute.as_ref().is_some_and(|compute| {
                compute
                    .topology_groups
                    .iter()
                    .any(|group| group.performance_class.as_ref() == Some(performance_class_id))
            })
    }))
}

pub(crate) fn validate_policy(policy: &RealizationPolicy) -> Result<(), PlannerError> {
    let has_empty_identity = policy
        .preferences
        .iter()
        .any(|preference| match preference {
            RealizationPreference::MinimizeResourceUnits(identity) => identity.as_str().is_empty(),
            RealizationPreference::MaximizeComputeServiceGuarantee(identity) => {
                identity.as_str().is_empty()
            }
            RealizationPreference::PreferComputePerformanceClass {
                resource_class_id,
                performance_class_id,
            } => resource_class_id.as_str().is_empty() || performance_class_id.as_str().is_empty(),
            RealizationPreference::PreferWithoutHostOperation(identity) => {
                identity.as_str().is_empty()
            }
            RealizationPreference::PreferWithoutAuthority(identity) => identity.as_str().is_empty(),
            RealizationPreference::MinimizeCharacteristicCount(identity)
            | RealizationPreference::MaximizeCharacteristicCount(identity) => {
                identity.as_str().is_empty()
            }
            RealizationPreference::PreferCharacteristicFlag {
                characteristic_id, ..
            } => characteristic_id.as_str().is_empty(),
            RealizationPreference::MaximizeQueueItems
            | RealizationPreference::MaximizeQueueBytes => false,
        });
    if has_empty_identity {
        return Err(PlannerError::InvalidRealizationPolicy(
            "policy identities must be non-empty".to_string(),
        ));
    }
    Ok(())
}
