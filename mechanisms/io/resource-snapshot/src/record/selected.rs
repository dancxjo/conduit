//! Compact semantic preparation; Root separately proves exact assigned selection,
//! admission, current capability possession, Start and safe provider execution.
use super::*;
use conduit_core::{
    AuthorityGrant, BootId, CapabilityId, HostCallRequirement, HostId, KindId, ResourceBinding,
    ResourceDereferenceRequirement, ResourceReferenceBinding,
};

/// Original selected facts, borrowed without constructing a synthetic PlannedGear.
/// These public descriptors are not evidence of admission or possession.
#[derive(Clone, Copy)]
pub struct SnapshotRecordSelection<'a> {
    pub host: &'a HostId,
    pub boot: &'a BootId,
    pub kind: &'a KindId,
    pub capability: &'a CapabilityId,
    pub binding: &'a ResourceBinding,
    pub host_call: &'a HostCallRequirement,
    pub authority: &'a AuthorityBinding,
}
impl PreparedSnapshotRecord {
    /// The two local grant arguments must be independently retained by the
    /// trusted composition root, never reconstructed from incoming descriptors.
    /// Core checks exact reference identity/version, availability and ceilings;
    /// the original authority grant checks operation, subject and current Boot.
    /// No table revocation, active Play or flash-window witness is minted here.
    /// Preparation allocates bounded codec/authority/reference-check workspace;
    /// callers must fund it before execution. Existing rich prepare is unchanged
    /// in strength and does not invent these independent grants.
    pub fn prepare_selected(
        selected: SnapshotRecordSelection<'_>,
        reference: &BoundedResourceRef,
        original: &AuthorityGrant,
        local_reference: &ResourceReferenceBinding,
    ) -> Result<Self, SnapshotRefusal> {
        let authority = selected.authority;
        if original.host_id != *selected.host {
            return Err(SnapshotRefusal::ForeignHost);
        }
        if original.boot_id != *selected.boot {
            return Err(SnapshotRefusal::StaleBoot);
        }
        if original.grant_id != authority.grant_id
            || original.contract_id != authority.contract_id
            || original.host_call_contract_id != authority.host_call_contract_id
            || original.subject_kind != authority.subject_kind
            || original.host_id != authority.host_id
            || original.boot_id != authority.boot_id
            || original.capability_id != authority.capability_id
            || local_reference.authority_grant != original.grant_id
        {
            return Err(SnapshotRefusal::AuthorityDenied);
        }
        let content = selected
            .binding
            .content
            .as_ref()
            .ok_or(SnapshotRefusal::InvalidBinding)?;
        let required = ResourceDereferenceRequirement {
            content_profile: content.contract.content_profile.clone(),
            access_class: selected.binding.class_id.clone(),
            authority_contract: original.contract_id.clone(),
            maximum_bytes: u64::from(content.contract.maximum_bytes),
            maximum_items: Some(u64::from(content.contract.maximum_items)),
        };
        required
            .admit(reference, local_reference)
            .map_err(SnapshotRefusal::LocalReference)?;
        Self::prepare_parts(selected, reference)
    }
}

#[cfg(test)]
mod tests;
