use super::{as_u16, LoweringError};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use conduit_core::{
    AuthorityGrantId, BootId, CapabilityId, HostId, PlacementId, PlanFragment, PoolMemberLimits,
    ResourceBinding, SharedPoolId,
};
use conduit_kernel::{shared_pool::PoolId, NodeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredPoolRealization {
    pub realization: u16,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub member_capacity: u16,
    pub resources: Vec<ResourceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredSharedPool {
    pub pool: PoolId,
    pub pool_id: SharedPoolId,
    pub maximum_members: u16,
    pub member_limits: PoolMemberLimits,
    pub admission_authority: AuthorityGrantId,
    pub realizations: Vec<LoweredPoolRealization>,
    pub local_consumers: Vec<NodeId>,
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
                    capability_id: realization.capability_id.clone(),
                    member_capacity: realization.member_capacity,
                    resources: realization.resources.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        shared_pools.push(LoweredSharedPool {
            pool: PoolId(as_u16(pool_index)?),
            pool_id: pool.pool_id.clone(),
            maximum_members: pool.maximum_members,
            member_limits: pool.member_limits,
            admission_authority: pool.admission_authority.clone(),
            realizations,
            local_consumers,
        });
    }
    Ok(shared_pools)
}
