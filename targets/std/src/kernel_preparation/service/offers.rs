//! Offers are projected only after each bounded worker reports ready.
use super::*;
use conduit_core::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RegionServiceConfig {
    pub lanes: u8,
    pub stack_bytes: u32,
    pub transfer_bytes: u32,
    /// Managed per-Play quota for each lane, including its stack reservation.
    pub worker_memory_bytes: u32,
}
impl RegionServiceConfig {
    pub fn new(lanes: u8) -> Self {
        Self {
            lanes,
            stack_bytes: 2 * 1024 * 1024,
            transfer_bytes: 256 * 1024,
            worker_memory_bytes: 16 * 1024 * 1024,
        }
    }
    pub(super) fn validate(self) -> Result<(), String> {
        if !matches!(self.lanes, 1 | 2 | 4)
            || !(2 * 1024 * 1024..=16 * 1024 * 1024).contains(&self.stack_bytes)
            || !self.stack_bytes.is_multiple_of(4096)
            || !(1..=512 * 1024).contains(&self.transfer_bytes)
            || self.worker_memory_bytes > 64 * 1024 * 1024
            || self
                .worker_memory_bytes
                .checked_sub(self.stack_bytes)
                .is_none_or(|bytes| bytes < 1024 * 1024)
        {
            return Err("region service configuration is outside the finite Linux profile".into());
        }
        Ok(())
    }
    pub fn execution_profile_id(self) -> ExecutionProfileId {
        format!(
            "conduit.std/region-worker/lanes-{}/stack-{}/transfer-{}/memory-{}@1",
            self.lanes, self.stack_bytes, self.transfer_bytes, self.worker_memory_bytes
        )
        .into()
    }
}
pub(super) struct ReadyOffers {
    pub advertisement: HostAdvertisement,
    pub registry: BaseRegistry,
    pub provider: BaseProviderEntry,
    pub shutdown_generation: OfferGeneration,
    pub control_backing_bytes: usize,
}
pub(super) fn initialize(
    previous: &HostAdvertisement,
    registry: &BaseRegistry,
    config: RegionServiceConfig,
    workers: &[worker::InitializedRegionWorker],
) -> Result<ReadyOffers, String> {
    config.validate()?;
    if previous.protocol_version != PROTOCOL_VERSION
        || previous.host_id.as_str().is_empty()
        || previous.boot_id.as_str().is_empty()
        || previous.offer_generation.0 == 0
    {
        return Err("worker service requires an exact current Host Boot".into());
    }
    let generation = previous
        .offer_generation
        .0
        .checked_add(1)
        .ok_or("worker offer generation exhausted")?;
    let shutdown_generation = OfferGeneration(
        generation
            .checked_add(1)
            .ok_or("worker retirement generation exhausted")?,
    );
    if workers.len() != usize::from(config.lanes)
        || workers
            .iter()
            .any(|w| !w.is_alive() || w.stack_bytes() != config.stack_bytes)
    {
        return Err("worker capacity is not initialized".into());
    }
    let control_backing_bytes = workers.iter().try_fold(0usize, |bytes, w| {
        bytes
            .checked_add(
                w.control_backing_bytes()
                    .map_err(|e| format!("worker backing: {e:?}"))?,
            )
            .ok_or_else(|| "worker control backing overflow".to_string())
    })?;
    let base_id: HostBaseId = format!("{}/std-region-workers", previous.host_id.as_str()).into();
    let lane_pool: ResourcePoolId = format!("{}/lanes", base_id.as_str()).into();
    let memory_pool: ResourcePoolId = format!("{}/memory", base_id.as_str()).into();
    if previous
        .resources
        .iter()
        .any(|r| r.pool_id == lane_pool || r.pool_id == memory_pool)
        || previous.bases.iter().any(|b| b.base_id == base_id)
    {
        return Err("worker service resource/Base identity collides with current offers".into());
    }
    let provider = BaseProviderEntry {
        base_id: base_id.clone(),
        provider_instance_id: format!(
            "{}/{}/{}",
            base_id.as_str(),
            previous.boot_id.as_str(),
            generation
        )
        .into(),
        provider_generation: generation,
        implementation_id: "conduit.std/region-workers-linux@1".into(),
        mechanism_family: "conduit.base/std-cooperative-workers@1".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: vec![
            ResourceOffer {
                pool_id: lane_pool,
                class_id: "conduit.resource/execution-lane@1".into(),
                capacity_units: u32::from(config.lanes),
                content: None,
                compute: Some(ComputePoolContract {
                    service_guarantee: ComputeServiceGuarantee::Shared,
                    architecture_base_id: base_id.as_str().into(),
                    architecture_base_kind: ArchitectureBaseKind::HostedOs,
                    topology_groups: Vec::new(),
                }),
            },
            ResourceOffer {
                pool_id: memory_pool,
                class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
                capacity_units: config
                    .worker_memory_bytes
                    .checked_mul(u32::from(config.lanes))
                    .ok_or("worker memory capacity overflow")?,
                compute: None,
                content: None,
            },
        ],
    };
    let mut registry = registry.clone();
    if let Some(old) = registry
        .entries()
        .iter()
        .find(|entry| entry.base_id == base_id)
        .cloned()
    {
        if old.lifecycle != BaseLifecycle::Stopped
            || old.implementation_id != provider.implementation_id
            || old.mechanism_family != provider.mechanism_family
        {
            return Err("worker Base is already owned or not retired".into());
        }
        registry
            .replace(
                &base_id,
                &old.provider_instance_id,
                old.provider_generation,
                provider.clone(),
            )
            .map_err(|e| format!("worker Base replacement: {e:?}"))?;
    } else {
        registry
            .register(provider.clone())
            .map_err(|e| format!("worker Base registration: {e:?}"))?;
    }
    let mut advertisement = previous.clone();
    advertisement.offer_generation = OfferGeneration(generation);
    advertisement.profile = config.execution_profile_id().as_str().into();
    registry
        .project_base_ready_into(&base_id, &mut advertisement)
        .map_err(|e| format!("worker Base projection: {e:?}"))?;
    advertisement.resources.sort();
    Ok(ReadyOffers {
        advertisement,
        registry,
        provider,
        shutdown_generation,
        control_backing_bytes,
    })
}
