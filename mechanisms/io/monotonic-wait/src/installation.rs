//! Retained initialized timer slot, offered before planning and transferred once.
#[path = "assigned.rs"]
mod assigned;
use crate::*;
use alloc::vec;
use conduit_core::*;
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;

pub const WAIT_CAPABILITY: &str = "conduit/monotonic-wait@1";
pub const WAIT_EXECUTION_PROFILE: &str = "conduit/monotonic-wait-cooperative-bounded@1";
pub const WAIT_ATTACHMENT_CLASS: &str = "machine/monotonic-wait-attachment";
pub const WAIT_AUTHORITY: &str = "conduit.authority/monotonic-wait@1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WaitNativeIdentity {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub base_id: HostBaseId,
    pub provider_instance_id: BaseInstanceId,
    pub provider_generation: u64,
    pub resource_pool_id: ResourcePoolId,
    pub resource_generation_id: ResourceGenerationId,
    pub envelope_id: CapabilityEnvelopeId,
    pub artifact_id: ArtifactId,
    pub implementation_id: ImplementationId,
    pub base_implementation_id: BaseImplementationId,
}
pub struct ReadyMonotonicWaitBase<P> {
    identity: WaitNativeIdentity,
    attachment: WaitAttachment,
    provider: P,
}
#[derive(Debug, PartialEq, Eq)]
pub enum WaitInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: MonotonicWaitProvider> ReadyMonotonicWaitBase<P> {
    /// Retain one actual initialized monotonic timer slot before advertising it.
    ///
    /// # Safety
    /// Root must own this exact initialized provider, finite timer slot and
    /// generation. Waiting must terminate within its admitted duration/work
    /// and drop pending work on cancellation. A descriptor is not possession.
    pub unsafe fn new(
        identity: WaitNativeIdentity,
        attachment: WaitAttachment,
        provider: P,
    ) -> Result<Self, WaitInstallationRefusal> {
        if [
            identity.host_id.as_str(),
            identity.boot_id.as_str(),
            identity.base_id.as_str(),
            identity.provider_instance_id.as_str(),
            identity.resource_pool_id.as_str(),
            &identity.resource_generation_id.0,
            identity.envelope_id.as_str(),
            identity.artifact_id.as_str(),
            identity.implementation_id.as_str(),
            identity.base_implementation_id.as_str(),
        ]
        .iter()
        .any(|s| s.is_empty())
            || identity.provider_generation == 0
            || attachment.generation == 0
            || attachment.resource_bytes == 0
        {
            return Err(WaitInstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            attachment,
            provider,
        })
    }

