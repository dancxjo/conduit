//! Copyless scoped idle inventory delta. Existing pool/instance objects and
//! their exact accounting remain owned by the original ledger throughout.
use super::*;
use conduit_core::{BaseProviderEntry, CapabilityId, OfferGeneration, ResourcePoolId};
pub(crate) struct PreparedStorageLedgerRefresh {
    old_pools: Vec<ResourcePoolId>,
    old_instances: Vec<CapabilityId>,
    new_pools: Vec<PoolUsage>,
    new_instances: Vec<(CapabilityId, u16)>,
    generation: OfferGeneration,
    pub(crate) workspace_bytes: u64,
}
impl KernelResourceLedger {
    pub(crate) fn prepare_storage_refresh(
        &mut self,
        current: &HostAdvertisement,
        previous: Option<&BaseProviderEntry>,
        replacement: &BaseProviderEntry,
        backing: Option<&BaseBackingReservation>,
    ) -> Result<PreparedStorageLedgerRefresh, String> {
        let HostAdvertisement {
            declared_name: _,
            protocol_version,
            host_id,
            boot_id,
            offer_generation,
            profile: _,
            bases: _,
            resources: _,
            capabilities: _,
            planner_capabilities: _,
        } = current;
        if *protocol_version != PROTOCOL_VERSION
            || self.identity != (host_id.clone(), boot_id.clone(), *offer_generation)
            || !match backing {
                Some(token) => self.only_base_backing_is_reserved(token),
                None => self.is_idle(),
            }
        {
            return Err(
                "storage refresh requires exact current ledger and quiescent child Plays".into(),
            );
        }
        let generation = OfferGeneration(
            offer_generation
                .0
                .checked_add(1)
                .ok_or("offer generation exhausted")?,
        );
        let old_pools: Vec<_> = previous
            .into_iter()
            .flat_map(|b| &b.resources)
            .map(|r| r.pool_id.clone())
            .collect();
        let old_instances: Vec<_> = previous
            .into_iter()
            .flat_map(|b| &b.capabilities)
            .map(|r| r.capability_id.clone())
            .collect();
        if let Some(previous) = previous {
            if !previous
                .resources
                .iter()
                .all(|r| current.resources.iter().filter(|c| *c == r).count() == 1)
                || !previous
                    .capabilities
                    .iter()
                    .all(|r| current.capabilities.iter().filter(|c| *c == r).count() == 1)
            {
                return Err("selected prior storage inventory changed".into());
            }
        }
        if replacement.resources.iter().any(|r| {
            current
                .resources
                .iter()
                .any(|c| c.pool_id == r.pool_id && !old_pools.contains(&c.pool_id))
        }) || replacement.capabilities.iter().any(|r| {
            current.capabilities.iter().any(|c| {
                c.capability_id == r.capability_id && !old_instances.contains(&c.capability_id)
            })
        }) {
            return Err("storage inventory collides with unrelated equipment".into());
        }
        // Storage owns no compute topology or runtime-memory backing. The retained
        // Source's compute contracts, groups and reserved pool objects never clone.
        let mut new_pools = Vec::with_capacity(replacement.resources.len());
        for resource in &replacement.resources {
            if resource.compute.is_some()
                || resource.class_id.as_str() == conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS
            {
                return Err(
                    "storage delta cannot replace compute or runtime-memory custody".into(),
                );
            }
            if new_pools
                .iter()
                .any(|p: &PoolUsage| p.pool_id == resource.pool_id)
            {
                return Err("duplicate storage pool".into());
            }
            new_pools.push(PoolUsage::new(resource)?);
        }
        let new_instances: Vec<_> = replacement
            .capabilities
            .iter()
            .map(|c| (c.capability_id.clone(), 0))
            .collect();
        if new_instances
            .iter()
            .enumerate()
            .any(|(i, (id, _))| new_instances[..i].iter().any(|(other, _)| other == id))
        {
            return Err("duplicate storage capability".into());
        }
        let prior_pools_capacity = self.pools.capacity();
        let prior_instances_capacity = self.instances.capacity();
        self.pools
            .try_reserve_exact(new_pools.len().saturating_sub(old_pools.len()))
            .map_err(|_| "storage pool ledger backing unavailable")?;
        self.instances
            .try_reserve_exact(new_instances.len().saturating_sub(old_instances.len()))
            .map_err(|_| "storage instance ledger backing unavailable")?;
        let strings = old_pools.iter().map(|id| id.as_str().len()).sum::<usize>()
            + old_instances
                .iter()
                .map(|id| id.as_str().len())
                .sum::<usize>()
            + new_pools
                .iter()
                .map(|p| p.pool_id.as_str().len() + p.class_id.as_str().len())
                .sum::<usize>()
            + new_instances
                .iter()
                .map(|(id, _)| id.as_str().len())
                .sum::<usize>();
        let arrays = old_pools.capacity() * std::mem::size_of::<ResourcePoolId>()
            + old_instances.capacity() * std::mem::size_of::<CapabilityId>()
            + new_pools.capacity() * std::mem::size_of::<PoolUsage>()
            + new_instances.capacity() * std::mem::size_of::<(CapabilityId, u16)>();
        let growth = if self.pools.capacity() > prior_pools_capacity {
            (self.pools.capacity() + prior_pools_capacity) * std::mem::size_of::<PoolUsage>()
        } else {
            0
        } + if self.instances.capacity() > prior_instances_capacity {
            (self.instances.capacity() + prior_instances_capacity)
                * std::mem::size_of::<(CapabilityId, u16)>()
        } else {
            0
        };
        Ok(PreparedStorageLedgerRefresh {
            old_pools,
            old_instances,
            new_pools,
            new_instances,
            generation,
            workspace_bytes: (std::mem::size_of::<PreparedStorageLedgerRefresh>()
                + strings
                + arrays
                + growth) as u64,
        })
    }
    pub(crate) fn apply_storage_refresh(&mut self, prepared: PreparedStorageLedgerRefresh) {
        self.pools
            .retain(|p| !prepared.old_pools.contains(&p.pool_id));
        self.pools.extend(prepared.new_pools);
        self.instances
            .retain(|(id, _)| !prepared.old_instances.contains(id));
        self.instances.extend(prepared.new_instances);
        self.identity.2 = prepared.generation;
    }
}
