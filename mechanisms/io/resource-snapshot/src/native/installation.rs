//! Actual initialized provider custody, independent of incoming descriptors.
use super::*;
use crate::{nor::PreparedNorSnapshotWriter, PreparedSnapshotRecord, SnapshotRecordSelection};
use alloc::vec::Vec;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotNativeIdentity {
    pub host_id: c::HostId,
    pub boot_id: c::BootId,
    pub base_id: c::HostBaseId,
    pub provider_instance_id: c::BaseInstanceId,
    pub provider_generation: u64,
    pub resource_pool_id: c::ResourcePoolId,
    pub resource_generation_id: c::ResourceGenerationId,
    pub envelope_id: c::CapabilityEnvelopeId,
    pub artifact_id: c::ArtifactId,
    pub implementation_id: c::ImplementationId,
    pub base_implementation_id: c::BaseImplementationId,
}
pub struct ReadySnapshotPublisher<S> {
    pub(super) identity: SnapshotNativeIdentity,
    pub(super) provider: PreparedNorSnapshotWriter<S>,
    pub(super) record: PreparedSnapshotRecord,
    pub(super) reference: Vec<u8>,
    pub(super) authority: c::AuthorityBinding,
}
impl<S: NorStorage> ReadySnapshotPublisher<S> {
    /// Retain the actual exclusively owned finite slot before advertising it.
    /// # Safety
    /// Root owns this exact provider/generation and lawful bounded synchronous
    /// flash windows excluding timing-sensitive in-flight device operations; XIP
    /// safety alone does not establish that exclusion. Provider inline/backing/work and all preparation workspace
    /// must be funded independently. Original grants come from retained local
    /// authority, not from the selected/incoming descriptors. No write happens.
    pub unsafe fn new(
        identity: SnapshotNativeIdentity,
        provider: PreparedNorSnapshotWriter<S>,
        selected: SnapshotRecordSelection<'_>,
        reference: &c::BoundedResourceRef,
        original: &c::AuthorityGrant,
        local: &c::ResourceReferenceBinding,
    ) -> Result<Self, SnapshotCallRefusal> {
        use SnapshotCallRefusal as E;
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
        {
            return Err(E::InvalidIdentity);
        }
        let content = selected.binding.content.as_ref().ok_or(E::WrongBinding)?;
        if selected.host != &identity.host_id
            || selected.boot != &identity.boot_id
            || selected.kind.as_str() != SNAPSHOT_KIND
            || selected.capability.as_str() != SNAPSHOT_CAPABILITY
            || selected.binding.pool_id != identity.resource_pool_id
            || content.base_id != identity.base_id
            || content.contract.access != c::ResourceAccessMode::WriteCandidatePublish
        {
            return Err(E::WrongBinding);
        }
        let record = PreparedSnapshotRecord::prepare_selected(selected, reference, original, local)
            .map_err(E::Record)?;
        if !provider.fits_record(record.record_bytes()) {
            return Err(E::Media(NorRefusal::Capacity));
        }
        let reference = reference.encode().map_err(|_| E::WrongBinding)?;
        Ok(Self {
            identity,
            provider,
            record,
            reference,
            authority: selected.authority.clone(),
        })
    }
    /// Return sole media custody before binding or after revoked retirement.
    /// No live Base operation, erase permission or new grant is returned.
    pub fn into_provider(self) -> PreparedNorSnapshotWriter<S> {
        self.provider
    }
    pub fn native_identity(&self) -> &SnapshotNativeIdentity {
        &self.identity
    }
    pub fn slot_bytes(&self) -> usize {
        self.provider.slot_bytes()
    }
    pub fn maximum_publish_work(&self) -> u64 {
        self.provider.maximum_publish_calls() as u64
    }
    pub(super) fn authority(&self) -> &c::AuthorityBinding {
        &self.authority
    }
    /// Codec/identity/reference heap payload; inline Self/provider backing and
    /// allocator overhead remain separate mandatory Root charges.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let base = self
            .record
            .retained_backing_bytes()?
            .checked_add(self.reference.capacity())?;
        [
            &self.authority.grant_id.0,
            &self.authority.contract_id.0,
            &self.authority.host_call_contract_id.0,
            &self.authority.subject_kind.0,
            &self.authority.host_id.0,
            &self.authority.boot_id.0,
            &self.authority.capability_id.0,
            &self.identity.host_id.0,
            &self.identity.boot_id.0,
            &self.identity.base_id.0,
            &self.identity.provider_instance_id.0,
            &self.identity.resource_pool_id.0,
            &self.identity.resource_generation_id.0,
            &self.identity.envelope_id.0,
            &self.identity.artifact_id.0,
            &self.identity.implementation_id.0,
            &self.identity.base_implementation_id.0,
        ]
        .iter()
        .try_fold(base, |bytes, text| bytes.checked_add(text.capacity()))
    }
}
