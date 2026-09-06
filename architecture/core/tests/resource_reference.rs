use conduit_core::{
    decode_structured_transport, encode_structured_transport, kind_id, AdmittedResourceAccess,
    AuthorityContractId, AuthorityGrantId, BoundedResourceRef, ResourceClassId,
    ResourceDereferenceRequirement, ResourceExtent, ResourceHandleId, ResourceLifetime,
    ResourceReferenceAccessRefusal, ResourceReferenceAvailability, ResourceReferenceBinding,
    ResourceReferenceRefusal, ResourceSemanticIdentity, ResourceVersionIdentity,
    StructuredFieldType, StructuredFieldValue, StructuredInfoType, StructuredInfoValue,
    StructuredInfoValueShape, TemporalInstant, TemporalRelation, TemporalScale,
    MAXIMUM_REFERENCED_BYTES, MAXIMUM_STRUCTURED_TRANSPORT_BYTES, RESOURCE_REFERENCE_INFO_ID,
};

fn digest(marker: u8) -> [u8; 32] {
    [marker; 32]
}

fn reference(profile: &str, bytes: u64, items: Option<u64>) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(digest(1)),
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("content/read@1"),
        extent: ResourceExtent { bytes, items },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest(digest(2)),
            expires_at: Some(TemporalInstant {
                ticks: 2_000,
                scale: TemporalScale::Milliseconds,
                clock_basis: "unix/utc@1".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            }),
        },
    }
}

fn requirement(profile: &str, bytes: u64, items: Option<u64>) -> ResourceDereferenceRequirement {
    ResourceDereferenceRequirement {
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("content/read@1"),
        authority_contract: AuthorityContractId::from("content/read-authority@1"),
        maximum_bytes: bytes,
        maximum_items: items,
    }
}

fn binding(reference: &BoundedResourceRef, handle: &str) -> ResourceReferenceBinding {
    ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from(handle),
        authority_contract: AuthorityContractId::from("content/read-authority@1"),
        authority_grant: AuthorityGrantId::from("grant/content-read/1"),
        maximum_bytes: reference.extent.bytes,
        maximum_items: reference.extent.items,
        availability: ResourceReferenceAvailability::Available,
    }
}

#[test]
fn filesystem_and_audio_profiles_share_one_exact_canonical_reference_contract() {
    let file = reference("content/utf8-document@1", 65_536, Some(4_096));
    let audio = reference("content/pcm-s16le-mono-48000@1", 192_000, Some(96_000));

    for (value, handle) in [(file, "filesystem/document-7"), (audio, "audio/buffer-19")] {
        value.validate().unwrap();
        let encoded = value.encode().unwrap();
        assert_eq!(BoundedResourceRef::decode(&encoded), Ok(value.clone()));
        let access = requirement(
            value.content_profile.as_str(),
            value.extent.bytes,
            value.extent.items,
        )
        .admit(&value, &binding(&value, handle))
        .unwrap();
        assert_eq!(access.handle.as_str(), handle);
        assert_ne!(value.semantic_digest().unwrap(), [0; 32]);
        let debug = format!("{value:?}");
        assert!(!debug.contains("file://"));
        assert!(!debug.contains("https://"));
        assert!(!debug.contains("/home/"));
    }
    assert_eq!(RESOURCE_REFERENCE_INFO_ID, "value/resource-ref@1");
}

#[test]
fn encoded_view_validates_without_owning_reference_identities() {
    let resource = reference("content/image-rgba8@1", 16_384, Some(4_096));
    let encoded = resource.encode().unwrap();
    let view = BoundedResourceRef::validate_encoded(&encoded).unwrap();
    assert_eq!(view.content_profile, resource.content_profile.as_str());
    assert_eq!(view.access_class, resource.access_class.as_str());
    assert_eq!(view.extent, resource.extent);

    let mut malformed = encoded.clone();
    malformed.push(0);
    assert_eq!(
        BoundedResourceRef::validate_encoded(&malformed),
        Err(ResourceReferenceRefusal::MalformedEncoding)
    );
    let mut zero_identity = encoded;
    zero_identity[1..33].fill(0);
    assert_eq!(
        BoundedResourceRef::validate_encoded(&zero_identity),
        Err(ResourceReferenceRefusal::ZeroSemanticIdentity)
    );
}

