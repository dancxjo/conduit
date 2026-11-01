//! Ready native ownership projected into ordinary planning, then consumed by Play.
use super::{
    contract::{HistoryReadContract, HISTORY_CALL, HISTORY_MAXIMUM_BYTES},
    owner::{HistoryAttachment, HistoryCallRefusal, HistoryCallSelection, HistoryHostCall},
    HistoryProvider, MAXIMUM_SIGNED_BATCH_BYTES,
};
use alloc::vec;
use conduit_core::*;

pub const HISTORY_IMPLEMENTATION: &str = "environment/signed-history-read@1";
pub const HISTORY_EXECUTION_PROFILE: &str = "environment/signed-history-bounded@1";
pub const HISTORY_ATTACHMENT_CLASS: &str = "environment/signed-history-evidence";
pub const HISTORY_AUTHORITY: &str = "environment.authority/signed-history-read@1";

/// Native ownership facts. Protocol-level identity and probe success are absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryNativeIdentity {
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

pub struct ReadyHistoryBase<P> {
    identity: HistoryNativeIdentity,
    attachment: HistoryAttachment,
    provider: P,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
    InsufficientBacking,
}
impl<P: HistoryProvider> ReadyHistoryBase<P> {
    /// Retain an exclusively owned, configured evidence provider before planning.
    ///
    /// # Safety
    /// The native composition root must own exactly the named provider, resource
    /// generation, evidence scope. The provider
    /// must preserve bounded termination and mandatory local safety on failure.
    /// Descriptive discovery or an authored request does not establish ownership.
    pub unsafe fn new(
        identity: HistoryNativeIdentity,
        attachment: HistoryAttachment,
        provider: P,
    ) -> Result<Self, HistoryInstallationRefusal> {
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
        {
            return Err(HistoryInstallationRefusal::InvalidIdentity);
        }
        if super::minimum_attachment_bytes(&provider)
            .is_none_or(|required| attachment.resource_bytes < required)
        {
            return Err(HistoryInstallationRefusal::InsufficientBacking);
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
        contract: &HistoryReadContract,
    ) -> Result<(), HistoryInstallationRefusal> {
        if advertisement.host_id != self.identity.host_id
            || advertisement.boot_id != self.identity.boot_id
        {
            return Err(HistoryInstallationRefusal::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == self.identity.base_id
                || base.provider_instance_id == self.identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id.as_str() == "environment/signed-history-read@1")
            || advertisement
                .resources
                .iter()
                .any(|resource| resource.pool_id == self.identity.resource_pool_id)
        {
            return Err(HistoryInstallationRefusal::Advertisement);
        }
        let capability_capacity = u16::try_from(advertisement.capabilities.len().saturating_add(1))
            .map_err(|_| HistoryInstallationRefusal::Advertisement)?;
        let resource_capacity = u16::try_from(advertisement.resources.len().saturating_add(1))
            .map_err(|_| HistoryInstallationRefusal::Advertisement)?;
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: capability_capacity,
            maximum_advertised_resources: resource_capacity,
        })
        .map_err(|_| HistoryInstallationRefusal::Advertisement)?;
        registry
            .register(self.provider_entry(contract))
            .map_err(|_| HistoryInstallationRefusal::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| HistoryInstallationRefusal::Advertisement)
    }

    /// Native ownership metadata for another existing selected Host executor.
    /// This does not issue possession or execute a read.
    pub fn native_registration(
        &self,
        contract: &HistoryReadContract,
    ) -> (HistoryNativeIdentity, HistoryAttachment, BaseProviderEntry) {
        (
            self.identity.clone(),
            self.attachment,
            self.provider_entry(contract),
        )
    }

