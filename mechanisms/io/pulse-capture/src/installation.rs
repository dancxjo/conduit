//! Ready native ownership advertised before planning, consumed by exact Play.
//! Follows the retained I2C installation lifecycle; no peripheral protocol lives here.
use crate::{owner::*, *};
use alloc::vec;
use conduit_core::*;
use conduit_plan_lowering::selected_operation::SelectedOperationPlan;

pub const PULSE_CAPABILITY: &str = "conduit/pulse-read@1";
pub const PULSE_EXECUTION_PROFILE: &str = "conduit/pulse-cooperative-bounded@1";
pub const PULSE_ATTACHMENT_CLASS: &str = "machine/pulse-attachment";
pub const PULSE_AUTHORITY: &str = "conduit.authority/pulse-read@1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PulseNativeIdentity {
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
pub struct ReadyPulseBase<P> {
    identity: PulseNativeIdentity,
    attachment: PulseAttachment,
    provider: P,
}
#[derive(Debug, PartialEq, Eq)]
pub enum PulseInstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: PulseReadProvider> ReadyPulseBase<P> {
    /// Retain exclusively owned electrical resources before advertising them.
    ///
    /// # Safety
    /// The native composition root must own exactly this provider, resource and
    /// attachment generation. The provider must quiesce on cancellation and
    /// enforce finite termination and mandatory electrical safety on failure.
    /// An authored pin name or successful sensor probe does not establish ownership.
    pub unsafe fn new(
        identity: PulseNativeIdentity,
        attachment: PulseAttachment,
        provider: P,
    ) -> Result<Self, PulseInstallationRefusal> {
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
            || !attachment.valid()
        {
            return Err(PulseInstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            attachment,
            provider,
        })
    }

    /// Project only the actual retained provider into the matching Host/Boot.
    /// Preparation may allocate; this method performs no electrical work.
    pub fn append_to_advertisement(
        &self,
        advertisement: &mut HostAdvertisement,
        contract: &PulseReadContract,
    ) -> Result<(), PulseInstallationRefusal> {
        use PulseInstallationRefusal as R;
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
            .any(|c| c.capability_id.as_str() == PULSE_CAPABILITY)
            || advertisement
                .resources
                .iter()
                .any(|r| r.pool_id == identity.resource_pool_id)
        {
            return Err(R::Advertisement);
        }
        let kind = contract.kind();
        let offer = BackOfferBuilder::new(
            kind.clone(),
            Back {
                capability_id: PULSE_CAPABILITY.into(),
                execution_profile_id: PULSE_EXECUTION_PROFILE.into(),
                implementation_id: identity.implementation_id.clone(),
                artifact_id: identity.artifact_id.clone(),
                host_calls: vec![HostCallRequirement {
                    contract_id: PULSE_READ_CALL.into(),
                    target_kind: Some(kind.kind_id.clone()),
                    maximum_in_flight: 1,
                    maximum_input_bytes: PULSE_READ_MAXIMUM_BYTES,
                    maximum_output_bytes: PULSE_READ_MAXIMUM_BYTES,
                }],
                resource_requirements: vec![resource_requirement(PULSE_ATTACHMENT_CLASS, 1)],
                authority_requirements: vec![AuthorityRequirement {
                    contract_id: PULSE_AUTHORITY.into(),
                    host_call_contract_id: PULSE_READ_CALL.into(),
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
                mechanism_family: PULSE_ATTACHMENT_CLASS.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![resource_offer(
                    identity.resource_pool_id.as_str(),
                    PULSE_ATTACHMENT_CLASS,
                    1,
                )],
            })
            .map_err(|_| R::Advertisement)?;
        registry
            .project_ready_into(advertisement)
            .map_err(|_| R::Advertisement)
    }

    /// Consume this native owner into the exact selected operation.
    pub fn bind_selected(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PulseReadContract,
        selection: SelectedOperationPlan<'_>,
    ) -> Result<PulseHostCall<P>, PulseCallRefusal> {
        let gear = selection
            .fragment
            .placements
            .iter()
            .find(|g| &g.placement_id == selection.placement_id)
            .ok_or(PulseCallRefusal::WrongBinding)?;
        let base = gear.base.as_ref().ok_or(PulseCallRefusal::WrongBinding)?;
        let identity = &self.identity;
        if claim.host_id != identity.host_id
            || claim.boot_id != identity.boot_id
            || claim.base_instance_id != identity.provider_instance_id
            || claim.base_provider_generation != identity.provider_generation
            || claim.resource_pool_id != identity.resource_pool_id
            || claim.resource_generation_id != identity.resource_generation_id
            || claim.envelope_id != identity.envelope_id
            || claim.implementation_id != identity.implementation_id
            || gear.artifact_id != identity.artifact_id
            || gear.execution_profile_id.as_str() != PULSE_EXECUTION_PROFILE
            || gear.capability_id.as_str() != PULSE_CAPABILITY
            || gear.limits != contract.kind().limits
            || !gear.authority.iter().any(|a| {
                a.contract_id.as_str() == PULSE_AUTHORITY
                    && a.host_call_contract_id.as_str() == PULSE_READ_CALL
                    && a.subject_kind == contract.kind().kind_id
            })
            || base.base_id != identity.base_id
            || base.implementation_id != identity.base_implementation_id
            || base.mechanism_family.as_str() != PULSE_ATTACHMENT_CLASS
            || base.enforcement_class != BaseEnforcementClass::Cooperative
        {
            return Err(PulseCallRefusal::WrongBinding);
        }
        // SAFETY: new retained actual native ownership; the exact claim preserves
        // its Host/Boot, provider, resource generation and attachment envelope.
        unsafe {
            PulseHostCall::bind_selected(
                table,
                handle,
                claim,
                self.attachment,
                self.provider,
                contract,
                selection,
            )
        }
    }
    /// Consume actual native ownership under an independently authorized assigned
    /// Plan/Play. The selected native facts are checked against this owner.
    pub fn bind_assigned(
        self,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        contract: &PulseReadContract,
        plan: conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan<'_>,
    ) -> Result<PulseHostCall<P>, PulseCallRefusal> {
        let identity = &self.identity;
        if claim.host_id != identity.host_id
            || claim.boot_id != identity.boot_id
            || claim.base_instance_id != identity.provider_instance_id
            || claim.base_provider_generation != identity.provider_generation
            || claim.resource_pool_id != identity.resource_pool_id
            || claim.resource_generation_id != identity.resource_generation_id
            || claim.envelope_id != identity.envelope_id
            || claim.implementation_id != identity.implementation_id
        {
            return Err(PulseCallRefusal::WrongBinding);
        }
        let native = conduit_plan_lowering::selected_assigned_operation::AssignedNativeIdentity {
            capability: PULSE_CAPABILITY,
            execution_profile: PULSE_EXECUTION_PROFILE,
            artifact: identity.artifact_id.as_str(),
            base: identity.base_id.as_str(),
            base_implementation: identity.base_implementation_id.as_str(),
            mechanism: PULSE_ATTACHMENT_CLASS,
            authority: PULSE_AUTHORITY,
            resource_class: PULSE_ATTACHMENT_CLASS,
        };
        // SAFETY: new retained actual ownership; above preserves its native epoch,
        // resource generation and envelope. The common binder checks the selected
        // native facts and the local opaque capability before any electrical work.
        unsafe {
            PulseHostCall::bind_assigned(
                table,
                handle,
                claim,
                self.attachment,
                self.provider,
                crate::owner::AssignedPulseSelection {
                    contract,
                    native,
                    plan,
                },
            )
        }
    }
}
