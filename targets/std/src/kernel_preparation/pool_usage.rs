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
    /// Heap payload of a closed non-compute pool, including allocated capacity
    /// even when the topology vector is empty. Shared provider storage is not
    /// owned by this descriptor.
    pub(super) fn closed_backing_bytes(&self) -> Option<usize> {
        if self.compute.is_some() || !self.groups.is_empty() {
            return None;
        }
        self.pool_id
            .0
            .capacity()
            .checked_add(self.class_id.0.capacity())?
            .checked_add(
                self.groups
                    .capacity()
                    .checked_mul(core::mem::size_of::<GroupUsage>())?,
            )
    }
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

impl super::KernelResourceLedger {
    /// Requested allocations owned by one closed startup reservation result.
    /// This is a partial constructor term, not an admission/authority receipt.
    /// Lowering, both staged ledger clones, exact-offer checks and error String
    /// formatting have separate owners and must be added by the constructor.
    /// This method borrows existing owners and never clones or reserves usage.
    pub(crate) fn closed_startup_reservation_request_bound(
        &self,
        placements: &[conduit_core::PlannedGear],
        plan_id: &conduit_core::PlanId,
    ) -> Option<usize> {
        use conduit_core::CapabilityId;
        use core::mem::size_of;
        if !cfg!(conduit_pinned_collection_layout) || placements.len() != 2 {
            return None;
        }
        // Reuse the existing allocation-free qualification for quarantine and
        // every live pool's unsupported compute/group ownership. Its number
        // is intentionally not added: ledger staging is a distinct term.
        self.closed_clone_request_bound()?;
        let source = placements.iter().find(|p| {
            p.implementation_id.as_str() == conduit_std_offers::BODY_WAKE_IMPLEMENTATION
                && p.capability_id.as_str() == conduit_std_offers::BODY_WAKE_CAPABILITY
        })?;
        let sink = placements.iter().find(|p| {
            p.implementation_id.as_str() == conduit_std_offers::STARTUP_CHIME_IMPLEMENTATION
                && p.capability_id.as_str() == conduit_std_offers::STARTUP_CHIME_CAPABILITY
        })?;
        if !source.resources.is_empty() || sink.resources.len() != 1 {
            return None;
        }
        let binding = &sink.resources[0];
        if binding.class_id.as_str() != conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS
            || binding.units != 1
            || binding.protected.is_some()
            || binding.compute.is_some()
            || binding.content.is_some()
            || !self
                .pools
                .iter()
                .any(|p| p.pool_id == binding.pool_id && p.class_id == binding.class_id)
        {
            return None;
        }
        // placement_resources.rs visits ledger rows, not placement rows. Prove
        // exactly two rows will be pushed even if handed a malformed ledger.
        for selected in [&source.capability_id, &sink.capability_id] {
            if self
                .instances
                .iter()
                .filter(|(id, _)| id == selected)
                .count()
                != 1
            {
                return None;
            }
        }
        // prepare_and_reserve_partitions with exactly one borrowed partition:
        // Vec::with_capacity(1), then its popped result is moved, not cloned.
        let mut bytes = size_of::<super::KernelResourceReservation>();
        // placement_resources.rs:14 starts Vec::new and pushes two instance
        // rows. Pinned raw_vec.rs mincap is4 for this String/u16 tuple; assert
        // the type range rather than silently transplanting a host layout.
        let row = size_of::<(CapabilityId, u16)>();
        if !(2..=1024).contains(&row) {
            return None;
        }
        bytes = bytes.checked_add(4usize.checked_mul(row)?)?;
        bytes = bytes.checked_add(source.capability_id.as_str().len())?;
        bytes = bytes.checked_add(sink.capability_id.as_str().len())?;
        // Lowered one-resource table -> Vec::with_capacity(1), semantic.clone.
        // Plain ResourceBinding has exactly these two owned String leaves.
        bytes = bytes.checked_add(size_of::<ResourceBinding>())?;
        bytes = bytes.checked_add(binding.pool_id.as_str().len())?;
        bytes = bytes.checked_add(binding.class_id.as_str().len())?;
        bytes = bytes.checked_add(plan_id.as_str().len())?;
        // Every clone is a length-sized slice allocation on the qualified
        // library. The sum dominates every individual RawVec request.
        (bytes <= isize::MAX as usize).then_some(bytes)
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