    fn provider_entry(&self, contract: &HistoryReadContract) -> BaseProviderEntry {
        let kind = contract.kind();
        let capability = conduit_core::capability_offer_from_parts! {
            semantic_contract: kind.semantic_contract(),
            startup_parameters: kind.startup_parameters.clone(),
            shorthand: kind.shorthand.clone(),
            capability_id: CapabilityId::from("environment/signed-history-read@1"),
            kind_id: kind.kind_id.clone(),
            kind_contract_revision: kind.kind_contract_revision.clone(),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(HISTORY_EXECUTION_PROFILE),
                implementation_id: ImplementationId::from(HISTORY_IMPLEMENTATION),
                artifact_id: self.identity.artifact_id.clone(),
            },
            inputs: kind.inputs.clone(),
            outputs: kind.outputs.clone(),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(HISTORY_CALL),
                target_kind: Some(kind.kind_id.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: HISTORY_MAXIMUM_BYTES,
                maximum_output_bytes: HISTORY_MAXIMUM_BYTES,
            }],
            resource_requirements: vec![resource_requirement(HISTORY_ATTACHMENT_CLASS, 1)],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(HISTORY_AUTHORITY),
                host_call_contract_id: HostCallContractId::from(HISTORY_CALL),
                subject_kind: kind.kind_id.clone(),
            }],
            limits: kind.limits.clone(),
        };
        let resource = resource_offer(
            self.identity.resource_pool_id.as_str(),
            HISTORY_ATTACHMENT_CLASS,
            1,
        );
        BaseProviderEntry {
            base_id: self.identity.base_id.clone(),
            provider_instance_id: self.identity.provider_instance_id.clone(),
            provider_generation: self.identity.provider_generation,
            implementation_id: BaseImplementationId::from("environment.base/signed-history@1"),
            mechanism_family: HostBaseKindId::from(HISTORY_ATTACHMENT_CLASS),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![capability],
            resources: vec![resource],
        }
    }

    /// Perform only the raw native read after an independently selected executor
    /// has validated the canonical request and authorized its exact operation.
    /// # Safety
    /// Caller must hold that actual current scoped operation authorization and
    /// retain this owner for its full lifetime. Metadata or an observer receipt
    /// cannot establish authorization. This method creates no capability.
    pub unsafe fn read_after_native_authorization(
        &mut self,
        cursor: u32,
        output: &mut [u8; MAXIMUM_SIGNED_BATCH_BYTES],
    ) -> Result<usize, super::HistoryProviderRefusal> {
        self.provider.read_signed_batch(cursor, output)
    }

    /// Consume native ownership into the exact selected Host Call.
    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        selection: HistoryCallSelection<'_>,
    ) -> Result<HistoryHostCall<P>, HistoryCallRefusal> {
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement)
            .ok_or(HistoryCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(HistoryCallRefusal::WrongBinding)?;
        if claim.host_id != self.identity.host_id
            || claim.boot_id != self.identity.boot_id
            || claim.base_instance_id != self.identity.provider_instance_id
            || claim.base_provider_generation != self.identity.provider_generation
            || claim.resource_pool_id != self.identity.resource_pool_id
            || claim.resource_generation_id != self.identity.resource_generation_id
            || claim.envelope_id != self.identity.envelope_id
            || claim.implementation_id.as_str() != HISTORY_IMPLEMENTATION
            || gear.artifact_id != self.identity.artifact_id
            || gear.execution_profile_id.as_str() != HISTORY_EXECUTION_PROFILE
            || gear.capability_id.as_str() != "environment/signed-history-read@1"
            || gear.limits != selection.contract.kind().limits
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == HISTORY_AUTHORITY
                    && authority.host_call_contract_id.as_str() == HISTORY_CALL
                    && authority.subject_kind == selection.contract.kind().kind_id
            })
            || base.base_id != self.identity.base_id
            || base.implementation_id.as_str() != "environment.base/signed-history@1"
            || base.mechanism_family.as_str() != HISTORY_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(HistoryCallRefusal::WrongBinding);
        }
        // SAFETY: new retained actual native ownership; the exact claim above
        // preserves its host, boot, provider, resource generation and envelope.
        unsafe {
            HistoryHostCall::bind_selected(
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