    /// Project only the actual retained provider into the matching Host/Boot.
    /// Preparation may allocate; this method starts no timer work.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
    ) -> Result<(), WaitInstallationRefusal> {
        use WaitInstallationRefusal as R;
        let identity = &self.identity;
        if advertisement.host_id != identity.host_id || advertisement.boot_id != identity.boot_id {
            return Err(R::WrongBoot);
        }
        if advertisement.bases.iter().any(|base| {
            base.base_id == identity.base_id
                || base.provider_instance_id == identity.provider_instance_id
        }) || advertisement
            .capabilities
            .iter()
            .any(|c| c.capability_id.as_str() == WAIT_CAPABILITY)
            || advertisement
                .resources
                .iter()
                .any(|r| r.pool_id == identity.resource_pool_id)
        {
            return Err(R::Advertisement);
        }
        let kind = conduit_time::tick_semantic_contract();
        let offer = BackOfferBuilder::new(
            kind.clone(),
            Back {
                capability_id: WAIT_CAPABILITY.into(),
                execution_profile_id: WAIT_EXECUTION_PROFILE.into(),
                implementation_id: identity.implementation_id.clone(),
                artifact_id: identity.artifact_id.clone(),
                host_calls: vec![HostCallRequirement {
                    contract_id: WAIT_HOST_CALL_CONTRACT.into(),
                    target_kind: Some(kind.kind_id.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: PARAMETER_BYTES,
                    maximum_output_bytes: 0,
                }],
                resource_requirements: vec![resource_requirement(WAIT_ATTACHMENT_CLASS, 1)],
                authority_requirements: vec![AuthorityRequirement {
                    contract_id: WAIT_AUTHORITY.into(),
                    host_call_contract_id: WAIT_HOST_CALL_CONTRACT.into(),
                    subject_kind: kind.kind_id.clone(),
                }],
            },
        )
        .build();
        let mut registry = BaseRegistry::new(BaseRegistryLimits {
            maximum_bases: 1,
            maximum_capabilities_per_base: 1,
            maximum_resources_per_base: 1,
            maximum_advertised_capabilities: u16::try_from(
                advertisement.capabilities.len().saturating_add(1),
            )
            .map_err(|_| R::Advertisement)?,
            maximum_advertised_resources: u16::try_from(
                advertisement.resources.len().saturating_add(1),
            )
            .map_err(|_| R::Advertisement)?,
        })
        .map_err(|_| R::Advertisement)?;
        registry
            .register(BaseProviderEntry {
                base_id: identity.base_id.clone(),
                provider_instance_id: identity.provider_instance_id.clone(),
                provider_generation: identity.provider_generation,
                implementation_id: identity.base_implementation_id.clone(),
                mechanism_family: WAIT_ATTACHMENT_CLASS.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    identity.resource_pool_id.as_str(),
                    WAIT_ATTACHMENT_CLASS,
                    1,
                )],
            })
            .map_err(|_| R::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| R::Advertisement)
    }

    /// Preflight every native and selected identity before taking the owner.
    /// A refusal leaves the actual provider in Root's retained slot.
    pub fn bind_selected_retained(
        slot: &mut Option<Self>,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        configuration: &[ConfigurationEntry],
        selection: SelectedOperationPlan<'_>,
    ) -> Result<MonotonicWaitHostCall<P>, WaitCallRefusal> {
        let owner = slot.as_ref().ok_or(WaitCallRefusal::Possession)?;
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == selection.placement_id)
            .ok_or(WaitCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(WaitCallRefusal::WrongBinding)?;
        let identity = &owner.identity;
        if claim.host_id != identity.host_id
            || claim.boot_id != identity.boot_id
            || claim.base_instance_id != identity.provider_instance_id
            || claim.base_provider_generation != identity.provider_generation
            || claim.resource_pool_id != identity.resource_pool_id
            || claim.resource_generation_id != identity.resource_generation_id
            || claim.envelope_id != identity.envelope_id
            || claim.implementation_id != identity.implementation_id
            || gear.artifact_id != identity.artifact_id
            || gear.execution_profile_id.as_str() != WAIT_EXECUTION_PROFILE
            || gear.capability_id.as_str() != WAIT_CAPABILITY
            || base.base_id != identity.base_id
            || base.implementation_id != identity.base_implementation_id
            || base.mechanism_family.as_str() != WAIT_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
            || !gear.authority.iter().any(|authority| {
                authority.contract_id.as_str() == WAIT_AUTHORITY
                    && authority.host_call_contract_id.as_str() == WAIT_HOST_CALL_CONTRACT
                    && authority.subject_kind.as_str() == conduit_time::TICK_KIND
            })
        {
            return Err(WaitCallRefusal::WrongBinding);
        }
        let node = crate::owner::checked_binding(
            &table,
            &claim,
            owner.attachment,
            configuration,
            selection,
        )?;
        let owner = slot.take().expect("preflighted retained timer");
        Ok(MonotonicWaitHostCall::from_bound(
            table,
            handle,
            claim,
            owner.attachment,
            owner.provider,
            node,
        ))
    }
    pub fn native_identity(&self) -> &WaitNativeIdentity {
        &self.identity
    }
}
