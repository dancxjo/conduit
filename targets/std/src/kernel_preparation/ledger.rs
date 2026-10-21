//! Initialized resource ledger state, current observations and release.
use super::*;

#[derive(Debug, Clone)]
pub(crate) struct KernelResourceLedger {
    pub(super) identity: (
        conduit_core::HostId,
        conduit_core::BootId,
        conduit_core::OfferGeneration,
    ),
    pub(super) pools: Vec<PoolUsage>,
    pub(super) instances: Vec<(conduit_core::CapabilityId, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KernelResourceReservation {
    pub(super) plan_id: PlanId,
    pub(super) bindings: Vec<ResourceBinding>,
    pub(super) instances: Vec<(conduit_core::CapabilityId, u16)>,
}

impl KernelResourceLedger {
    pub(crate) fn is_idle(&self) -> bool {
        self.pools.iter().all(|pool| pool.used_units == 0)
            && self.instances.iter().all(|(_, active)| *active == 0)
    }

    pub(crate) fn new(advertisement: &HostAdvertisement) -> Result<Self, String> {
        if advertisement.protocol_version != PROTOCOL_VERSION {
            return Err("kernel host advertisement uses the wrong protocol version".to_string());
        }
        let mut pools = Vec::with_capacity(advertisement.resources.len());
        for offer in &advertisement.resources {
            if pools
                .iter()
                .any(|pool: &PoolUsage| pool.pool_id == offer.pool_id)
            {
                return Err("kernel host resource offers are malformed".into());
            }
            pools.push(PoolUsage::new(offer)?);
        }
        let mut instances = Vec::with_capacity(advertisement.capabilities.len());
        for capability in &advertisement.capabilities {
            if instances
                .iter()
                .any(|(id, _)| id == &capability.capability_id)
            {
                return Err("kernel host capability identities are duplicated".into());
            }
            instances.push((capability.capability_id.clone(), 0));
        }
        Ok(Self {
            identity: (
                advertisement.host_id.clone(),
                advertisement.boot_id.clone(),
                advertisement.offer_generation,
            ),
            pools,
            instances,
        })
    }

    pub(crate) fn release(&mut self, reservation: KernelResourceReservation) -> Result<(), String> {
        for (id, released) in &reservation.instances {
            let (_, used) = self
                .instances
                .iter_mut()
                .find(|(candidate, _)| candidate == id)
                .ok_or_else(|| "released capability is absent from the ledger".to_string())?;
            *used = used
                .checked_sub(*released)
                .ok_or_else(|| "capability release exceeded its reservation".to_string())?;
        }
        PoolUsage::release_all(&mut self.pools, &reservation.bindings)?;
        Ok(())
    }

    pub(crate) fn observe_bindings(
        &self,
        advertisement: &HostAdvertisement,
        bindings: &[ResourceBinding],
        sign_ids: &[SignId],
    ) -> Result<Vec<ResourceObservation>, String> {
        self.observe_pools(
            advertisement,
            bindings.iter().map(|b| (&b.pool_id, &b.class_id)),
            sign_ids,
        )
    }

    pub(crate) fn observe_offers(
        &self,
        advertisement: &HostAdvertisement,
        offers: &[conduit_core::ResourceOffer],
        sign_ids: &[SignId],
    ) -> Result<Vec<ResourceObservation>, String> {
        if self.identity
            != (
                advertisement.host_id.clone(),
                advertisement.boot_id.clone(),
                advertisement.offer_generation,
            )
        {
            return Err("resource observation targets a stale initialized Host ledger".into());
        }
        self.observe_pools(
            advertisement,
            offers.iter().map(|o| (&o.pool_id, &o.class_id)),
            sign_ids,
        )
    }

    fn observe_pools<'a>(
        &self,
        advertisement: &HostAdvertisement,
        pools: impl ExactSizeIterator<
            Item = (
                &'a conduit_core::ResourcePoolId,
                &'a conduit_core::ResourceClassId,
            ),
        >,
        sign_ids: &[SignId],
    ) -> Result<Vec<ResourceObservation>, String> {
        if pools.len() != sign_ids.len() {
            return Err(
                "resource observation Sign width does not match the sealed bindings".into(),
            );
        }
        let mut observations = Vec::with_capacity(pools.len());
        for ((pool_id, class_id), sign_id) in pools.zip(sign_ids) {
            if sign_id.as_str().is_empty() {
                return Err("resource observation Sign identity is empty".into());
            }
            let pool = self
                .pools
                .iter()
                .find(|pool| &pool.pool_id == pool_id && &pool.class_id == class_id)
                .ok_or_else(|| {
                    format!(
                        "sealed resource pool '{}' is absent from the current host ledger",
                        pool_id.as_str()
                    )
                })?;
            observations.push(ResourceObservation {
                host_id: advertisement.host_id.clone(),
                boot_id: advertisement.boot_id.clone(),
                offer_generation: advertisement.offer_generation,
                pool_id: pool.pool_id.clone(),
                class_id: pool.class_id.clone(),
                health: ResourceHealth::Ready,
                unreserved_units: pool.capacity_units - pool.used_units,
                utilized_units: pool.used_units,
                sign_id: sign_id.clone(),
            });
        }
        Ok(observations)
    }
}
