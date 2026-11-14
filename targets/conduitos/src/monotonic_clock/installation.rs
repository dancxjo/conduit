//! Ready native ownership projected into ordinary planning, then consumed by Play.
use alloc::vec;
use conduit_core::*;
use conduit_monotonic_wait::clock_at::{
    contract::{CLOCK_CALL, MonotonicClockContract},
    owner::MonotonicDeadlineProvider,
    owner::{ClockCallRefusal, ClockCallSelection, MonotonicClockHostCall},
};

pub use conduit_monotonic_wait::clock_at::installation::{
    CLOCK_AUTHORITY, CLOCK_EXECUTION_PROFILE, CLOCK_IMPLEMENTATION, CLOCK_RESOURCE_CLASS,
};

/// Native clock ownership and generation facts. No device protocol is present.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockNativeIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub base_id: HostBaseId,
    pub provider_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub resource_pool_id: ResourcePoolId,
    pub resource_generation_id: ResourceGenerationId,
    pub envelope_id: CapabilityEnvelopeId,
    pub artifact_id: ArtifactId,
}

pub struct ReadyClockBase<P> {
    identity: ClockNativeIdentity,
    provider: P,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: MonotonicDeadlineProvider> ReadyClockBase<P> {
    /// Retain an owned, calibrated native millisecond clock before planning.
    ///
    /// # Safety
    /// The native root owns the named clock generation and its calibration.
    /// Provider observations must use milliseconds on that exact monotonic
    /// basis, with bounded polls and quiescence on revocation. Discovery
    /// and authored deadlines do not establish possession.
    pub unsafe fn new(
        identity: ClockNativeIdentity,
        provider: P,
    ) -> Result<Self, ClockInstallationRefusal> {
        if identity.host_id.as_str().is_empty()
            || identity.boot_id.as_str().is_empty()
            || identity.base_id.as_str().is_empty()
            || identity.provider_instance_id.as_str().is_empty()
            || identity.provider_generation == 0
            || identity.resource_pool_id.as_str().is_empty()
            || identity.resource_generation_id.0.is_empty()
            || identity.envelope_id.as_str().is_empty()
            || identity.artifact_id.as_str().is_empty()
        {
            return Err(ClockInstallationRefusal::InvalidIdentity);
        }
        Ok(Self { identity, provider })
    }

    /// Offer only the provider retained by this ready native owner. No observations or timer effects.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &MonotonicClockContract,
    ) -> Result<(), ClockInstallationRefusal> {
        if advertisement.host_id != self.identity.host_id
            || advertisement.boot_id != self.identity.boot_id
        {
            return Err(ClockInstallationRefusal::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == self.identity.base_id
                || base.provider_instance_id == self.identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id.as_str() == "conduitos/monotonic-clock-at@1")
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.identity.resource_pool_id)
        {
            return Err(ClockInstallationRefusal::Advertisement);
        }
        let capability = conduit_monotonic_wait::clock_at::installation::capability_offer(
            contract,
            &self.identity.artifact_id,
        )
        .map_err(|_| ClockInstallationRefusal::InvalidIdentity)?;
        let resource = resource_offer(
            self.identity.resource_pool_id.as_str(),
            CLOCK_RESOURCE_CLASS,
            1,
        );
        let capability_capacity = u16::try_from(advertisement.capabilities.len().saturating_add(1))
            .map_err(|_| ClockInstallationRefusal::Advertisement)?;
        let resource_capacity = u16::try_from(advertisement.resources.len().saturating_add(1))
            .map_err(|_| ClockInstallationRefusal::Advertisement)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: capability_capacity,
            maximum_advertised_resources: resource_capacity,
        })
        .map_err(|_| ClockInstallationRefusal::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: self.identity.base_id.clone(),
                provider_instance_id: self.identity.provider_instance_id.clone(),
                provider_generation: self.identity.provider_generation,
                implementation_id: BaseImplementationId::from("conduitos.base/monotonic-clock@1"),
                mechanism_family: HostBaseKindId::from(CLOCK_RESOURCE_CLASS),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![capability],
                resources: vec![resource],
            })
            .map_err(|_| ClockInstallationRefusal::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| ClockInstallationRefusal::Advertisement)
    }

    /// Consume native ownership into the exact selected Host Call.
    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: ClockCallSelection<'_>,
    ) -> Result<MonotonicClockHostCall<P>, ClockCallRefusal> {
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement)
            .ok_or(ClockCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(ClockCallRefusal::WrongBinding)?;
        if claim.host_id != self.identity.host_id
            || claim.boot_id != self.identity.boot_id
            || claim.base_instance_id != self.identity.provider_instance_id
            || claim.base_provider_generation != self.identity.provider_generation
            || claim.resource_pool_id != self.identity.resource_pool_id
            || claim.resource_generation_id != self.identity.resource_generation_id
            || claim.envelope_id != self.identity.envelope_id
            || claim.implementation_id.as_str() != CLOCK_IMPLEMENTATION
            || gear.artifact_id != self.identity.artifact_id
            || gear.execution_profile_id.as_str() != CLOCK_EXECUTION_PROFILE
            || gear.capability_id.as_str() != "conduitos/monotonic-clock-at@1"
            || gear.limits != selection.contract.kind().limits
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == CLOCK_AUTHORITY
                    && authority.host_call_contract_id.as_str() == CLOCK_CALL
                    && authority.subject_kind == selection.contract.kind().kind_id
            })
            || base.base_id != self.identity.base_id
            || base.implementation_id.as_str() != "conduitos.base/monotonic-clock@1"
            || base.mechanism_family.as_str() != CLOCK_RESOURCE_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(ClockCallRefusal::WrongBinding);
        }
        // SAFETY: new retained actual native ownership; the exact claim above
        // preserves its host, boot, provider, resource generation and envelope.
        unsafe {
            MonotonicClockHostCall::bind_selected(table, handle, claim, self.provider, selection)
        }
    }
}

#[cfg(test)]
mod tests;
