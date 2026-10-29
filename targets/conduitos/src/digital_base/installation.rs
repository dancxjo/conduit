//! Ready native ownership projected into ordinary planning, then consumed by Play.
use super::{
    DigitalProvider,
    contract::{DIGITAL_CALL, DIGITAL_MAXIMUM_BYTES, DigitalContract},
    owner::{DigitalAttachment, DigitalCallRefusal, DigitalCallSelection, DigitalHostCall},
};
use alloc::vec;
use conduit_core::*;

pub const DIGITAL_IMPLEMENTATION: &str = "conduitos/digital-access@1";
pub const DIGITAL_EXECUTION_PROFILE: &str = "conduitos/digital-cooperative-bounded@1";
pub const DIGITAL_ATTACHMENT_CLASS: &str = "machine/digital-attachment";
pub const DIGITAL_AUTHORITY: &str = "conduitos.authority/digital-access@1";

/// Native ownership facts. Protocol-level identity and probe success are absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DigitalNativeIdentity {
    pub capability_id: CapabilityId,
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

pub struct ReadyDigitalBase<P: DigitalProvider> {
    identity: DigitalNativeIdentity,
    attachment: DigitalAttachment,
    provider: Option<P>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DigitalInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: DigitalProvider> ReadyDigitalBase<P> {
    /// Retain an exclusively owned, configured native controller before planning.
    ///
    /// # Safety
    /// The native composition root must own exactly the named provider, resource
    /// generation, configuration authority and electrical attachment. The provider
    /// must preserve bounded termination and mandatory local safety on failure.
    /// Descriptive discovery or an authored request does not establish ownership.
    pub unsafe fn new(
        identity: DigitalNativeIdentity,
        attachment: DigitalAttachment,
        provider: P,
    ) -> Result<Self, DigitalInstallationRefusal> {
        let provider = super::provider_guard::ProviderGuard::new(provider);
        if identity.capability_id.as_str().is_empty()
            || identity.host_id.as_str().is_empty()
            || identity.boot_id.as_str().is_empty()
            || identity.base_id.as_str().is_empty()
            || identity.provider_instance_id.as_str().is_empty()
            || identity.provider_generation == 0
            || identity.resource_pool_id.as_str().is_empty()
            || identity.resource_generation_id.0.is_empty()
            || identity.envelope_id.as_str().is_empty()
            || identity.artifact_id.as_str().is_empty()
            || attachment.generation == 0
            || attachment.resource_bytes < 1
        {
            return Err(DigitalInstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            attachment,
            provider: Some(provider.into_inner()),
        })
    }

    /// Erase only the provider's Rust type during preparation; retain exact ownership.
    pub fn erase(mut self) -> ReadyDigitalBase<alloc::boxed::Box<dyn DigitalProvider>>
    where
        P: 'static,
    {
        ReadyDigitalBase {
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
        contract: &DigitalContract,
    ) -> Result<(), DigitalInstallationRefusal> {
        if advertisement.host_id != self.identity.host_id
            || advertisement.boot_id != self.identity.boot_id
        {
            return Err(DigitalInstallationRefusal::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == self.identity.base_id
                || base.provider_instance_id == self.identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id == self.identity.capability_id)
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.identity.resource_pool_id)
        {
            return Err(DigitalInstallationRefusal::Advertisement);
        }
        let kind = contract.kind();
        let capability = conduit_core::capability_offer_from_parts! {
            semantic_contract: kind.semantic_contract(),
            startup_parameters: kind.startup_parameters.clone(),
            shorthand: kind.shorthand.clone(),
            capability_id: self.identity.capability_id.clone(),
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(DIGITAL_EXECUTION_PROFILE),
                implementation_id: ImplementationId::from(DIGITAL_IMPLEMENTATION),
                artifact_id: self.identity.artifact_id.clone(),
            },
            inputs: kind.inputs.clone(),
            outputs: kind.outputs.clone(),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(DIGITAL_CALL),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: DIGITAL_MAXIMUM_BYTES,
                maximum_output_bytes: DIGITAL_MAXIMUM_BYTES,
            }],
            resource_requirements: vec![resource_requirement(DIGITAL_ATTACHMENT_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(DIGITAL_AUTHORITY),
                host_call_contract_id: HostCallContractId::from(DIGITAL_CALL),
                subject_kind: kind.kind_id.clone(),
            }],
            limits: kind.limits.clone(),
        };
        let resource = resource_offer(
            self.identity.resource_pool_id.as_str(),
            DIGITAL_ATTACHMENT_CLASS,
            1,
        );
        let capability_capacity = u16::try_from(advertisement.capabilities.len().saturating_add(1))
            .map_err(|_| DigitalInstallationRefusal::Advertisement)?;
        let resource_capacity = u16::try_from(advertisement.resources.len().saturating_add(1))
            .map_err(|_| DigitalInstallationRefusal::Advertisement)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: capability_capacity,
            maximum_advertised_resources: resource_capacity,
        })
        .map_err(|_| DigitalInstallationRefusal::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: self.identity.base_id.clone(),
                provider_instance_id: self.identity.provider_instance_id.clone(),
                provider_generation: self.identity.provider_generation,
                implementation_id: BaseImplementationId::from(
                    "conduitos.base/digital-controller@1",
                ),
                mechanism_family: HostBaseKindId::from(DIGITAL_ATTACHMENT_CLASS),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![capability],
                resources: vec![resource],
            })
            .map_err(|_| DigitalInstallationRefusal::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| DigitalInstallationRefusal::Advertisement)
    }

    /// Consume native ownership into the exact selected Host Call.
    pub fn bind_selected(
        mut self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: DigitalCallSelection<'_>,
    ) -> Result<DigitalHostCall<P>, DigitalCallRefusal> {
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement)
            .ok_or(DigitalCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(DigitalCallRefusal::WrongBinding)?;
        if claim.host_id != self.identity.host_id
            || claim.boot_id != self.identity.boot_id
            || claim.base_instance_id != self.identity.provider_instance_id
            || claim.base_provider_generation != self.identity.provider_generation
            || claim.resource_pool_id != self.identity.resource_pool_id
            || claim.resource_generation_id != self.identity.resource_generation_id
            || claim.envelope_id != self.identity.envelope_id
            || claim.implementation_id.as_str() != DIGITAL_IMPLEMENTATION
            || gear.artifact_id != self.identity.artifact_id
            || gear.execution_profile_id.as_str() != DIGITAL_EXECUTION_PROFILE
            || gear.capability_id != self.identity.capability_id
            || gear.limits != selection.contract.kind().limits
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == DIGITAL_AUTHORITY
                    && authority.host_call_contract_id.as_str() == DIGITAL_CALL
                    && authority.subject_kind == selection.contract.kind().kind_id
            })
            || base.base_id != self.identity.base_id
            || base.implementation_id.as_str() != "conduitos.base/digital-controller@1"
            || base.mechanism_family.as_str() != DIGITAL_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(DigitalCallRefusal::WrongBinding);
        }
        // SAFETY: new retained actual native ownership; the exact claim above
        // preserves its host, boot, provider, resource generation and envelope.
        unsafe {
            DigitalHostCall::bind_selected(
                table,
                handle,
                claim,
                self.attachment,
                self.provider.take().ok_or(DigitalCallRefusal::Possession)?,
                selection,
            )
        }
    }
}

#[cfg(test)]
mod tests;

impl<P: DigitalProvider> Drop for ReadyDigitalBase<P> {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.as_mut() {
            provider.revoke();
        }
    }
}
