use conduit_core::*;
use conduit_std_host::hosted_resource::HostedResourceGeneration;
pub const FRAME_BYTES: usize = 256 * 256 * 4;
pub fn prepared(sharing: ResourceSharing, generation: u8) -> HostedResourceGeneration {
    let write = sharing == ResourceSharing::SingleWriterPublished;
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("image/rgba@1"),
        access_class: ResourceClassId::from("resource/frame"),
        extent: ResourceExtent {
            bytes: FRAME_BYTES as u64,
            items: Some(65536),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([generation; 32]),
            expires_at: None,
        },
    };
    let contract = ResourceContentRequirement {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        maximum_bytes: FRAME_BYTES as u32,
        maximum_items: 65536,
        retention: ResourceRetention::Play,
        sharing,
        access: if write {
            ResourceAccessMode::WriteCandidatePublish
        } else {
            ResourceAccessMode::ReadPublished
        },
        generation_slots: 1,
        reader_leases: 3,
        publication_slots: u16::from(write),
        sensitive: false,
    };
    let binding = ResourceBinding {
        pool_id: ResourcePoolId::from("pool/frame"),
        class_id: reference.access_class.clone(),
        units: 1,
        compute: None,
        protected: None,
        content: Some(ResourceContentOffer {
            contract,
            owner_host: HostId::from("host/local"),
            owner_boot: BootId::from("boot/local"),
            base_id: HostBaseId::from("base/arena"),
            residence_profile: kind_id("host/arena@1"),
        }),
    };
    let access = ResourceDereferenceRequirement {
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        authority_contract: AuthorityContractId::from("authority/read-frame@1"),
        maximum_bytes: FRAME_BYTES as u64,
        maximum_items: Some(65536),
    };
    let reference_binding = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        access_class: reference.access_class.clone(),
        handle: ResourceHandleId::from("opaque/local-frame"),
        authority_contract: access.authority_contract.clone(),
        authority_grant: reader(),
        maximum_bytes: FRAME_BYTES as u64,
        maximum_items: Some(65536),
        availability: ResourceReferenceAvailability::Available,
    };
    HostedResourceGeneration::new(
        &binding,
        reference,
        access,
        reference_binding,
        write.then(writer),
    )
    .unwrap()
}
pub fn reader() -> AuthorityGrantId {
    AuthorityGrantId::from("grant/frame-read")
}
pub fn writer() -> AuthorityGrantId {
    AuthorityGrantId::from("grant/frame-publish")
}
