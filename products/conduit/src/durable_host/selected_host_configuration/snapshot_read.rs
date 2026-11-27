//! Independent trusted Root authorization for one selected existing file mapping.
use conduit_core::*;
use conduit_linux_snapshot_store::{
    SnapshotDirectory, SnapshotRecordSelection, AUTHORITY_CONTRACT, MAXIMUM_SNAPSHOT_BYTES,
    READ_OPERATION,
};
use conduit_std_host::{environment_recording_history::CapturedSnapshotSelection, StdHostConfig};

const SNAPSHOT_ACCESS_CLASS: &str = "resource/snapshot@1";

pub(super) struct ReadBinding<'directory> {
    directory: &'directory SnapshotDirectory,
    host: HostId,
    boot: BootId,
    kind: KindId,
    capability: CapabilityId,
    reference: BoundedResourceRef,
    grant: AuthorityGrant,
    authority: AuthorityBinding,
    local: ResourceReferenceBinding,
    residence: ResourceBinding,
    call: HostCallRequirement,
}
impl<'directory> ReadBinding<'directory> {
    /// Only called after Root acquires actual selected directory custody and
    /// validates its local operator intent. Reference fields select the mapping;
    /// they do not determine the original operation/subject/Host/Boot grant.
    pub(super) fn authorize(
        config: &StdHostConfig,
        directory: &'directory SnapshotDirectory,
        reference: BoundedResourceRef,
    ) -> Result<Self, String> {
        if reference.access_class.as_str() != SNAPSHOT_ACCESS_CLASS {
            return Err("captured read exceeds the reviewed reference profile".into());
        }
        let host = config.host_id.clone();
        let boot = config.boot_id.clone();
        let kind = kind_id("resource/snapshot-read");
        let capability: CapabilityId = READ_OPERATION.into();
        let grant = AuthorityGrant {
            grant_id: format!("{}/environment-recording-read", boot.as_str()).into(),
            contract_id: AUTHORITY_CONTRACT.into(),
            host_call_contract_id: READ_OPERATION.into(),
            subject_kind: kind.clone(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            capability_id: capability.clone(),
        };
        let authority = AuthorityBinding {
            grant_id: grant.grant_id.clone(),
            contract_id: grant.contract_id.clone(),
            host_call_contract_id: grant.host_call_contract_id.clone(),
            subject_kind: grant.subject_kind.clone(),
            host_id: grant.host_id.clone(),
            boot_id: grant.boot_id.clone(),
            capability_id: grant.capability_id.clone(),
        };
        let local = ResourceReferenceBinding {
            identity: reference.identity,
            version: reference.lifetime.version,
            content_profile: reference.content_profile.clone(),
            access_class: SNAPSHOT_ACCESS_CLASS.into(),
            handle: format!("{}/environment-recording-file", boot.as_str()).into(),
            authority_contract: grant.contract_id.clone(),
            authority_grant: grant.grant_id.clone(),
            maximum_bytes: MAXIMUM_SNAPSHOT_BYTES as u64,
            maximum_items: Some(1),
            availability: ResourceReferenceAvailability::Available,
        };
        // This descriptor is a current-Boot read lease over actual held bytes.
        // Opening a directory establishes no durable storage Back or recovery.
        let residence = ResourceBinding {
            pool_id: format!("{}/environment-recording-read", boot.as_str()).into(),
            class_id: SNAPSHOT_ACCESS_CLASS.into(),
            units: 1,
            compute: None,
            protected: None,
            content: Some(ResourceContentOffer {
                contract: ResourceContentRequirement {
                    identity: reference.identity,
                    version: reference.lifetime.version,
                    content_profile: reference.content_profile.clone(),
                    maximum_bytes: MAXIMUM_SNAPSHOT_BYTES as u32,
                    maximum_items: 1,
                    retention: ResourceRetention::Boot,
                    sharing: ResourceSharing::SingleWriterPublished,
                    access: ResourceAccessMode::ReadPublished,
                    generation_slots: 1,
                    reader_leases: 1,
                    publication_slots: 0,
                    sensitive: true,
                },
                owner_host: host.clone(),
                owner_boot: boot.clone(),
                base_id: "root/selected-snapshot-directory".into(),
                residence_profile: kind_id("linux/snapshot@1"),
            }),
        };
        let call = HostCallRequirement {
            contract_id: READ_OPERATION.into(),
            target_kind: Some(kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
            maximum_output_bytes: MAXIMUM_SNAPSHOT_BYTES as u32,
        };
        Ok(Self {
            directory,
            host,
            boot,
            kind,
            capability,
            reference,
            grant,
            authority,
            local,
            residence,
            call,
        })
    }
    pub(super) fn capture(&self) -> CapturedSnapshotSelection<'_> {
        CapturedSnapshotSelection {
            directory: self.directory,
            selected: SnapshotRecordSelection {
                host: &self.host,
                boot: &self.boot,
                kind: &self.kind,
                capability: &self.capability,
                binding: &self.residence,
                host_call: &self.call,
                authority: &self.authority,
            },
            reference: &self.reference,
            original_grant: &self.grant,
            local_reference: &self.local,
        }
    }
}
