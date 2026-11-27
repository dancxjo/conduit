//! Actual immutable-record test fixtures, not storage admission or live-producer proof.
//! The consuming test module supplies the public captured-selection type alias.
use super::CapturedSnapshotSelection;
use conduit_core::*;
use conduit_linux_snapshot_store::{SnapshotDirectory, SnapshotRecordSelection};
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

const READ: &str = "conduit.host/resource-snapshot-read@1";
const WRITE: &str = "conduit.host/resource-snapshot-publish@1";
const AUTHORITY: &str = "authority/resource-snapshot@1";
pub(crate) struct Directory(pub(crate) std::path::PathBuf);
impl Directory {
    pub(crate) fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "conduit-quantity-capture-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        Self(path)
    }
    pub(crate) fn open(&self) -> SnapshotDirectory {
        SnapshotDirectory::prepare(fs::File::open(&self.0).unwrap()).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if self.0.exists() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}
pub(crate) struct Facts {
    pub(crate) host: HostId,
    pub(crate) boot: BootId,
    pub(crate) kind: KindId,
    pub(crate) capability: CapabilityId,
    pub(crate) binding: ResourceBinding,
    pub(crate) call: HostCallRequirement,
    pub(crate) authority: AuthorityBinding,
    pub(crate) reference: BoundedResourceRef,
    pub(crate) original: AuthorityGrant,
    pub(crate) local: ResourceReferenceBinding,
}
impl Facts {
    pub(crate) fn new(seed: u8, write: bool, bytes: usize) -> Self {
        Self::new_on(
            seed,
            write,
            bytes,
            &"host/capture-fixture".into(),
            &"boot/current-fixture".into(),
        )
    }
    pub(crate) fn new_on(
        seed: u8,
        write: bool,
        bytes: usize,
        host: &HostId,
        boot: &BootId,
    ) -> Self {
        let operation = if write { WRITE } else { READ };
        let host = host.clone();
        let boot = boot.clone();
        let kind = kind_id(if write {
            "resource/snapshot-publish"
        } else {
            "resource/snapshot-read"
        });
        let capability: CapabilityId = operation.into();
        let profile: KindId = "text/json-utf8@1".into();
        let identity = ResourceSemanticIdentity::from_digest([seed; 32]);
        let version = ResourceVersionIdentity::from_digest([seed + 1; 32]);
        // Independent local Root fixture authorization for the directory it owns;
        // production capture never reconstructs this grant from a descriptor.
        let original = AuthorityGrant {
            grant_id: format!("grant/{seed}/{operation}").into(),
            contract_id: AUTHORITY.into(),
            host_call_contract_id: operation.into(),
            subject_kind: kind.clone(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            capability_id: capability.clone(),
        };
        let authority = AuthorityBinding {
            grant_id: original.grant_id.clone(),
            contract_id: original.contract_id.clone(),
            host_call_contract_id: original.host_call_contract_id.clone(),
            subject_kind: original.subject_kind.clone(),
            host_id: original.host_id.clone(),
            boot_id: original.boot_id.clone(),
            capability_id: original.capability_id.clone(),
        };
        let reference = BoundedResourceRef {
            identity,
            content_profile: profile.clone(),
            access_class: "resource/snapshot@1".into(),
            extent: ResourceExtent {
                bytes: bytes as u64,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version,
                expires_at: None,
            },
        };
        let local = ResourceReferenceBinding {
            identity,
            version,
            content_profile: profile.clone(),
            access_class: "resource/snapshot@1".into(),
            handle: format!("local/snapshot-{seed}").into(),
            authority_contract: AUTHORITY.into(),
            authority_grant: original.grant_id.clone(),
            maximum_bytes: 4096,
            maximum_items: Some(1),
            availability: ResourceReferenceAvailability::Available,
        };
        let binding = ResourceBinding {
            pool_id: format!("fixture/pool-{seed}").into(),
            class_id: "resource/snapshot@1".into(),
            units: 1,
            compute: None,
            protected: None,
            content: Some(ResourceContentOffer {
                contract: ResourceContentRequirement {
                    identity,
                    version,
                    content_profile: profile,
                    maximum_bytes: 4096,
                    maximum_items: 1,
                    retention: ResourceRetention::ExternalDurable,
                    sharing: ResourceSharing::SingleWriterPublished,
                    access: if write {
                        ResourceAccessMode::WriteCandidatePublish
                    } else {
                        ResourceAccessMode::ReadPublished
                    },
                    generation_slots: 1,
                    reader_leases: 1,
                    publication_slots: u16::from(write),
                    sensitive: false,
                },
                owner_host: host.clone(),
                owner_boot: boot.clone(),
                base_id: "fixture/linux-store".into(),
                residence_profile: kind_id("linux/snapshot@1"),
            }),
        };
        let call = HostCallRequirement {
            contract_id: operation.into(),
            target_kind: Some(kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: if write { 4096 } else { 512 },
            maximum_output_bytes: if write { 512 } else { 4096 },
        };
        Self {
            host,
            boot,
            kind,
            capability,
            binding,
            call,
            authority,
            reference,
            original,
            local,
        }
    }
    pub(crate) fn selected(&self) -> SnapshotRecordSelection<'_> {
        SnapshotRecordSelection {
            host: &self.host,
            boot: &self.boot,
            kind: &self.kind,
            capability: &self.capability,
            binding: &self.binding,
            host_call: &self.call,
            authority: &self.authority,
        }
    }
    pub(crate) fn capture<'a>(
        &'a self,
        directory: &'a SnapshotDirectory,
    ) -> CapturedSnapshotSelection<'a> {
        CapturedSnapshotSelection {
            directory,
            selected: self.selected(),
            reference: &self.reference,
            original_grant: &self.original,
            local_reference: &self.local,
        }
    }
}
