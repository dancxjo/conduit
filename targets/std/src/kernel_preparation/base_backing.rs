//! Base-owned backing is reserved before initialization and outlives child Plays.
//! This token is not a Plan, a grant, or permission to perform a Host Call.
use super::{pool_usage::PoolUsage, KernelResourceLedger};
use conduit_core::{
    BaseInstanceId, BaseLifecycle, BaseRegistry, HostAdvertisement, ResourceBinding,
    ResourcePoolId, RUNTIME_MEMORY_RESOURCE_CLASS,
};

/// Affine accounting custody. Dropping a token never makes live backing free.
/// The native owner must first establish actual quiescence and retirement.
pub(crate) struct BaseBackingReservation {
    identity: (
        conduit_core::HostId,
        conduit_core::BootId,
        conduit_core::OfferGeneration,
    ),
    base: BaseInstanceId,
    generation: u64,
    binding: ResourceBinding,
}
impl BaseBackingReservation {
    pub(crate) fn bytes(&self) -> u32 {
        self.binding.units
    }
    pub(crate) fn adopt_refreshed_ledger(&mut self, ledger: &KernelResourceLedger) {
        self.identity = ledger.identity.clone();
    }
}
impl KernelResourceLedger {
    /// Add capability offers between child Plays without retiring or freeing
    /// the actual Base owner. Existing resources/providers must be unchanged.
    pub(crate) fn prepare_capability_refresh(
        &self,
        current: &HostAdvertisement,
        next: &HostAdvertisement,
        backing: Option<&BaseBackingReservation>,
    ) -> Result<Self, String> {
        if self.identity
            != (
                current.host_id.clone(),
                current.boot_id.clone(),
                current.offer_generation,
            )
            || next.host_id != current.host_id
            || next.boot_id != current.boot_id
            || next.offer_generation.0
                != current
                    .offer_generation
                    .0
                    .checked_add(1)
                    .ok_or("offer generation exhausted")?
            || next.resources != current.resources
            || next.bases != current.bases
            || !current
                .capabilities
                .iter()
                .all(|offer| next.capabilities.contains(offer))
            || !match backing {
                Some(token) => self.only_base_backing_is_reserved(token),
                None => self.is_idle(),
            }
        {
            return Err(
                "capability refresh requires unchanged Base resources and quiescent child Plays"
                    .into(),
            );
        }
        let mut staged = Self::new(next)?;
        if let Some(token) = backing {
            // Transfer accounting into the staged generation, not a second
            // owner/token and not a release/reacquire of physical backing.
            PoolUsage::reserve_all(&mut staged.pools, core::slice::from_ref(&token.binding))?;
        }
        Ok(staged)
    }
    /// The native Root supplies an independently configured current pool and
    /// finite backing size. A received Plan cannot provide this expectation.
    pub(crate) fn reserve_base_backing(
        &mut self,
        host: &HostAdvertisement,
        registry: &BaseRegistry,
        base: &BaseInstanceId,
        generation: u64,
        pool: &ResourcePoolId,
        bytes: u32,
    ) -> Result<BaseBackingReservation, String> {
        if bytes == 0
            || self.identity
                != (
                    host.host_id.clone(),
                    host.boot_id.clone(),
                    host.offer_generation,
                )
        {
            return Err("Base backing requires the exact initialized Host ledger".into());
        }
        let provider = registry
            .entries()
            .iter()
            .find(|entry| {
                &entry.provider_instance_id == base
                    && entry.provider_generation == generation
                    && entry.lifecycle == BaseLifecycle::Ready
            })
            .ok_or("Base backing provider is not current and ready")?;
        if !host.bases.iter().any(|entry| {
            entry.binding()
                == conduit_core::BaseProviderBinding {
                    base_id: provider.base_id.clone(),
                    provider_instance_id: provider.provider_instance_id.clone(),
                    provider_generation: provider.provider_generation,
                    implementation_id: provider.implementation_id.clone(),
                    mechanism_family: provider.mechanism_family.clone(),
                    enforcement_class: provider.enforcement_class,
                }
        }) {
            return Err("Base backing provider is absent from the staged current offer".into());
        }
        let offer = provider
            .resources
            .iter()
            .find(|offer| {
                &offer.pool_id == pool && offer.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS
            })
            .ok_or("Base backing pool is not independently configured on this provider")?;
        if !host.resources.iter().any(|current| current == offer)
            || !self
                .pools
                .iter()
                .any(|current| current.matches_offer(offer))
            || offer.compute.is_some()
            || offer.content.is_some()
        {
            return Err("Base backing pool differs from the initialized resource ledger".into());
        }
        let binding = ResourceBinding {
            pool_id: pool.clone(),
            class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
            units: bytes,
            protected: None,
            compute: None,
            content: None,
        };
        PoolUsage::reserve_all(&mut self.pools, core::slice::from_ref(&binding))?;
        Ok(BaseBackingReservation {
            identity: self.identity.clone(),
            base: base.clone(),
            generation,
            binding,
        })
    }
    pub(crate) fn only_base_backing_is_reserved(&self, token: &BaseBackingReservation) -> bool {
        token.identity == self.identity
            && self.instances.iter().all(|(_, used)| *used == 0)
            && self.pools.iter().all(|pool| {
                pool.used_units
                    == if pool.pool_id == token.binding.pool_id
                        && pool.class_id == token.binding.class_id
                    {
                        token.binding.units
                    } else {
                        0
                    }
            })
    }
    /// Call only after the actual native owner has joined/retired. Refusal
    /// returns the unchanged token; it cannot silently discard live custody.
    #[allow(clippy::result_large_err)] // Refusal returns affine custody without another allocation.
    pub(crate) fn release_base_backing(
        &mut self,
        token: BaseBackingReservation,
    ) -> Result<(), (BaseBackingReservation, String)> {
        if token.identity != self.identity
            || token.base.as_str().is_empty()
            || token.generation == 0
        {
            return Err((
                token,
                "Base backing retirement targets a different ledger".into(),
            ));
        }
        if let Err(reason) =
            PoolUsage::release_all(&mut self.pools, core::slice::from_ref(&token.binding))
        {
            return Err((token, reason));
        }
        Ok(())
    }
}

