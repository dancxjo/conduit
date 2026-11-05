use conduit_core::*;

fn contract() -> ResourceContentRequirement {
    ResourceContentRequirement {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        version: ResourceVersionIdentity::from_digest([2; 32]),
        content_profile: kind_id("image/rgba@1"),
        maximum_bytes: 262144,
        maximum_items: 65536,
        retention: ResourceRetention::Play,
        sharing: ResourceSharing::SingleWriterPublished,
        access: ResourceAccessMode::WriteCandidatePublish,
        generation_slots: 1,
        reader_leases: 3,
        publication_slots: 1,
        sensitive: false,
    }
}
fn offer() -> ResourceContentOffer {
    ResourceContentOffer {
        contract: contract(),
        owner_host: HostId::from("host/frame"),
        owner_boot: BootId::from("boot/1"),
        base_id: HostBaseId::from("base/arena"),
        residence_profile: kind_id("host-arena@1"),
    }
}
#[test]
fn content_contract_admission_preserves_exact_meaning_and_refuses_foreign_residence() {
    let mut required = resource_requirement("frame", 1);
    required.content = Some(contract());
    let mut offered = resource_offer("frames", "frame", 1);
    offered.content = Some(offer());
    let selected = bind_resource_content(
        &required,
        &offered,
        &HostId::from("host/frame"),
        &BootId::from("boot/1"),
    )
    .unwrap();
    let binding = ResourceBinding {
        pool_id: offered.pool_id.clone(),
        class_id: offered.class_id.clone(),
        units: 1,
        protected: None,
        compute: None,
        content: selected,
    };
    assert!(resource_binding_satisfies(&binding, &required, &offered));
    assert_eq!(
        bind_resource_content(
            &required,
            &offered,
            &HostId::from("host/remote"),
            &BootId::from("boot/1")
        ),
        Err(ResourceContentRefusal::ForeignResidence)
    );
    let mut forged = binding.clone();
    forged.content.as_mut().unwrap().contract.version =
        ResourceVersionIdentity::from_digest([3; 32]);
    assert!(!resource_binding_satisfies(&forged, &required, &offered));
    for mutate in [
        |c: &mut ResourceContentRequirement| c.maximum_bytes = 0,
        |c: &mut ResourceContentRequirement| c.reader_leases = 0,
        |c: &mut ResourceContentRequirement| c.generation_slots = 0,
        |c: &mut ResourceContentRequirement| c.publication_slots = 2,
        |c: &mut ResourceContentRequirement| {
            c.version = ResourceVersionIdentity::from_digest([0; 32])
        },
    ] {
        let mut c = contract();
        mutate(&mut c);
        assert_eq!(c.validate(), Err(ResourceContentRefusal::InvalidContract));
    }
    let mut c = contract();
    c.sharing = ResourceSharing::SynchronizedMutable;
    assert_eq!(
        c.validate(),
        Err(ResourceContentRefusal::UnsupportedCoherence)
    );
}
#[test]
fn reference_encoding_remains_portable_and_lifetime_and_coherence_are_separate_contracts() {
    let c = contract();
    let reference = BoundedResourceRef {
        identity: c.identity,
        content_profile: c.content_profile.clone(),
        access_class: ResourceClassId::from("frame"),
        extent: ResourceExtent {
            bytes: 262144,
            items: Some(65536),
        },
        lifetime: ResourceLifetime {
            version: c.version,
            expires_at: None,
        },
    };
    assert_eq!(c.accepts_reference(&reference), Ok(()));
    let encoded = reference.encode().unwrap();
    for retention in [
        ResourceRetention::Invocation,
        ResourceRetention::Play,
        ResourceRetention::Boot,
        ResourceRetention::BodyDurable,
        ResourceRetention::ExternalDurable,
    ] {
        let mut c = c.clone();
        c.retention = retention;
        assert_eq!(c.accepts_reference(&reference), Ok(()));
        assert_eq!(reference.encode().unwrap(), encoded);
    }
    let mut read = c.clone();
    read.sharing = ResourceSharing::ImmutableReadMany;
    read.access = ResourceAccessMode::ReadPublished;
    read.publication_slots = 0;
    assert_eq!(read.validate(), Ok(()));
    let mut wrong = reference.clone();
    wrong.lifetime.version = ResourceVersionIdentity::from_digest([9; 32]);
    assert_eq!(
        c.accepts_reference(&wrong),
        Err(ResourceContentRefusal::ReferenceMismatch)
    );
}

