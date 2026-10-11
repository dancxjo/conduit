//! Native USB identity and non-authorizing ordinary Host offer projection.
use super::{control_contract::ControlContract, control_factory::*, control_offer};
use alloc::vec;
use conduit_core::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsbNativeIdentity {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsbInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}

impl UsbNativeIdentity {
    pub(crate) fn validate(&self) -> Result<(), UsbInstallationRefusal> {
        if self.host_id.as_str().is_empty()
            || self.boot_id.as_str().is_empty()
            || self.base_id.as_str().is_empty()
            || self.provider_instance_id.as_str().is_empty()
            || self.provider_generation == 0
            || self.resource_pool_id.as_str().is_empty()
            || self.resource_generation_id.0.is_empty()
            || self.envelope_id.as_str().is_empty()
            || self.artifact_id.as_str().is_empty()
        {
            return Err(UsbInstallationRefusal::InvalidIdentity);
        }
        Ok(())
    }

    pub(crate) fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &ControlContract,
    ) -> Result<(), UsbInstallationRefusal> {
        self.validate()?;
        if advertisement.host_id != self.host_id || advertisement.boot_id != self.boot_id {
            return Err(UsbInstallationRefusal::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == self.base_id || base.provider_instance_id == self.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id.as_str() == CONTROL_IMPLEMENTATION)
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.resource_pool_id)
        {
            return Err(UsbInstallationRefusal::Advertisement);
        }
        let capabilities = u16::try_from(advertisement.capabilities.len().saturating_add(1))
            .map_err(|_| UsbInstallationRefusal::Advertisement)?;
        let resources = u16::try_from(advertisement.resources.len().saturating_add(1))
            .map_err(|_| UsbInstallationRefusal::Advertisement)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: capabilities,
            maximum_advertised_resources: resources,
        })
        .map_err(|_| UsbInstallationRefusal::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: self.base_id.clone(),
                provider_instance_id: self.provider_instance_id.clone(),
                provider_generation: self.provider_generation,
                implementation_id: CONTROL_BASE.into(),
                mechanism_family: CONTROL_ATTACHMENT.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![control_offer::offer(contract, self.artifact_id.clone())],
                resources: vec![resource_offer(
                    self.resource_pool_id.as_str(),
                    CONTROL_ATTACHMENT,
                    1,
                )],
            })
            .map_err(|_| UsbInstallationRefusal::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| UsbInstallationRefusal::Advertisement)
    }
}

#[cfg(test)]
mod tests;
