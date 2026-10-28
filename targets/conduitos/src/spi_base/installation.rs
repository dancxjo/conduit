//! Ready native ownership projected into ordinary planning, then consumed by Play.
use super::{
    SpiProvider,
    contract::{SPI_CALL, SPI_MAXIMUM_BYTES, SpiContract},
    owner::{SpiAttachment, SpiCallRefusal, SpiCallSelection, SpiHostCall},
};
use alloc::vec;
use conduit_core::*;

pub const SPI_IMPLEMENTATION: &str = "conduitos/spi-exchange@1";
pub const SPI_EXECUTION_PROFILE: &str = "conduitos/spi-cooperative-bounded@1";
pub const SPI_ATTACHMENT_CLASS: &str = "machine/spi-attachment";
pub const SPI_AUTHORITY: &str = "conduitos.authority/spi-exchange@1";

/// Native ownership facts. Protocol-level identity and probe success are absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpiNativeIdentity {
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

pub struct ReadySpiBase<P: SpiProvider> {
    identity: SpiNativeIdentity,
    attachment: SpiAttachment,
    provider: Option<P>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpiInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: SpiProvider> ReadySpiBase<P> {
    /// Retain an exclusively owned, configured native controller before planning.
    ///
    /// # Safety
    /// The native composition root must own exactly the named provider, resource
    /// generation, configuration authority and electrical attachment. The provider
    /// must preserve bounded termination and mandatory local safety on failure.
    /// Descriptive discovery or an authored request does not establish ownership.
    pub unsafe fn new(
        identity: SpiNativeIdentity,
        attachment: SpiAttachment,
        provider: P,
    ) -> Result<Self, SpiInstallationRefusal> {
        let provider = super::provider_guard::ProviderGuard::new(provider);
        if identity.host_id.as_str().is_empty()
            || identity.boot_id.as_str().is_empty()
            || identity.base_id.as_str().is_empty()
            || identity.provider_instance_id.as_str().is_empty()
            || identity.provider_generation == 0
            || identity.resource_pool_id.as_str().is_empty()
            || identity.resource_generation_id.0.is_empty()
            || identity.envelope_id.as_str().is_empty()
            || identity.artifact_id.as_str().is_empty()
            || attachment.generation == 0
            || attachment.resource_bytes < super::transaction::MAXIMUM_TRANSACTION_BYTES as u64
        {
            return Err(SpiInstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            attachment,
            provider: Some(provider.into_inner()),
        })
    }

    /// Erase only the provider's Rust type during preparation; retain exact ownership.
    pub fn erase(mut self) -> ReadySpiBase<alloc::boxed::Box<dyn SpiProvider>>
    where
        P: 'static,
    {
        ReadySpiBase {
            identity: self.identity.clone(),
            attachment: self.attachment,
            provider: Some(alloc::boxed::Box::new(
                self.provider.take().expect("retained native provider"),
            )),
        }
    }

    /// Offer only the provider retained by this ready native owner. No effects.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &SpiContract,
    ) -> Result<(), SpiInstallationRefusal> {
        if advertisement.host_id != self.identity.host_id
            || advertisement.boot_id != self.identity.boot_id
        {
            return Err(SpiInstallationRefusal::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == self.identity.base_id
                || base.provider_instance_id == self.identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id.as_str() == "conduitos/spi-exchange@1")
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.identity.resource_pool_id)
        {
            return Err(SpiInstallationRefusal::Advertisement);
        }
        let kind = contract.kind();
        let capability = conduit_core::capability_offer_from_parts! {
            semantic_contract: kind.semantic_contract(),
            startup_parameters: kind.startup_parameters.clone(),
            shorthand: kind.shorthand.clone(),
            capability_id: CapabilityId::from("conduitos/spi-exchange@1"),
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(SPI_EXECUTION_PROFILE),
                implementation_id: ImplementationId::from(SPI_IMPLEMENTATION),
                artifact_id: self.identity.artifact_id.clone(),
            },
            inputs: kind.inputs.clone(),
            outputs: kind.outputs.clone(),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(SPI_CALL),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: SPI_MAXIMUM_BYTES,
                maximum_output_bytes: SPI_MAXIMUM_BYTES,
            }],
            resource_requirements: vec![resource_requirement(SPI_ATTACHMENT_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(SPI_AUTHORITY),
                host_call_contract_id: HostCallContractId::from(SPI_CALL),
                subject_kind: kind.kind_id.clone(),
            }],
            limits: kind.limits.clone(),
        };
        let resource = resource_offer(
            self.identity.resource_pool_id.as_str(),
            SPI_ATTACHMENT_CLASS,
            1,
        );
        let capability_capacity = u16::try_from(advertisement.capabilities.len().saturating_add(1))
            .map_err(|_| SpiInstallationRefusal::Advertisement)?;
        let resource_capacity = u16::try_from(advertisement.resources.len().saturating_add(1))
            .map_err(|_| SpiInstallationRefusal::Advertisement)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: capability_capacity,
            maximum_advertised_resources: resource_capacity,
        })
        .map_err(|_| SpiInstallationRefusal::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: self.identity.base_id.clone(),
                provider_instance_id: self.identity.provider_instance_id.clone(),
                provider_generation: self.identity.provider_generation,
                implementation_id: BaseImplementationId::from("conduitos.base/spi-controller@1"),
                mechanism_family: HostBaseKindId::from(SPI_ATTACHMENT_CLASS),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![capability],
                resources: vec![resource],
            })
            .map_err(|_| SpiInstallationRefusal::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| SpiInstallationRefusal::Advertisement)
    }

    /// Consume native ownership into the exact selected Host Call.
    pub fn bind_selected(
        mut self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: SpiCallSelection<'_>,
    ) -> Result<SpiHostCall<P>, SpiCallRefusal> {
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement)
            .ok_or(SpiCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(SpiCallRefusal::WrongBinding)?;
        if claim.host_id != self.identity.host_id
            || claim.boot_id != self.identity.boot_id
            || claim.base_instance_id != self.identity.provider_instance_id
            || claim.base_provider_generation != self.identity.provider_generation
            || claim.resource_pool_id != self.identity.resource_pool_id
            || claim.resource_generation_id != self.identity.resource_generation_id
            || claim.envelope_id != self.identity.envelope_id
            || claim.implementation_id.as_str() != SPI_IMPLEMENTATION
            || gear.artifact_id != self.identity.artifact_id
            || gear.execution_profile_id.as_str() != SPI_EXECUTION_PROFILE
            || gear.capability_id.as_str() != "conduitos/spi-exchange@1"
            || gear.limits != selection.contract.kind().limits
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == SPI_AUTHORITY
                    && authority.host_call_contract_id.as_str() == SPI_CALL
                    && authority.subject_kind == selection.contract.kind().kind_id
            })
            || base.base_id != self.identity.base_id
            || base.implementation_id.as_str() != "conduitos.base/spi-controller@1"
            || base.mechanism_family.as_str() != SPI_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(SpiCallRefusal::WrongBinding);
        }
        // SAFETY: new retained actual native ownership; the exact claim above
        // preserves its host, boot, provider, resource generation and envelope.
        unsafe {
            SpiHostCall::bind_selected(
                table,
                handle,
                claim,
                self.attachment,
                self.provider.take().ok_or(SpiCallRefusal::Possession)?,
                selection,
            )
        }
    }
}

#[cfg(test)]
mod tests;

impl<P: SpiProvider> Drop for ReadySpiBase<P> {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.as_mut() {
            provider.revoke();
        }
    }
}
