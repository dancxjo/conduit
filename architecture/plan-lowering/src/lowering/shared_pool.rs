use super::{as_u16, LoweringError};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use conduit_core::{
    AdmittedLine, ArtifactId, AuthorityGrantId, BootId, CapabilityId, HostId, ImplementationId,
    OfferGeneration, PlacementId, PlanFragment, PoolMemberLimits, PoolRealizationHealth,
    PoolRealizationObservation, ResourceBinding, ResourceHealth, SharedPoolId,
    SharedPoolSelectionPolicy, SignId,
};
use conduit_kernel::{
    shared_pool::{
        LoweredObservationHealth, LoweredPoolObservation,
        LoweredPoolRealization as KernelPoolRealization, LoweredPoolResourceRequirement,
        MemberPlacement, PoolId,
    },
    NodeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPoolRealization {
    pub realization: u16,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub capability_id: CapabilityId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
    pub member_capacity: u16,
    pub resources: Vec<ResourceBinding>,
    pub admitted_lines: Vec<AdmittedLine>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSharedPool {
    pub pool: PoolId,
    pub pool_id: SharedPoolId,
    pub maximum_members: u16,
    pub member_limits: PoolMemberLimits,
    pub member_sessions_required: bool,
    pub admission_authority: AuthorityGrantId,
    pub selection_policy: SharedPoolSelectionPolicy,
    pub realizations: Vec<LoweredPoolRealization>,
    pub local_consumers: Vec<NodeId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPoolSelectionFacts {
    pub envelope: Vec<KernelPoolRealization>,
    pub requirements: Vec<LoweredPoolResourceRequirement>,
    pub observations: Vec<LoweredPoolObservation>,
    pub observation_sign_ids: Vec<SignId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolObservationLoweringError {
    CapacityOverflow,
    DuplicateCurrentObservation,
    InvalidCurrentObservation,
}

impl LoweredSharedPool {
    /// Bind rich current provider/resource observations to the exact numeric
    /// selection tables consumed by the allocation-independent kernel. Stale
    /// and unsealed observations are omitted; duplicate exact truth refuses.
    pub fn lower_selection_facts(
        &self,
        current: &[PoolRealizationObservation],
        first_member_node: NodeId,
        play: u16,
    ) -> Result<LoweredPoolSelectionFacts, PoolObservationLoweringError> {
        let mut envelope = Vec::with_capacity(self.realizations.len());
        let mut requirements = Vec::new();
        let mut observations = Vec::new();
        let mut observation_sign_ids = Vec::new();
        for realization in &self.realizations {
            let node = first_member_node
                .0
                .checked_add(realization.realization)
                .ok_or(PoolObservationLoweringError::CapacityOverflow)?;
            let token = realization.realization;
            envelope.push(KernelPoolRealization {
                realization: token,
                placement: MemberPlacement {
                    node: NodeId(node),
                    realization: token,
                    play,
                },
                boot: token,
                offer_generation: realization.offer_generation.0,
                capability: token,
                implementation: token,
                artifact: token,
                member_capacity: realization.member_capacity,
            });
            requirements.push(LoweredPoolResourceRequirement {
                realization: token,
                resource: 0,
                units: 1,
            });
            for (resource_index, binding) in realization.resources.iter().enumerate() {
                requirements.push(LoweredPoolResourceRequirement {
                    realization: token,
                    resource: u16::try_from(resource_index + 1)
                        .map_err(|_| PoolObservationLoweringError::CapacityOverflow)?,
                    units: binding.units,
                });
            }

            let mut exact = current.iter().filter(|observation| {
                observation.host_id == realization.host_id
                    && observation.boot_id == realization.boot_id
                    && observation.offer_generation == realization.offer_generation
                    && observation.capability_id == realization.capability_id
                    && observation.implementation_id == realization.implementation_id
                    && observation.artifact_id == realization.artifact_id
            });
            let Some(observation) = exact.next() else {
                continue;
            };
            if exact.next().is_some() {
                return Err(PoolObservationLoweringError::DuplicateCurrentObservation);
            }
            let planned = conduit_core::PoolRealizationEnvelope {
                host_id: realization.host_id.clone(),
                boot_id: realization.boot_id.clone(),
                offer_generation: realization.offer_generation,
                capability_id: realization.capability_id.clone(),
                implementation_id: realization.implementation_id.clone(),
                artifact_id: realization.artifact_id.clone(),
                member_capacity: realization.member_capacity,
                resources: realization.resources.clone(),
                admitted_lines: realization.admitted_lines.clone(),
            };
            if !observation.is_current_for(&planned) {
                return Err(PoolObservationLoweringError::InvalidCurrentObservation);
            }
            let provider_sign = push_sign(&mut observation_sign_ids, &observation.sign_id)?;
            observations.push(LoweredPoolObservation {
                realization: token,
                resource: 0,
                boot: token,
                offer_generation: realization.offer_generation.0,
                capability: token,
                implementation: token,
                artifact: token,
                health: lower_health(matches!(observation.health, PoolRealizationHealth::Ready)),
                unreserved_units: u32::from(observation.health == PoolRealizationHealth::Ready),
                utilized_units: 0,
                sign: provider_sign,
            });
            for (resource_index, binding) in realization.resources.iter().enumerate() {
                let resource = observation
                    .resources
                    .iter()
                    .find(|item| {
                        item.pool_id == binding.pool_id && item.class_id == binding.class_id
                    })
                    .ok_or(PoolObservationLoweringError::InvalidCurrentObservation)?;
                let sign = push_sign(&mut observation_sign_ids, &resource.sign_id)?;
                observations.push(LoweredPoolObservation {
                    realization: token,
                    resource: u16::try_from(resource_index + 1)
                        .map_err(|_| PoolObservationLoweringError::CapacityOverflow)?,
                    boot: token,
                    offer_generation: realization.offer_generation.0,
                    capability: token,
                    implementation: token,
                    artifact: token,
                    health: lower_health(matches!(resource.health, ResourceHealth::Ready)),
                    unreserved_units: resource.unreserved_units,
                    utilized_units: resource.utilized_units,
                    sign,
                });
            }
        }
        Ok(LoweredPoolSelectionFacts {
            envelope,
            requirements,
            observations,
            observation_sign_ids,
        })
    }
}

fn push_sign(signs: &mut Vec<SignId>, sign: &SignId) -> Result<u16, PoolObservationLoweringError> {
    let index =
        u16::try_from(signs.len()).map_err(|_| PoolObservationLoweringError::CapacityOverflow)?;
    signs.push(sign.clone());
    Ok(index)
}

fn lower_health(ready: bool) -> LoweredObservationHealth {
    if ready {
        LoweredObservationHealth::Ready
    } else {
        LoweredObservationHealth::Unavailable
    }
}

pub(super) fn lower_shared_pools(
    fragment: &PlanFragment,
    placement_nodes: &BTreeMap<PlacementId, NodeId>,
) -> Result<Vec<LoweredSharedPool>, LoweringError> {
    let mut pool_ids = BTreeSet::new();
    let mut shared_pools = Vec::with_capacity(fragment.shared_pools.len());
    for (pool_index, pool) in fragment.shared_pools.iter().enumerate() {
        if !pool_ids.insert(pool.pool_id.clone()) || pool.validate().is_err() {
            return Err(LoweringError::SharedPoolInvalid(pool.pool_id.clone()));
        }
        let local_consumers = pool
            .consumers
            .iter()
            .filter_map(|consumer| placement_nodes.get(consumer).copied())
            .collect::<Vec<_>>();
        for placement in &fragment.placements {
            let references_pool = placement.pool_references.contains(&pool.pool_id);
            let declared_consumer = pool.consumers.contains(&placement.placement_id);
            if references_pool != declared_consumer {
                return Err(LoweringError::SharedPoolConsumerMissing(
                    pool.pool_id.clone(),
                ));
            }
        }
        let realizations = pool
            .realization_envelope
            .iter()
            .enumerate()
            .map(|(index, realization)| {
                Ok(LoweredPoolRealization {
                    realization: as_u16(index)?,
                    host_id: realization.host_id.clone(),
                    boot_id: realization.boot_id.clone(),
                    offer_generation: realization.offer_generation,
                    capability_id: realization.capability_id.clone(),
                    implementation_id: realization.implementation_id.clone(),
                    artifact_id: realization.artifact_id.clone(),
                    member_capacity: realization.member_capacity,
                    resources: realization.resources.clone(),
                    admitted_lines: realization.admitted_lines.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        shared_pools.push(LoweredSharedPool {
            pool: PoolId(as_u16(pool_index)?),
            pool_id: pool.pool_id.clone(),
            maximum_members: pool.maximum_members,
            member_limits: pool.member_limits,
            member_sessions_required: pool.member_sessions_required,
            admission_authority: pool.admission_authority.clone(),
            selection_policy: pool.selection_policy,
            realizations,
            local_consumers,
        });
    }
    Ok(shared_pools)
}
