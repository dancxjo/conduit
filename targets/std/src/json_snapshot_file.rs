//! One explicitly selected immutable JSON generation; no inventory or ACK policy.
use conduit_core::{
    AuthorityBinding, AuthorityGrant, BoundedResourceRef, ConfigurationValue, PlannedGear,
    ResourceReferenceBinding, RUNTIME_MEMORY_RESOURCE_CLASS,
};
use conduit_linux_snapshot_store::{
    FileSnapshotRefusal, PreparedFileSnapshot, SnapshotDirectory, SnapshotRecordSelection,
};
use conduit_std_offers::*;

#[derive(Debug)]
pub enum JsonSnapshotFileRefusal {
    InvalidBinding,
    WrongReference,
    InvalidJson,
    Unavailable,
    File(FileSnapshotRefusal),
}
impl From<FileSnapshotRefusal> for JsonSnapshotFileRefusal {
    fn from(value: FileSnapshotRefusal) -> Self {
        Self::File(value)
    }
}
fn validate_reference(reference: &BoundedResourceRef) -> Result<(), JsonSnapshotFileRefusal> {
    reference
        .validate()
        .map_err(|_| JsonSnapshotFileRefusal::WrongReference)?;
    if reference.content_profile.as_str() != "text/json-utf8@1"
        || reference.access_class.as_str() != JSON_SNAPSHOT_ACCESS_CLASS
        || !(1..=u64::from(JSON_SNAPSHOT_MAXIMUM_BYTES)).contains(&reference.extent.bytes)
        || reference.extent.items != Some(1)
        || reference.lifetime.expires_at.is_some()
    {
        return Err(JsonSnapshotFileRefusal::WrongReference);
    }
    Ok(())
}
/// Root-owned directory FD and descriptive exact reference. Construction grants
/// no write/read authority, disk reservation, Host offer or operation admission.
pub struct SelectedJsonSnapshotResidence {
    pub(crate) directory: SnapshotDirectory,
    pub(crate) reference: BoundedResourceRef,
    pub(crate) original: AuthorityGrant,
    pub(crate) local_reference: ResourceReferenceBinding,
}
impl SelectedJsonSnapshotResidence {
    pub fn prepare(
        directory: std::fs::File,
        reference: BoundedResourceRef,
        original: AuthorityGrant,
        local_reference: ResourceReferenceBinding,
    ) -> Result<Self, JsonSnapshotFileRefusal> {
        validate_reference(&reference)?;
        let directory = SnapshotDirectory::prepare(directory)
            .map_err(|_| JsonSnapshotFileRefusal::InvalidBinding)?;
        Ok(Self {
            directory,
            reference,
            original,
            local_reference,
        })
    }
    pub fn directory(&self) -> &SnapshotDirectory {
        &self.directory
    }
    pub fn reference(&self) -> &BoundedResourceRef {
        &self.reference
    }
    pub fn authority(&self) -> &AuthorityGrant {
        &self.original
    }
    pub fn local_reference(&self) -> &ResourceReferenceBinding {
        &self.local_reference
    }
    pub fn prepare_operation(
        &self,
        placement: &PlannedGear,
    ) -> Result<PreparedJsonSnapshot<'_>, JsonSnapshotFileRefusal> {
        PreparedJsonSnapshot::prepare(self, placement)
    }
}
/// One selected operation. Preparation owns finite file/record buffers; Root
/// separately admits all backing, current operation authority and effect work.
pub struct PreparedJsonSnapshot<'a> {
    file: PreparedFileSnapshot<'a>,
    reference: Vec<u8>,
    write: bool,
    retired: bool,
}
impl<'a> PreparedJsonSnapshot<'a> {
    pub fn prepare(
        selection: &'a SelectedJsonSnapshotResidence,
        placement: &PlannedGear,
    ) -> Result<Self, JsonSnapshotFileRefusal> {
        use JsonSnapshotFileRefusal as E;
        let reference = &selection.reference;
        validate_reference(reference)?;
        let [first, second] = placement.resources.as_slice() else {
            return Err(E::InvalidBinding);
        };
        let (resource, memory) = if first.class_id.as_str() == JSON_SNAPSHOT_ACCESS_CLASS
            && second.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS
        {
            (first, second)
        } else if second.class_id.as_str() == JSON_SNAPSHOT_ACCESS_CLASS
            && first.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS
        {
            (second, first)
        } else {
            return Err(E::InvalidBinding);
        };
        let content = resource.content.as_ref().ok_or(E::InvalidBinding)?;
        if memory.pool_id == resource.pool_id
            || memory.class_id.as_str() != RUNTIME_MEMORY_RESOURCE_CLASS
            || memory.units != JSON_SNAPSHOT_PREPARATION_BYTES
            || memory.protected.is_some()
            || memory.content.is_some()
            || memory.compute.is_some()
        {
            return Err(E::InvalidBinding);
        }
        let original = &selection.original;
        let local = &selection.local_reference;
        let authority = match placement.authority.as_slice() {
            [authority] => authority,
            _ => return Err(E::InvalidBinding),
        };
        if [
            placement.host_id.as_str(),
            placement.boot_id.as_str(),
            placement.kind_id.as_str(),
            placement.capability_id.as_str(),
            resource.class_id.as_str(),
            content.base_id.as_str(),
            content.residence_profile.as_str(),
            content.owner_host.as_str(),
            content.owner_boot.as_str(),
            reference.content_profile.as_str(),
            reference.access_class.as_str(),
            authority.grant_id.as_str(),
            authority.contract_id.as_str(),
            authority.host_call_contract_id.as_str(),
            authority.subject_kind.as_str(),
            authority.host_id.as_str(),
            authority.boot_id.as_str(),
            authority.capability_id.as_str(),
            original.grant_id.as_str(),
            original.contract_id.as_str(),
            original.host_call_contract_id.as_str(),
            original.subject_kind.as_str(),
            original.host_id.as_str(),
            original.boot_id.as_str(),
            original.capability_id.as_str(),
            local.content_profile.as_str(),
            local.access_class.as_str(),
            local.handle.as_str(),
            local.authority_contract.as_str(),
            local.authority_grant.as_str(),
        ]
        .iter()
        .any(|text| text.is_empty() || text.len() > JSON_SNAPSHOT_MAXIMUM_ID_BYTES)
        {
            return Err(E::InvalidBinding);
        }

        if resource.units != 1
            || resource.protected.is_some()
            || resource.compute.is_some()
            || content.base_id.as_str() != JSON_SNAPSHOT_BASE
            || content.residence_profile.as_str() != JSON_SNAPSHOT_RESIDENCE
        {
            return Err(E::InvalidBinding);
        }
        let offer = json_snapshot_offer(content.contract.clone()).map_err(|_| E::InvalidBinding)?;
        if placement.semantic_contract != offer.semantic_contract
            || placement.kind_id != offer.kind_id
            || placement.kind_contract_revision != offer.kind_contract_revision
            || placement.implementation_id != offer.implementation.implementation_id
            || placement.execution_profile_id != offer.implementation.execution_profile_id
            || placement.artifact_id != offer.implementation.artifact_id
            || placement.capability_id != offer.capability_id
            || placement.host_calls != offer.host_calls
            || placement.inputs != offer.inputs
            || placement.outputs != offer.outputs
            || placement.limits != offer.limits
        {
            return Err(E::InvalidBinding);
        }
        let [configured] = placement.configuration.as_slice() else {
            return Err(E::InvalidBinding);
        };
        let ConfigurationValue::Text(hex) = &configured.value else {
            return Err(E::InvalidBinding);
        };
        if configured.key != "reference" {
            return Err(E::InvalidBinding);
        }
        if hex.is_empty()
            || hex.len() > 1024
            || hex.len() % 2 != 0
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(E::WrongReference);
        }
        let encoded =
            conduit_data::bytes_literal::decode_hex(hex).map_err(|_| E::WrongReference)?;
        let decoded = BoundedResourceRef::decode(&encoded).map_err(|_| E::WrongReference)?;
        if &decoded != reference || decoded.encode().map_err(|_| E::WrongReference)? != encoded {
            return Err(E::WrongReference);
        }
        Ok(Self {
            file: PreparedFileSnapshot::prepare_selected(
                &selection.directory,
                SnapshotRecordSelection {
                    host: &placement.host_id,
                    boot: &placement.boot_id,
                    kind: &placement.kind_id,
                    capability: &placement.capability_id,
                    binding: resource,
                    host_call: placement.host_calls.first().ok_or(E::InvalidBinding)?,
                    authority: match placement.authority.as_slice() {
                        [authority] => authority,
                        _ => return Err(E::InvalidBinding),
                    },
                },
                reference,
                &selection.original,
                &selection.local_reference,
            )?,
            reference: encoded,
            write: content.contract.access
                == conduit_core::ResourceAccessMode::WriteCandidatePublish,
            retired: false,
        })
    }
    /// Returns the exact canonical reference only after actual file/directory
    /// publication succeeds. This is not kernel output commit or producer ACK.
    pub fn publish(
        &mut self,
        authority: &AuthorityBinding,
        json: &[u8],
    ) -> Result<&[u8], JsonSnapshotFileRefusal> {
        if self.retired {
            return Err(JsonSnapshotFileRefusal::Unavailable);
        }
        if !self.write {
            return Err(JsonSnapshotFileRefusal::InvalidBinding);
        }
        conduit_web::validate_json_text(json).map_err(|_| JsonSnapshotFileRefusal::InvalidJson)?;
        self.file.publish(authority, json)?;
        Ok(&self.reference)
    }
    pub fn read(
        &mut self,
        authority: &AuthorityBinding,
        reference: &[u8],
    ) -> Result<&[u8], JsonSnapshotFileRefusal> {
        if self.retired {
            return Err(JsonSnapshotFileRefusal::Unavailable);
        }
        if self.write {
            return Err(JsonSnapshotFileRefusal::InvalidBinding);
        }
        if reference != self.reference {
            return Err(JsonSnapshotFileRefusal::WrongReference);
        }
        let json = self.file.read(authority)?;
        conduit_web::validate_json_text(json).map_err(|_| JsonSnapshotFileRefusal::InvalidJson)?;
        Ok(json)
    }
    pub fn retire(&mut self) {
        self.retired = true;
    }
}

