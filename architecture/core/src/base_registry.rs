//! Bounded current-truth registry for independently realized Host Bases.
//!
//! The registry describes and aggregates Base-owned offers. It grants no
//! authority and performs no planning or effects.

use crate::{
    BaseImplementationId, BaseInstanceId, BootId, CapabilityId, CapabilityOffer, HostAdvertisement,
    HostBaseId, HostBaseKindId, HostId, HostProfileId, OfferGeneration, ResourceOffer,
    ResourcePoolId, PROTOCOL_VERSION,
};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseEnforcementClass {
    Cooperative,
    ProcessIsolated,
    WasmConfined,
    OsCapabilityMediated,
    ConduitOsKernelEnforced,
    HardwareGated,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseLifecycle {
    Configured,
    Starting,
    Ready,
    Degraded,
    Lost,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseProviderEntry {
    pub base_id: HostBaseId,
    pub provider_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub implementation_id: BaseImplementationId,
    pub mechanism_family: HostBaseKindId,
    pub enforcement_class: BaseEnforcementClass,
    pub lifecycle: BaseLifecycle,
    pub capabilities: Vec<CapabilityOffer>,
    pub resources: Vec<ResourceOffer>,
}

/// Current non-authorizing provenance for one advertised Base provider.
///
/// Capability and resource identities reference the ordinary offers in the
/// containing Host advertisement. They do not affect semantic Front matching
/// and possession is still issued and checked independently.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseProviderAdvertisement {
    pub base_id: HostBaseId,
    pub provider_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub implementation_id: BaseImplementationId,
    pub mechanism_family: HostBaseKindId,
    pub enforcement_class: BaseEnforcementClass,
    pub lifecycle: BaseLifecycle,
    pub capability_ids: Vec<CapabilityId>,
    pub resource_pool_ids: Vec<ResourcePoolId>,
}

/// Exact Base provider selected into immutable Plan truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaseProviderBinding {
    pub base_id: HostBaseId,
    pub provider_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub implementation_id: BaseImplementationId,
    pub mechanism_family: HostBaseKindId,
    pub enforcement_class: BaseEnforcementClass,
}