impl KernelResourceLedger {
    /// Replace only one independently selected storage Base while preserving
    /// all unrelated inventory and the same existing native Source backing.
    pub(crate) fn prepare_storage_refresh(
        &self,
        current: &HostAdvertisement,
        next: &HostAdvertisement,
        previous: Option<&conduit_core::BaseProviderEntry>,
        replacement: &conduit_core::BaseProviderEntry,
        backing: Option<&BaseBackingReservation>,
    ) -> Result<Self, String> {
        let strip = |host: &HostAdvertisement, base: Option<&conduit_core::BaseProviderEntry>| {
            let mut host = host.clone();
            if let Some(base) = base {
                host.resources
                    .retain(|r| !base.resources.iter().any(|b| b == r));
                host.capabilities
                    .retain(|r| !base.capabilities.iter().any(|b| b == r));
                host.bases.retain(|b| b.base_id != base.base_id);
            }
            host.resources.sort();
            host.capabilities.sort_by(|a,b|a.capability_id.cmp(&b.capability_id));
            host.bases.sort_by(|a,b|a.base_id.cmp(&b.base_id));
            host.offer_generation = conduit_core::OfferGeneration(0);
            host
        };
        if self.identity
            != (
                current.host_id.clone(),
                current.boot_id.clone(),
                current.offer_generation,
            )
            || next.offer_generation.0
                != current
                    .offer_generation
                    .0
                    .checked_add(1)
                    .ok_or("offer generation exhausted")?
            || strip(current, previous) != strip(next, Some(replacement))
            || !match backing {
                Some(token) => self.only_base_backing_is_reserved(token),
                None => self.is_idle(),
            }
        {
            return Err(
                "storage refresh requires exact scoped inventory and quiescent child Plays".into(),
            );
        }
        let mut staged = Self::new(next)?;
        if let Some(token) = backing {
            if !current
                .resources
                .iter()
                .any(|r| r.pool_id == token.binding.pool_id && next.resources.contains(r))
            {
                return Err("storage refresh changed retained Source backing pool".into());
            }
            PoolUsage::reserve_all(&mut staged.pools, core::slice::from_ref(&token.binding))?;
        }
        Ok(staged)
    }
}
