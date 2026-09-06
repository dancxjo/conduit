use super::*;
use conduit_core::*;

// Explicit sealed-binding fixture. The record codec is not a planner.
pub(crate) fn placement(
    boot: &str,
    write: bool,
    bytes: usize,
) -> (PlannedGear, BoundedResourceRef) {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("text/json-utf8@1"),
        access_class: "resource/snapshot@1".into(),
        extent: ResourceExtent {
            bytes: bytes as u64,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let kind = kind_id(if write {
        "resource/snapshot-publish"
    } else {
        "resource/snapshot-read"
    });
    let operation = if write {
        PUBLISH_OPERATION
    } else {
        READ_OPERATION
    };
    let host: HostId = "snapshot-host".into();
    let boot: BootId = boot.into();
    let capability: CapabilityId = operation.into();
    let authority = AuthorityBinding {
        grant_id: format!("grant/{operation}").into(),
        contract_id: "authority/resource-snapshot@1".into(),
        host_operation_contract_id: operation.into(),
        subject_kind: kind.clone(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        capability_id: capability.clone(),
    };
    (
        PlannedGear {
            placement_id: "snapshot-placement".into(),
            gear_id: "snapshot".into(),
            kind_id: kind.clone(),
            kind_contract_revision: "resource/snapshot@1".into(),
            execution_profile_id: "browser/resource-snapshot@1".into(),
            configuration: Vec::new(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            offer_generation: OfferGeneration(1),
            capability_id: capability,
            implementation_id: "browser/resource-snapshot@1".into(),
            artifact_id: "browser/resource-snapshot@1".into(),
            realization_characteristics: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 4096,
            },
            inputs: Vec::new(),
            outputs: Vec::new(),
            host_operations: vec![HostOperationRequirement {
                contract_id: operation.into(),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes: if write { 4096 } else { 512 },
                maximum_output_bytes: if write { 512 } else { 4096 },
            }],
            resources: vec![ResourceBinding {
                pool_id: "snapshot-pool".into(),
                class_id: reference.access_class.clone(),
                units: 1,
                compute: None,
                protected: None,
                content: Some(ResourceContentOffer {
                    contract: ResourceContentRequirement {
                        identity: reference.identity,
                        version: reference.lifetime.version,
                        content_profile: reference.content_profile.clone(),
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
                    owner_host: host,
                    owner_boot: boot,
                    base_id: "browser/indexeddb".into(),
                    residence_profile: kind_id("browser/indexeddb@1"),
                }),
            }],
            authority: vec![authority],
            pool_references: Vec::new(),
        },
        reference,
    )
}

#[test]
fn snapshot_record_preserves_generation_across_new_boot_but_requires_new_authority() {
    let (write, reference) = placement("boot/one", true, 2);
    let mut writer = PreparedSnapshotRecord::prepare(&write, &reference).unwrap();
    let bytes = writer
        .publication(&write.authority[0], b"[]")
        .unwrap()
        .to_vec();
    let (old_read, _) = placement("boot/one", false, 2);
    let (read, _) = placement("boot/two", false, 2);
    let reader = PreparedSnapshotRecord::prepare(&read, &reference).unwrap();
    assert_eq!(writer.storage_key(), reader.storage_key());
    assert_eq!(reader.restore(&read.authority[0], &bytes).unwrap(), b"[]");
    assert_eq!(
        reader.restore(&old_read.authority[0], &bytes),
        Err(SnapshotRefusal::StaleBoot)
    );
    let mut foreign = read.authority[0].clone();
    foreign.host_id = "foreign".into();
    assert_eq!(
        reader.restore(&foreign, &bytes),
        Err(SnapshotRefusal::ForeignHost)
    );
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert_eq!(
        reader.restore(&read.authority[0], &corrupt),
        Err(SnapshotRefusal::CorruptRecord)
    );
    assert_eq!(
        reader.restore(&read.authority[0], &bytes[..bytes.len() - 1]),
        Err(SnapshotRefusal::CorruptRecord)
    );
}

#[test]
fn snapshot_binding_refuses_wrong_bounds_authority_and_lifetime() {
    let (mut write, reference) = placement("boot/one", true, 2);
    write.host_operations[0].maximum_input_bytes = 1;
    assert!(matches!(
        PreparedSnapshotRecord::prepare(&write, &reference),
        Err(SnapshotRefusal::InvalidBinding)
    ));
    write.host_operations[0].maximum_input_bytes = 4096;
    write.authority[0].capability_id = "wrong".into();
    assert!(matches!(
        PreparedSnapshotRecord::prepare(&write, &reference),
        Err(SnapshotRefusal::AuthorityDenied)
    ));
    let (mut write, _) = placement("boot/one", true, 2);
    write.resources[0]
        .content
        .as_mut()
        .unwrap()
        .contract
        .retention = ResourceRetention::Play;
    assert!(matches!(
        PreparedSnapshotRecord::prepare(&write, &reference),
        Err(SnapshotRefusal::UnsupportedLifetime)
    ));
}

#[test]
fn snapshot_generation_key_cannot_be_changed_by_claiming_a_different_extent() {
    let (one, first) = placement("boot/one", true, 2);
    let (two, second) = placement("boot/one", true, 3);
    let mut one = PreparedSnapshotRecord::prepare(&one, &first).unwrap();
    let two = PreparedSnapshotRecord::prepare(&two, &second).unwrap();
    assert_eq!(one.storage_key(), two.storage_key());
    let (placement, _) = placement("boot/one", true, 2);
    assert_eq!(
        one.publication(&placement.authority[0], b"[0]"),
        Err(SnapshotRefusal::ContentExtent)
    );
}
