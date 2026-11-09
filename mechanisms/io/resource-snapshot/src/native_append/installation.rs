//! Initialized affine residence owner; no incoming descriptor grants possession.
use super::*;
pub use crate::native::SnapshotNativeIdentity as AppendNativeIdentity;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppendAttachment {
    pub residence: RecordResidenceId,
    pub resource_bytes: u32,
    pub slots: u32,
    pub remaining_slots: u32,
    pub maximum_operations: u32,
    pub maximum_append_media_calls: u32,
    pub maximum_recovery_reads: u64,
}
pub struct ReadyRecordAppender<S> {
    pub(super) identity: AppendNativeIdentity,
    pub(super) provider: PreparedNorRecordResidence<S>,
    pub(super) original: c::AuthorityGrant,
    pub(super) maximum_operations: u32,
    pub(super) encoder: result::Encoder,
}
impl<S: NorStorage> ReadyRecordAppender<S> {
    /// Retain the actual recovered provider before advertisement/Start. No I/O.
    /// # Safety
    /// Root owns the exact exclusive residence/generation and independently
    /// authorizes this original current-Boot grant. It must fund all inline and
    /// backing bytes, the whole physical extent, finite scan and operation work,
    /// and lawful synchronous windows excluding in-flight timing-sensitive IO.
    /// Recovery's gap/damage report must remain observable. Only actual kernel
    /// acceptance may call acknowledge_kernel_completion; media is not that proof.
    pub unsafe fn new(
        identity: AppendNativeIdentity,
        provider: PreparedNorRecordResidence<S>,
        original: &c::AuthorityGrant,
        maximum_operations: u32,
    ) -> Result<Self, AppendCallRefusal> {
        use AppendCallRefusal as E;
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
        {
            return Err(E::InvalidIdentity);
        }
        if original.grant_id.as_str().is_empty()
            || original.host_id != identity.host_id
            || original.boot_id != identity.boot_id
            || original.contract_id.as_str() != APPEND_AUTHORITY
            || original.host_call_contract_id.as_str() != APPEND_OPERATION
            || original.subject_kind.as_str() != APPEND_KIND
            || original.capability_id.as_str() != APPEND_CAPABILITY
        {
            return Err(E::WrongBinding);
        }
        if maximum_operations == 0 {
            return Err(E::InvalidIdentity);
        }
        if !provider.recovered() {
            return Err(E::Media(RecordResidenceRefusal::RecoveryRequired));
        }
        Ok(Self {
            identity,
            provider,
            original: original.clone(),
            maximum_operations,
            encoder: result::Encoder::new().map_err(|_| E::Possession)?,
        })
    }
    /// Actual provider recovery state, including after revoked retirement.
    /// This is not a grant or evidence of resource/Play admission.
    pub fn recovered(&self) -> bool {
        self.provider.recovered()
    }
    pub fn native_identity(&self) -> &AppendNativeIdentity {
        &self.identity
    }
    pub fn attachment(&self) -> AppendAttachment {
        AppendAttachment {
            residence: self.provider.residence_id(),
            resource_bytes: self.provider.residence_bytes(),
            slots: self.provider.total_slots(),
            maximum_operations: self.maximum_operations,
            remaining_slots: self.provider.remaining_slots().unwrap_or(0),
            maximum_append_media_calls: self.provider.maximum_append_media_calls(),
            maximum_recovery_reads: self.provider.maximum_recovery_reads(),
        }
    }
    pub fn into_provider(self) -> PreparedNorRecordResidence<S> {
        self.provider
    }
    /// Actual retained heap payload only; inline/provider backing and allocator
    /// overhead require separate admission charges.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        [
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
            &self.original.grant_id.0,
            &self.original.contract_id.0,
            &self.original.host_call_contract_id.0,
            &self.original.subject_kind.0,
            &self.original.host_id.0,
            &self.original.boot_id.0,
            &self.original.capability_id.0,
        ]
        .iter()
        .try_fold(self.encoder.backing()?, |n, s| n.checked_add(s.capacity()))
    }
}
