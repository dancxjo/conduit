//! Sealed binding fixture adapted from browser snapshot conformance.
//! These identities and authority are proof data, not actual Body admission.
use conduit_browser_runtime::resource_snapshot::*;
use conduit_core::*;
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
        host_call_contract_id: operation.into(),
        subject_kind: kind.clone(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        capability_id: capability.clone(),
    };
    (
        conduit_core::planned_gear_from_parts! {
            semantic_contract: Default::default(),
            placement_id: "snapshot-placement".into(),
            gear_id: "snapshot".into(),
            kind_id: kind.clone(),
            kind_contract_revision: "resource/snapshot@1".into(),
            execution_profile_id: "browser/resource-snapshot@1".into(),
            configuration: Default::default(),
            host_id: host.clone(),
            boot_id: boot.clone(),
            offer_generation: OfferGeneration(1),
            capability_id: capability,
            implementation_id: "browser/resource-snapshot@1".into(),
            artifact_id: "browser/resource-snapshot@1".into(),
            base: None,
            realization_characteristics: Vec::new(),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 4096,
            },
            inputs: Vec::new(),
            outputs: Vec::new(),
            terminal_transductions: Vec::new(),
            host_calls: vec![HostCallRequirement {
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

/// Fixture custody for an independently selected exact generation.
pub(crate) fn for_reference(
    boot: &str,
    write: bool,
    reference: &BoundedResourceRef,
) -> PlannedGear {
    let (mut placement, _) = placement(boot, write, reference.extent.bytes as usize);
    let contract = &mut placement.resources[0].content.as_mut().unwrap().contract;
    contract.identity = reference.identity;
    contract.version = reference.lifetime.version;
    placement
}
