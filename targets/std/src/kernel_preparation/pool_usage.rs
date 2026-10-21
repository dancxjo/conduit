//! Atomic accounting for existing resource pools and their topology groups.
use conduit_core::{ComputePoolContract, ComputeTopologyGroupId, ResourceBinding, ResourceOffer};
use conduit_core::{ResourceClassId, ResourcePoolId};

#[derive(Debug, Clone, PartialEq, Eq)]
struct GroupUsage {
    group_id: ComputeTopologyGroupId,
    capacity_units: u32,
    used_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PoolUsage {
    pub(super) pool_id: ResourcePoolId,
    pub(super) class_id: ResourceClassId,
    pub(super) capacity_units: u32,
    pub(super) used_units: u32,
    compute: Option<ComputePoolContract>,
    groups: Vec<GroupUsage>,
}

impl PoolUsage {
    pub(super) fn new(offer: &ResourceOffer) -> Result<Self, String> {
        if offer.pool_id.as_str().is_empty()
            || offer.class_id.as_str().is_empty()
            || offer.capacity_units == 0
            || offer
                .compute
                .as_ref()
                .is_some_and(|c| !c.is_valid_for_capacity(offer.capacity_units))
        {
            return Err("kernel host resource offers are malformed".into());
        }
        let groups = offer
            .compute
            .iter()
            .flat_map(|c| &c.topology_groups)
            .map(|g| GroupUsage {
                group_id: g.group_id.clone(),
                capacity_units: g.lane_capacity,
                used_units: 0,
            })
            .collect();
        Ok(Self {
            pool_id: offer.pool_id.clone(),
            class_id: offer.class_id.clone(),
            capacity_units: offer.capacity_units,
            used_units: 0,
            compute: offer.compute.clone(),
            groups,
        })
    }

    pub(super) fn matches_offer(&self, offer: &ResourceOffer) -> bool {
        self.pool_id == offer.pool_id
            && self.class_id == offer.class_id
            && self.capacity_units == offer.capacity_units
            && self.compute == offer.compute
    }

    pub(super) fn copy_usage_from(&mut self, other: &Self) {
        self.used_units = other.used_units;
        for (live, staged) in self.groups.iter_mut().zip(&other.groups) {
            live.used_units = staged.used_units;
        }
    }

    fn group_units(&self, bindings: &[ResourceBinding], group: &GroupUsage) -> Result<u32, String> {
        bindings
            .iter()
            .filter(|b| {
                b.pool_id == self.pool_id
                    && b.class_id == self.class_id
                    && b.compute
                        .as_ref()
                        .is_some_and(|c| c.topology_group_id.as_ref() == Some(&group.group_id))
            })
            .try_fold(0u32, |total, b| {
                total
                    .checked_add(b.units)
                    .ok_or_else(|| "resource topology group demand overflowed".into())
            })
    }

    /// Validate every pool and group first; a late failure changes no counter.
    pub(super) fn reserve_all(
        pools: &mut [Self],
        bindings: &[ResourceBinding],
    ) -> Result<(), String> {
        for binding in bindings {
            let pool = pools
                .iter()
                .find(|p| p.pool_id == binding.pool_id && p.class_id == binding.class_id)
                .ok_or_else(|| {
                    "resource binding is absent from the initialized ledger".to_string()
                })?;
            if let Some(group_id) = binding
                .compute
                .as_ref()
                .and_then(|c| c.topology_group_id.as_ref())
            {
                if !pool.groups.iter().any(|g| &g.group_id == group_id) {
                    return Err(
                        "resource topology group is absent from the initialized ledger".into(),
                    );
                }
            }
        }
        for pool in pools.iter() {
            let total = pool
                .used_units
                .checked_add(requested_units(bindings, &pool.pool_id, &pool.class_id)?)
                .ok_or_else(|| "resource pool usage overflowed".to_string())?;
            if total > pool.capacity_units {
                return Err(format!(
                    "resource pool '{}' requires {} units above capacity {}",
                    pool.pool_id.as_str(),
                    total,
                    pool.capacity_units
                ));
            }
            for group in &pool.groups {
                let total = group
                    .used_units
                    .checked_add(pool.group_units(bindings, group)?)
                    .ok_or_else(|| "resource topology group usage overflowed".to_string())?;
                if total > group.capacity_units {
                    return Err("resource topology group capacity is unavailable".into());
                }
            }
        }
        for pool in pools {
            // All arithmetic and membership were checked above.
            for index in 0..pool.groups.len() {
                pool.groups[index].used_units += pool.group_units(bindings, &pool.groups[index])?;
            }
            pool.used_units += requested_units(bindings, &pool.pool_id, &pool.class_id)?;
        }
        Ok(())
    }

    pub(super) fn release_all(
        pools: &mut [Self],
        bindings: &[ResourceBinding],
    ) -> Result<(), String> {
        for pool in pools.iter() {
            if requested_units(bindings, &pool.pool_id, &pool.class_id)? > pool.used_units {
                return Err("resource release exceeded its reservation".into());
            }
            for group in &pool.groups {
                if pool.group_units(bindings, group)? > group.used_units {
                    return Err("topology group release exceeded its reservation".into());
                }
            }
        }
        for pool in pools {
            for index in 0..pool.groups.len() {
                pool.groups[index].used_units -= pool.group_units(bindings, &pool.groups[index])?;
            }
            pool.used_units -= requested_units(bindings, &pool.pool_id, &pool.class_id)?;
        }
        Ok(())
    }
}

pub(super) fn requested_units(
    bindings: &[ResourceBinding],
    pool_id: &ResourcePoolId,
    class_id: &ResourceClassId,
) -> Result<u32, String> {
    bindings
        .iter()
        .filter(|binding| &binding.pool_id == pool_id && &binding.class_id == class_id)
        .try_fold(0_u32, |total, binding: &ResourceBinding| {
            total
                .checked_add(binding.units)
                .ok_or_else(|| format!("resource pool '{}' usage overflowed", pool_id.as_str()))
        })
}

#[cfg(test)]
#[path = "pool_usage_tests.rs"]
mod tests;
