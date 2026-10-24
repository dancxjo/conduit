//! Retain actual initialized electrical ownership before advertising readiness.
//! A descriptor, assignment or successful probe does not create this owner.
use crate::{owner::*, *};
use conduit_core::*;
#[path = "installation_advertisement.rs"]
mod advertisement;
#[path = "assigned.rs"]
mod assigned;
#[path = "installation_selected.rs"]
mod selected;
pub const WRITE_CAPABILITY: &str = "conduit/tm1637-write@1";
pub const WRITE_EXECUTION_PROFILE: &str = "conduit/tm1637-cooperative-bounded@1";
pub const WRITE_ATTACHMENT_CLASS: &str = "machine/tm1637-attachment";
pub const WRITE_AUTHORITY: &str = "conduit.authority/tm1637-write@1";
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteNativeIdentity {
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
pub struct ReadyWriteBase<P> {
    identity: WriteNativeIdentity,
    attachment: WriteAttachment,
    provider: P,
}
#[derive(Debug, PartialEq, Eq)]
pub enum InstallationRefusal {
    InvalidIdentity,
    WrongBoot,
    Advertisement,
}
impl<P: OpenDrain> ReadyWriteBase<P> {
    /// # Safety
    /// Root must own exactly these initialized pins, resource and attachment
    /// generation. Every synchronous edge and transaction must terminate inside
    /// its separately admitted work/time bound. Dropping the provider must release
    /// its pins; a failed provider cannot guarantee physical electrical cleanup.
    pub unsafe fn new(
        identity: WriteNativeIdentity,
        attachment: WriteAttachment,
        provider: P,
    ) -> Result<Self, InstallationRefusal> {
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
        .any(|id| id.is_empty())
            || identity.provider_generation == 0
            || !attachment.valid()
        {
            return Err(InstallationRefusal::InvalidIdentity);
        }
        Ok(Self {
            identity,
            attachment,
            provider,
        })
    }
    pub fn native_identity(&self) -> &WriteNativeIdentity {
        &self.identity
    }
    fn check_claim(&self, claim: &BaseOperationClaim) -> Result<(), WriteCallRefusal<P::Error>> {
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
            return Err(WriteCallRefusal::WrongBinding);
        }
        Ok(())
    }
}