#[test]
fn structured_form_value_carries_reference_without_inlining_large_content() {
    let reference_type = StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap();
    let record_type = StructuredInfoType::record(
        kind_id("media/input@1"),
        vec![StructuredFieldType::new("content", reference_type.clone()).unwrap()],
    )
    .unwrap();
    let resource = reference("content/pcm-s16le-mono-48000@1", 192_000, Some(96_000));
    let record = StructuredInfoValue::record(
        record_type.clone(),
        vec![StructuredFieldValue::new(
            "content",
            StructuredInfoValue::leaf(reference_type, resource.encode().unwrap()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();

    let maximum = MAXIMUM_STRUCTURED_TRANSPORT_BYTES as u32;
    let encoded = encode_structured_transport(&record, maximum).unwrap();
    assert!(encoded.len() < 512);
    assert!(encoded.len() < resource.extent.bytes as usize);
    let decoded = decode_structured_transport(&record_type, &encoded, maximum).unwrap();
    let StructuredInfoValueShape::Record(fields) = decoded.shape() else {
        panic!("resource specimen must remain a record");
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields[0].value().shape() else {
        panic!("resource reference must remain a semantic leaf");
    };
    assert_eq!(BoundedResourceRef::decode(bytes), Ok(resource));
}

#[test]
fn different_host_handles_bind_one_semantic_reference_without_changing_identity() {
    let resource = reference("content/utf8-document@1", 65_536, Some(4_096));
    let requirement = requirement("content/utf8-document@1", 65_536, Some(4_096));
    let local = requirement
        .admit(&resource, &binding(&resource, "host-a/fd-7"))
        .unwrap();
    let remote = requirement
        .admit(&resource, &binding(&resource, "host-b/object-19"))
        .unwrap();

    assert_eq!(local.maximum_bytes, remote.maximum_bytes);
    assert_eq!(local.maximum_items, remote.maximum_items);
    assert_ne!(local.handle, remote.handle);
    assert_eq!(resource.identity.digest(), digest(1));
    assert_eq!(resource.lifetime.version.digest(), digest(2));
    let AdmittedResourceAccess {
        authority_grant, ..
    } = local;
    assert_eq!(authority_grant.as_str(), "grant/content-read/1");
}

#[test]
fn access_refuses_lost_stale_mismatched_and_unadmitted_references() {
    let resource = reference("content/utf8-document@1", 65_536, Some(4_096));
    let requirement = requirement("content/utf8-document@1", 65_536, Some(4_096));

    let mut candidate = binding(&resource, "host-a/fd-7");
    candidate.availability = ResourceReferenceAvailability::Lost;
    assert_eq!(
        requirement.admit(&resource, &candidate),
        Err(ResourceReferenceAccessRefusal::ResourceLost)
    );
    candidate.availability = ResourceReferenceAvailability::Stale;
    assert_eq!(
        requirement.admit(&resource, &candidate),
        Err(ResourceReferenceAccessRefusal::ResourceStale)
    );
    candidate.availability = ResourceReferenceAvailability::Available;
    candidate.version = ResourceVersionIdentity::from_digest(digest(9));
    assert_eq!(
        requirement.admit(&resource, &candidate),
        Err(ResourceReferenceAccessRefusal::StaleVersion)
    );
    candidate.version = resource.lifetime.version;
    candidate.content_profile = kind_id("content/image-rgba8@1");
    assert_eq!(
        requirement.admit(&resource, &candidate),
        Err(ResourceReferenceAccessRefusal::ContentProfileMismatch)
    );
    candidate.content_profile = resource.content_profile.clone();
    candidate.authority_grant = AuthorityGrantId::from("");
    assert_eq!(
        requirement.admit(&resource, &candidate),
        Err(ResourceReferenceAccessRefusal::EmptyAuthorityGrant)
    );
}

#[test]
fn bounds_and_expiry_are_exact_and_fail_closed() {
    let resource = reference("content/image-rgba8@1", 16_384, Some(4_096));
    let before = TemporalInstant {
        ticks: 1_000,
        scale: TemporalScale::Milliseconds,
        clock_basis: "unix/utc@1".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    let after = TemporalInstant {
        ticks: 3_000,
        ..before.clone()
    };
    assert!(matches!(
        resource.expiry_relation(&before),
        Ok(Some(TemporalRelation::Future { .. }))
    ));
    assert!(matches!(
        resource.expiry_relation(&after),
        Ok(Some(TemporalRelation::Past { .. }))
    ));

    assert_eq!(
        requirement("content/image-rgba8@1", 16_383, Some(4_096))
            .admit(&resource, &binding(&resource, "image/slot-1")),
        Err(ResourceReferenceAccessRefusal::ByteBoundExceeded)
    );
    assert_eq!(
        requirement("content/image-rgba8@1", 16_384, Some(4_095))
            .admit(&resource, &binding(&resource, "image/slot-1")),
        Err(ResourceReferenceAccessRefusal::ItemBoundExceeded)
    );
    assert_eq!(
        requirement("content/image-rgba8@1", 16_384, None)
            .admit(&resource, &binding(&resource, "image/slot-1")),
        Err(ResourceReferenceAccessRefusal::ItemBoundExceeded)
    );

    let mut oversized = resource.clone();
    oversized.extent.bytes = MAXIMUM_REFERENCED_BYTES + 1;
    assert_eq!(
        oversized.validate(),
        Err(ResourceReferenceRefusal::ByteBoundExceeded)
    );
}

#[test]
fn malformed_or_locator_shaped_bytes_never_become_a_reference_identity() {
    let resource = reference("content/utf8-document@1", 12, None);
    let mut encoded = resource.encode().unwrap();
    encoded[0] = 9;
    assert_eq!(
        BoundedResourceRef::decode(&encoded),
        Err(ResourceReferenceRefusal::UnsupportedEncodingVersion)
    );
    assert_eq!(
        BoundedResourceRef::decode(b"file:///tmp/secret"),
        Err(ResourceReferenceRefusal::UnsupportedEncodingVersion)
    );
    let mut zero_identity = resource;
    zero_identity.identity = ResourceSemanticIdentity::from_digest([0; 32]);
    assert_eq!(
        zero_identity.validate(),
        Err(ResourceReferenceRefusal::ZeroSemanticIdentity)
    );
}