impl BaseProviderAdvertisement {
    pub fn binding(&self) -> BaseProviderBinding {
        BaseProviderBinding {
            base_id: self.base_id.clone(),
            provider_instance_id: self.provider_instance_id.clone(),
            provider_generation: self.provider_generation,
            implementation_id: self.implementation_id.clone(),
            mechanism_family: self.mechanism_family.clone(),
            enforcement_class: self.enforcement_class,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BaseRegistryLimits {
    pub maximum_bases: u16,
    pub maximum_capabilities_per_base: u16,
    pub maximum_resources_per_base: u16,
    pub maximum_advertised_capabilities: u16,
    pub maximum_advertised_resources: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseRegistryRefusal {
    EmptyIdentity,
    ZeroGeneration,
    InvalidLimits,
    RegistryFull,
    TooManyCapabilities,
    TooManyResources,
    DuplicateBase,
    DuplicateProviderInstance,
    UnknownBase,
    StaleProvider,
    GenerationDidNotAdvance,
    AdvertisementCapacity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseRegistry {
    limits: BaseRegistryLimits,
    entries: Vec<BaseProviderEntry>,
    revision: u64,
}

impl BaseRegistry {
    pub fn new(limits: BaseRegistryLimits) -> Result<Self, BaseRegistryRefusal> {
        if limits.maximum_bases == 0
            || limits.maximum_advertised_capabilities == 0
            || limits.maximum_advertised_resources == 0
        {
            return Err(BaseRegistryRefusal::InvalidLimits);
        }
        Ok(Self {
            limits,
            entries: Vec::with_capacity(limits.maximum_bases as usize),
            revision: 0,
        })
    }

    pub fn entries(&self) -> &[BaseProviderEntry] {
        &self.entries
    }

    pub fn register(&mut self, entry: BaseProviderEntry) -> Result<(), BaseRegistryRefusal> {
        validate_entry(&entry, self.limits)?;
        if self.entries.len() == self.limits.maximum_bases as usize {
            return Err(BaseRegistryRefusal::RegistryFull);
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.base_id == entry.base_id)
        {
            return Err(BaseRegistryRefusal::DuplicateBase);
        }
        if self
            .entries
            .iter()
            .any(|existing| existing.provider_instance_id == entry.provider_instance_id)
        {
            return Err(BaseRegistryRefusal::DuplicateProviderInstance);
        }
        self.entries.push(entry);
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn set_lifecycle(
        &mut self,
        base_id: &HostBaseId,
        provider_instance_id: &BaseInstanceId,
        provider_generation: u64,
        lifecycle: BaseLifecycle,
    ) -> Result<(), BaseRegistryRefusal> {
        let entry = self.exact_mut(base_id, provider_instance_id, provider_generation)?;
        entry.lifecycle = lifecycle;
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    pub fn replace(
        &mut self,
        base_id: &HostBaseId,
        previous_instance_id: &BaseInstanceId,
        previous_generation: u64,
        replacement: BaseProviderEntry,
    ) -> Result<(), BaseRegistryRefusal> {
        validate_entry(&replacement, self.limits)?;
        if replacement.base_id != *base_id {
            return Err(BaseRegistryRefusal::UnknownBase);
        }
        if replacement.provider_generation <= previous_generation {
            return Err(BaseRegistryRefusal::GenerationDidNotAdvance);
        }
        if self.entries.iter().any(|existing| {
            existing.base_id != *base_id
                && existing.provider_instance_id == replacement.provider_instance_id
        }) {
            return Err(BaseRegistryRefusal::DuplicateProviderInstance);
        }
        let entry = self.exact_mut(base_id, previous_instance_id, previous_generation)?;
        *entry = replacement;
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    fn exact_mut(
        &mut self,
        base_id: &HostBaseId,
        provider_instance_id: &BaseInstanceId,
        provider_generation: u64,
    ) -> Result<&mut BaseProviderEntry, BaseRegistryRefusal> {
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| &entry.base_id == base_id)
            .ok_or(BaseRegistryRefusal::UnknownBase)?;
        if &entry.provider_instance_id != provider_instance_id
            || entry.provider_generation != provider_generation
        {
            return Err(BaseRegistryRefusal::StaleProvider);
        }
        Ok(entry)
    }

    fn append_ready_offers(
        &self,
        capabilities: &mut Vec<CapabilityOffer>,
        resources: &mut Vec<ResourceOffer>,
        bases: &mut Vec<BaseProviderAdvertisement>,
    ) -> Result<(), BaseRegistryRefusal> {
        for entry in self
            .entries
            .iter()
            .filter(|entry| entry.lifecycle == BaseLifecycle::Ready)
        {
            if capabilities.len() + entry.capabilities.len()
                > self.limits.maximum_advertised_capabilities as usize
                || resources.len() + entry.resources.len()
                    > self.limits.maximum_advertised_resources as usize
            {
                return Err(BaseRegistryRefusal::AdvertisementCapacity);
            }
            capabilities.extend(entry.capabilities.iter().cloned());
            resources.extend(entry.resources.iter().cloned());
            bases.push(BaseProviderAdvertisement {
                base_id: entry.base_id.clone(),
                provider_instance_id: entry.provider_instance_id.clone(),
                provider_generation: entry.provider_generation,
                implementation_id: entry.implementation_id.clone(),
                mechanism_family: entry.mechanism_family.clone(),
                enforcement_class: entry.enforcement_class,
                lifecycle: entry.lifecycle,
                capability_ids: entry
                    .capabilities
                    .iter()
                    .map(|offer| offer.capability_id.clone())
                    .collect(),
                resource_pool_ids: entry
                    .resources
                    .iter()
                    .map(|offer| offer.pool_id.clone())
                    .collect(),
            });
        }
        Ok(())
    }

    /// Projects current ready entries into one host advertisement.
    ///
    /// The ordinary offers and their provider ownership are emitted together;
    /// callers cannot accidentally flatten away Base provenance.
    pub fn project_ready_into(
        &self,
        advertisement: &mut HostAdvertisement,
    ) -> Result<(), BaseRegistryRefusal> {
        self.append_ready_offers(
            &mut advertisement.capabilities,
            &mut advertisement.resources,
            &mut advertisement.bases,
        )
    }
}

fn validate_entry(
    entry: &BaseProviderEntry,
    limits: BaseRegistryLimits,
) -> Result<(), BaseRegistryRefusal> {
    if entry.base_id.as_str().is_empty()
        || entry.provider_instance_id.as_str().is_empty()
        || entry.implementation_id.as_str().is_empty()
        || entry.mechanism_family.as_str().is_empty()
    {
        return Err(BaseRegistryRefusal::EmptyIdentity);
    }
    if entry.provider_generation == 0 {
        return Err(BaseRegistryRefusal::ZeroGeneration);
    }
    if entry.capabilities.len() > limits.maximum_capabilities_per_base as usize {
        return Err(BaseRegistryRefusal::TooManyCapabilities);
    }
    if entry.resources.len() > limits.maximum_resources_per_base as usize {
        return Err(BaseRegistryRefusal::TooManyResources);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinHostSupervisor {
    host_id: HostId,
    boot_id: BootId,
    profile: HostProfileId,
    pure_offer_revision: u64,
    pure_capabilities: Vec<CapabilityOffer>,
    registry: BaseRegistry,
}

impl ThinHostSupervisor {
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        profile: HostProfileId,
        registry: BaseRegistry,
    ) -> Result<Self, BaseRegistryRefusal> {
        if host_id.as_str().is_empty() || boot_id.as_str().is_empty() || profile.as_str().is_empty()
        {
            return Err(BaseRegistryRefusal::EmptyIdentity);
        }
        Ok(Self {
            host_id,
            boot_id,
            profile,
            pure_offer_revision: 0,
            pure_capabilities: Vec::new(),
            registry,
        })
    }

    pub fn registry(&self) -> &BaseRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut BaseRegistry {
        &mut self.registry
    }

    pub fn set_pure_capabilities(
        &mut self,
        capabilities: Vec<CapabilityOffer>,
    ) -> Result<(), BaseRegistryRefusal> {
        if capabilities.len() > self.registry.limits.maximum_advertised_capabilities as usize {
            return Err(BaseRegistryRefusal::AdvertisementCapacity);
        }
        self.pure_capabilities = capabilities;
        self.pure_offer_revision = self.pure_offer_revision.saturating_add(1);
        Ok(())
    }

    pub fn advertisement(&self) -> Result<HostAdvertisement, BaseRegistryRefusal> {
        let mut capabilities = self.pure_capabilities.clone();
        let mut resources = Vec::new();
        let mut bases = Vec::new();
        self.registry
            .append_ready_offers(&mut capabilities, &mut resources, &mut bases)?;
        Ok(HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            offer_generation: OfferGeneration(
                1_u64
                    .saturating_add(self.pure_offer_revision)
                    .saturating_add(self.registry.revision),
            ),
            profile: self.profile.clone(),
            bases,
            resources,
            capabilities,
            planner_capabilities: Vec::new(),
        })
    }
}
