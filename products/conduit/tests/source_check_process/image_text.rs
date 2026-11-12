//! Independent local fixture directory and grant; no runtime authority inferred
//! from offers. Production uses the stopped local File policy instead.
use super::*;
use conduit_std_offers::*;
pub(super) struct Directory(std::path::PathBuf);
impl Directory {
    pub(super) fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(super) fn initialization(
    config: &StdHostConfig,
    image: &HostImage,
) -> (Directory, TextFileInitialization) {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "conduit-source-and-file-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    let directory = Directory(path);
    let base = image
        .manifest
        .base_selections
        .iter()
        .find(|base| base.kind == DURABLE_TEXT_BASE_KIND)
        .unwrap();
    let base_id: HostBaseId = base.id.as_str().into();
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([77; 32]),
        content_profile: kind_id("value/text"),
        access_class: DURABLE_TEXT_RESOURCE.into(),
        extent: ResourceExtent {
            bytes: 11,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([78; 32]),
            expires_at: None,
        },
    };
    let content = ResourceContentRequirement {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        maximum_bytes: 256,
        maximum_items: 1,
        retention: ResourceRetention::ExternalDurable,
        sharing: ResourceSharing::SingleWriterPublished,
        access: ResourceAccessMode::WriteCandidatePublish,
        generation_slots: 1,
        reader_leases: 1,
        publication_slots: 1,
        sensitive: false,
    };
    let offer = durable_text_offer(content.clone()).unwrap();
    let authority = AuthorityGrant {
        grant_id: "fixture/independent-text-file-write".into(),
        contract_id: DURABLE_TEXT_AUTHORITY.into(),
        host_call_contract_id: DURABLE_TEXT_PUBLISH_CALL.into(),
        subject_kind: DURABLE_TEXT_SAVE_KIND.into(),
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
        capability_id: DURABLE_TEXT_SAVE_IMPLEMENTATION.into(),
    };
    let initialization = TextFileInitialization {
        definition: NativeTextFileDefinition {
            base: BaseProviderEntry {
                base_id: base_id.clone(),
                provider_instance_id: "fixture/independent-text-file-provider".into(),
                provider_generation: 1,
                implementation_id: DURABLE_TEXT_SAVE_IMPLEMENTATION.into(),
                mechanism_family: DURABLE_TEXT_BASE_KIND.into(),
                enforcement_class: BaseEnforcementClass::Cooperative,
                lifecycle: BaseLifecycle::Ready,
                capabilities: vec![offer],
                resources: vec![ResourceOffer {
                    pool_id: "recording/generation".into(),
                    class_id: DURABLE_TEXT_RESOURCE.into(),
                    capacity_units: 1,
                    compute: None,
                    content: Some(ResourceContentOffer {
                        contract: content,
                        owner_host: config.host_id.clone(),
                        owner_boot: config.boot_id.clone(),
                        base_id,
                        residence_profile: DURABLE_TEXT_RESIDENCE.into(),
                    }),
                }],
            },
            authority,
            maximum_text_bytes: 256,
            preparation_bytes: 32768,
            retained_bytes: 16384,
        },
        directory: std::fs::File::open(directory.path()).unwrap(),
        reference,
    };
    (directory, initialization)
}
