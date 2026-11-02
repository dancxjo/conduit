//! Provider-owned accounting for finite resources held outside child Plays.
//! Reservation is neither a Line offer nor permission for a platform effect.
use super::{pool_usage::PoolUsage, KernelResourceLedger};
use conduit_core::{BaseInstanceId, BaseLifecycle, ResourceBinding};
use conduit_core::{BaseRegistry, HostAdvertisement};

/// Affine custody of exact provider pools. Dropping this token retains its
/// charges: a lost worker is not evidence that its resources are free.
#[must_use = "retain provider custody until actual resource retirement"]
pub struct ProviderResourceReservation {
    ledger_epoch: usize,
    base: BaseInstanceId,
    generation: u64,
    bindings: Vec<ResourceBinding>,
}
impl ProviderResourceReservation {
    pub fn bindings(&self) -> &[ResourceBinding] {
        &self.bindings
    }
}
impl KernelResourceLedger {
    /// Reserve the selected initialized provider's exact simple-unit pools.
    /// The trusted provider supplies these requirements independently of any
    /// received Plan. At most sixteen bindings are prepared atomically.
    /// This issues no execution authority and performs no platform operation.
    pub(crate) fn reserve_provider_resources(
        &mut self,
        host: &HostAdvertisement,
        registry: &BaseRegistry,
        base: &BaseInstanceId,
        generation: u64,
        bindings: &[ResourceBinding],
    ) -> Result<ProviderResourceReservation, String> {
        self.require_available()?;
        if self.identity
            != (
                host.host_id.clone(),
                host.boot_id.clone(),
                host.offer_generation,
            )
        {
            return Err("provider reservation requires the exact current Host ledger".into());
        }
        if bindings.is_empty() || bindings.len() > 16 {
            return Err("provider reservation width exceeds its finite bound".into());
        }
        let provider = registry
            .entries()
            .iter()
            .find(|entry| {
                &entry.provider_instance_id == base
                    && entry.provider_generation == generation
                    && entry.lifecycle == BaseLifecycle::Ready
            })
            .ok_or("provider reservation requires a current initialized Base")?;
        if !host.bases.iter().any(|entry| {
            entry.lifecycle == BaseLifecycle::Ready
                && bindings
                    .iter()
                    .all(|binding| entry.resource_pool_ids.contains(&binding.pool_id))
                && entry.binding()
                    == conduit_core::BaseProviderBinding {
                        base_id: provider.base_id.clone(),
                        provider_instance_id: provider.provider_instance_id.clone(),
                        provider_generation: provider.provider_generation,
                        implementation_id: provider.implementation_id.clone(),
                        mechanism_family: provider.mechanism_family.clone(),
                        enforcement_class: provider.enforcement_class,
                    }
        }) {
            return Err("provider reservation Base differs from the current Host offer".into());
        }
        for (index, binding) in bindings.iter().enumerate() {
            if binding.units == 0
                || binding.protected.is_some()
                || binding.compute.is_some()
                || binding.content.is_some()
                || bindings[..index]
                    .iter()
                    .any(|prior| prior.pool_id == binding.pool_id)
            {
                return Err("provider reservation requires distinct simple-unit pools".into());
            }
            let offer = provider
                .resources
                .iter()
                .find(|offer| {
                    offer.pool_id == binding.pool_id && offer.class_id == binding.class_id
                })
                .ok_or("provider reservation pool is not independently configured")?;
            if offer.compute.is_some()
                || offer.content.is_some()
                || !host.resources.iter().any(|current| current == offer)
                || !self.pools.iter().any(|pool| pool.matches_offer(offer))
            {
                return Err("provider reservation pool differs from the initialized ledger".into());
            }
        }
        // Allocate the bounded token before charging; failed preparation leaves
        // all counters untouched, including a late shortage in another pool.
        let bindings = bindings.to_vec();
        PoolUsage::reserve_all(&mut self.pools, &bindings)?;
        Ok(ProviderResourceReservation {
            ledger_epoch: self.custody_epoch,
            base: base.clone(),
            generation,
            bindings,
        })
    }

    /// Release only after every owner using these resources has actually
    /// quiesced and retired. Refusal returns the unchanged affine token.
    ///
    #[allow(clippy::result_large_err)]
    pub(crate) fn release_provider_resources(
        &mut self,
        registry: &BaseRegistry,
        token: ProviderResourceReservation,
    ) -> Result<(), (ProviderResourceReservation, String)> {
        if token.ledger_epoch != self.custody_epoch
            || !registry.entries().iter().any(|entry| {
                entry.provider_instance_id == token.base
                    && entry.provider_generation == token.generation
            })
        {
            return Err((
                token,
                "provider retirement targets a different ledger or incarnation".into(),
            ));
        }
        if let Err(error) = PoolUsage::release_all(&mut self.pools, &token.bindings) {
            return Err((token, error));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