impl JsonSnapshotFileRefusal {
    /// Stable finite effect diagnostics; uncertain publication stays distinct.
    pub(crate) fn detail(&self) -> u16 {
        use conduit_linux_snapshot_store::{FileSnapshotRefusal as F, IoStage};
        match self {
            Self::InvalidBinding => 10,
            Self::WrongReference => 11,
            Self::InvalidJson => 12,
            Self::Unavailable => 13,
            Self::File(F::Binding(_)) => 14,
            Self::File(F::InvalidKey | F::InvalidFile) => 15,
            Self::File(F::AlreadyAttempted) => 16,
            Self::File(F::CandidateNameExists) => 17,
            Self::File(F::PublishedNameExists) => 18,
            Self::File(F::Missing) => 19,
            Self::File(F::Io {
                attempt_may_have_published: true,
                ..
            }) => 29,
            Self::File(F::Io { stage, .. }) => {
                20 + match stage {
                    IoStage::CreateCandidate => 0,
                    IoStage::WriteCandidate => 1,
                    IoStage::SyncCandidate => 2,
                    IoStage::PublishLink => 3,
                    IoStage::RemoveCandidate => 4,
                    IoStage::SyncDirectory => 5,
                    IoStage::OpenRead => 6,
                    IoStage::Read => 7,
                }
            }
        }
    }
}
