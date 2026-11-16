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
        self.append_offer(
            advertisement,
            control_offer::offer(contract, self.artifact_id.clone()),
            CONTROL_BASE,
            CONTROL_ATTACHMENT,
        )
    }

    #[cfg(any(test, all(target_arch = "arm", feature = "armv6-product")))]
    pub(crate) fn append_endpoint_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &super::endpoint_read_contract::EndpointReadContract,
    ) -> Result<(), UsbInstallationRefusal> {
        self.append_offer(
            advertisement,
            super::endpoint_read_offer::offer(contract, self.artifact_id.clone()),
            super::endpoint_read_factory::ENDPOINT_READ_BASE,
            super::endpoint_read_factory::ENDPOINT_READ_ATTACHMENT,
        )
    }
    fn append_offer(
        &self,
        advertisement: &mut HostAdvertisement,
        offer: CapabilityOffer,
        base: &str,
        resource_class: &str,
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
            .any(|candidate| candidate.capability_id == offer.capability_id)
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
                implementation_id: base.into(),
                mechanism_family: resource_class.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    self.resource_pool_id.as_str(),
                    resource_class,
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