#[test]
fn byte_only_references_keep_exact_identity_version_and_byte_bounds_without_invented_items() {
    let c = contract();
    let mut reference = BoundedResourceRef {
        identity: c.identity,
        content_profile: c.content_profile.clone(),
        access_class: ResourceClassId::from("byte-content"),
        extent: ResourceExtent {
            bytes: u64::from(c.maximum_bytes),
            items: None,
        },
        lifetime: ResourceLifetime {
            version: c.version,
            expires_at: None,
        },
    };
    assert_eq!(c.accepts_reference(&reference), Ok(()));
    reference.extent.bytes += 1;
    assert_eq!(
        c.accepts_reference(&reference),
        Err(ResourceContentRefusal::ReferenceMismatch)
    );
    reference.extent.bytes -= 1;
    reference.extent.items = Some(u64::from(c.maximum_items) + 1);
    assert_eq!(
        c.accepts_reference(&reference),
        Err(ResourceContentRefusal::ReferenceMismatch)
    );
    reference.extent.items = None;
    reference.lifetime.version = ResourceVersionIdentity::from_digest([9; 32]);
    assert_eq!(
        c.accepts_reference(&reference),
        Err(ResourceContentRefusal::ReferenceMismatch)
    );
}

#[test]
fn reader_access_preserves_every_other_content_obligation() {
    let mut required = resource_requirement("frame", 1);
    let mut reader = contract();
    reader.access = ResourceAccessMode::ReadPublished;
    reader.publication_slots = 0;
    required.content = Some(reader);
    let mut offered = resource_offer("frames", "frame", 1);
    offered.content = Some(offer());
    let host = HostId::from("host/frame");
    let boot = BootId::from("boot/1");
    let bound = bind_resource_content(&required, &offered, &host, &boot)
        .unwrap()
        .unwrap();
    assert_eq!(bound.contract, *required.content.as_ref().unwrap());
    assert_eq!(
        offered.content.as_ref().unwrap().contract.access,
        ResourceAccessMode::WriteCandidatePublish
    );
    for change in [
        |c: &mut ResourceContentRequirement| {
            c.identity = ResourceSemanticIdentity::from_digest([7; 32])
        },
        |c: &mut ResourceContentRequirement| {
            c.version = ResourceVersionIdentity::from_digest([7; 32])
        },
        |c: &mut ResourceContentRequirement| c.content_profile = kind_id("other/frame@1"),
        |c: &mut ResourceContentRequirement| c.maximum_bytes -= 1,
        |c: &mut ResourceContentRequirement| c.maximum_items -= 1,
        |c: &mut ResourceContentRequirement| c.retention = ResourceRetention::Boot,
        |c: &mut ResourceContentRequirement| c.sharing = ResourceSharing::ImmutableReadMany,
        |c: &mut ResourceContentRequirement| c.generation_slots += 1,
        |c: &mut ResourceContentRequirement| c.reader_leases += 1,
        |c: &mut ResourceContentRequirement| c.sensitive = true,
    ] {
        let mut different = required.clone();
        change(different.content.as_mut().unwrap());
        assert_eq!(
            bind_resource_content(&different, &offered, &host, &boot),
            Err(ResourceContentRefusal::ContractMismatch)
        );
    }
    let mut writer = required.clone();
    writer.content = Some(contract());
    let offered_contract = &mut offered.content.as_mut().unwrap().contract;
    offered_contract.access = ResourceAccessMode::ReadPublished;
    offered_contract.publication_slots = 0;
    assert_eq!(
        bind_resource_content(&writer, &offered, &host, &boot),
        Err(ResourceContentRefusal::ContractMismatch)
    );
}
