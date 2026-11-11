//! Original initialized clock custody, independently retained before assignment.
use super::owner::{ClockCallRefusal, MonotonicDeadlineProvider};
use alloc::string::String;
use conduit_core::*;

/// Physical/native facts are not decoded from incoming assignment records.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClockAtNativeIdentity {
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
    pub capability_id: CapabilityId,
    pub execution_profile_id: ExecutionProfileId,
    pub authority_contract_id: AuthorityContractId,
    pub mechanism_family: String,
    pub resource_class: String,
}
/// Explicit selected resource extent, not an inferred allocator grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockAtAttachment {
    pub resource_bytes: u64,
}
pub struct ReadyClockAtBase<P> {
    pub(super) identity: ClockAtNativeIdentity,
    pub(super) attachment: ClockAtAttachment,
    pub(super) provider: P,
}
impl<P: MonotonicDeadlineProvider> ReadyClockAtBase<P> {
    /// # Safety
    /// Root owns this original provider, exact generation, finite calibrated
    /// millisecond basis and immutable profile facts. It admits provider and
    /// identity backing separately. Discovery/assignment supplies no possession.
    pub unsafe fn new(
        identity: ClockAtNativeIdentity,
        attachment: ClockAtAttachment,
        provider: P,
    ) -> Result<Self, ClockCallRefusal> {
        let texts = [
            identity.host_id.as_str(),
            identity.boot_id.as_str(),
            identity.base_id.as_str(),
            identity.provider_instance_id.as_str(),
            identity.resource_pool_id.as_str(),
            identity.resource_generation_id.0.as_str(),
            identity.envelope_id.as_str(),
            identity.artifact_id.as_str(),
            identity.implementation_id.as_str(),
            identity.base_implementation_id.as_str(),
            identity.capability_id.as_str(),
            identity.execution_profile_id.as_str(),
            identity.authority_contract_id.as_str(),
            identity.mechanism_family.as_str(),
            identity.resource_class.as_str(),
        ];
        if texts.iter().any(|text| text.is_empty())
            || identity.provider_generation == 0
            || attachment.resource_bytes == 0
        {
            return Err(ClockCallRefusal::WrongBinding);
        }
        Ok(Self {
            identity,
            attachment,
            provider,
        })
    }
    pub fn identity(&self) -> &ClockAtNativeIdentity {
        &self.identity
    }
}
