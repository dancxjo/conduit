//! Ready native ownership projected into ordinary planning, then consumed by Play.
use super::{
    contract::{I2cContract, I2C_CALL, I2C_MAXIMUM_BYTES},
    owner::{I2cAttachment, I2cCallRefusal, I2cCallSelection, I2cHostCall},
    I2cProvider,
};
use alloc::vec;
use conduit_core::*;

mod offer;
pub use offer::capability_offer;

pub const I2C_IMPLEMENTATION: &str = "conduitos/i2c-transaction@1";
pub const I2C_EXECUTION_PROFILE: &str = "conduitos/i2c-cooperative-bounded@1";
pub const I2C_ATTACHMENT_CLASS: &str = "machine/i2c-attachment";
pub const I2C_AUTHORITY: &str = "conduitos.authority/i2c-transaction@1";

/// Native ownership facts. Protocol-level identity and probe success are absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct I2cNativeIdentity {
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

pub struct ReadyI2cBase<P> {
    identity: I2cNativeIdentity,
    attachment: I2cAttachment,
    provider: P,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum I2cInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: I2cProvider> ReadyI2cBase<P> {
    /// Retain an exclusively owned, configured native controller before planning.
    ///
    /// # Safety
    /// The native composition root must own exactly the named provider, resource
    /// generation, address authority and electrical attachment. The provider
    /// must preserve bounded termination and mandatory local safety on failure.
    /// Descriptive discovery or an authored request does not establish ownership.
    pub unsafe fn new(
        identity: I2cNativeIdentity,
        attachment: I2cAttachment,
        provider: P,
    ) -> Result<Self, I2cInstallationRefusal> {
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
            || attachment.minimum_address < 8
            || attachment.maximum_address > 119
            || attachment.minimum_address > attachment.maximum_address
            || attachment.resource_bytes < super::transaction::MAXIMUM_TRANSACTION_BYTES as u64
        {
            return Err(I2cInstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            attachment,
            provider,
        })
    }

    /// Offer only the provider retained by this ready native owner. No effects.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &I2cContract,
    ) -> Result<(), I2cInstallationRefusal> {
        if advertisement.host_id != self.identity.host_id
            || advertisement.boot_id != self.identity.boot_id
        {
            return Err(I2cInstallationRefusal::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == self.identity.base_id
                || base.provider_instance_id == self.identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id.as_str() == "conduitos/i2c-transaction@1")
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.identity.resource_pool_id)
        {
            return Err(I2cInstallationRefusal::Advertisement);
        }
        let capability = capability_offer(contract, &self.identity.artifact_id)?;
        let resource = resource_offer(
            self.identity.resource_pool_id.as_str(),
            I2C_ATTACHMENT_CLASS,
            1,
        );
        let capability_capacity = u16::try_from(advertisement.capabilities.len().saturating_add(1))
            .map_err(|_| I2cInstallationRefusal::Advertisement)?;
        let resource_capacity = u16::try_from(advertisement.resources.len().saturating_add(1))
            .map_err(|_| I2cInstallationRefusal::Advertisement)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: capability_capacity,
            maximum_advertised_resources: resource_capacity,
        })
        .map_err(|_| I2cInstallationRefusal::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: self.identity.base_id.clone(),
                provider_instance_id: self.identity.provider_instance_id.clone(),
                provider_generation: self.identity.provider_generation,
                implementation_id: BaseImplementationId::from("conduitos.base/i2c-controller@1"),
                mechanism_family: HostBaseKindId::from(I2C_ATTACHMENT_CLASS),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![capability],
                resources: vec![resource],
            })
            .map_err(|_| I2cInstallationRefusal::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| I2cInstallationRefusal::Advertisement)
    }

    /// Consume native ownership into the exact selected Host Call.
    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: I2cCallSelection<'_>,
    ) -> Result<I2cHostCall<P>, I2cCallRefusal> {
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement)
            .ok_or(I2cCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(I2cCallRefusal::WrongBinding)?;
        if claim.host_id != self.identity.host_id
            || claim.boot_id != self.identity.boot_id
            || claim.base_instance_id != self.identity.provider_instance_id
            || claim.base_provider_generation != self.identity.provider_generation
            || claim.resource_pool_id != self.identity.resource_pool_id
            || claim.resource_generation_id != self.identity.resource_generation_id
            || claim.envelope_id != self.identity.envelope_id
            || claim.implementation_id.as_str() != I2C_IMPLEMENTATION
            || gear.artifact_id != self.identity.artifact_id
            || gear.execution_profile_id.as_str() != I2C_EXECUTION_PROFILE
            || gear.capability_id.as_str() != "conduitos/i2c-transaction@1"
            || gear.limits != selection.contract.kind().limits
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == I2C_AUTHORITY
                    && authority.host_call_contract_id.as_str() == I2C_CALL
                    && authority.subject_kind == selection.contract.kind().kind_id
            })
            || base.base_id != self.identity.base_id
            || base.implementation_id.as_str() != "conduitos.base/i2c-controller@1"
            || base.mechanism_family.as_str() != I2C_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(I2cCallRefusal::WrongBinding);
        }
        // SAFETY: new retained actual native ownership; the exact claim above
        // preserves its host, boot, provider, resource generation and envelope.
        unsafe {
            I2cHostCall::bind_selected(
                table,
                handle,
                claim,
                self.attachment,
                self.provider,
                selection,
            )
        }
    }
}
