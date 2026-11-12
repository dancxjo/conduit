//! Descriptive/correspondence tests only; no IMAGE/Ready or disk-capacity claim.
use super::*;
fn selected(write: bool) -> (NativeTextFileDefinition, BoundedResourceRef) {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([17; 32]),
        content_profile: kind_id("value/text"),
        access_class: DURABLE_TEXT_RESOURCE.into(),
        extent: ResourceExtent {
            bytes: 11,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([19; 32]),
            expires_at: None,
        },
    };
    let host: HostId = "host/selected-file".into();
    let boot: BootId = "boot/selected-file".into();
    let base_id: HostBaseId = "selected-file".into();
    let content = ResourceContentRequirement {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: reference.content_profile.clone(),
        maximum_bytes: 128,
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
    };
    let offer = durable_text_offer(content.clone()).unwrap();
    let grant = AuthorityGrant {
        grant_id: "grant/selected-file".into(),
        contract_id: DURABLE_TEXT_AUTHORITY.into(),
        host_call_contract_id: offer.host_calls[0].contract_id.clone(),
        subject_kind: offer.kind_id.clone(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        capability_id: offer.capability_id.clone(),
    };
    let d = NativeTextFileDefinition {
        base: BaseProviderEntry {
            base_id: base_id.clone(),
            provider_instance_id: "base/selected-file".into(),
            provider_generation: 1,
            implementation_id: offer.implementation.implementation_id.clone(),
            mechanism_family: DURABLE_TEXT_BASE_KIND.into(),
            enforcement_class: BaseEnforcementClass::Cooperative,
            lifecycle: BaseLifecycle::Ready,
            capabilities: vec![offer],
            resources: vec![ResourceOffer {
                pool_id: "pool/file-generation".into(),
                class_id: reference.access_class.clone(),
                capacity_units: 1,
                compute: None,
                content: Some(ResourceContentOffer {
                    contract: content,
                    owner_host: host,
                    owner_boot: boot,
                    base_id,
                    residence_profile: DURABLE_TEXT_RESIDENCE.into(),
                }),
            }],
        },
        authority: grant,
        maximum_text_bytes: 128,
        preparation_bytes: 32768,
        retained_bytes: 16384,
    };
    (d, reference)
}
#[test]
fn descriptive_scope_requires_exact_original_reference_and_independent_operation_grant() {
    for write in [true, false] {
        let (d, r) = selected(write);
        let host = d.authority.host_id.clone();
        let boot = d.authority.boot_id.clone();
        validate_definition(&d, &r, &host, &boot).unwrap();
        let mut other = r.clone();
        other.lifetime.version = ResourceVersionIdentity::from_digest([23; 32]);
        assert!(validate_definition(&d, &other, &host, &boot).is_err());
        let (mut d, r) = selected(write);
        d.authority.boot_id = "boot/foreign".into();
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
        let (mut d, r) = selected(write);
        d.authority.subject_kind = "value/text".into();
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
        let (mut d, r) = selected(write);
        d.base.resources[0].content.as_mut().unwrap().base_id = "another-base".into();
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
    }
}
#[test]
fn descriptive_provider_cannot_enlarge_its_declared_bounds_or_residence() {
    for case in 0..5 {
        let (mut d, r) = selected(true);
        let host = d.authority.host_id.clone();
        let boot = d.authority.boot_id.clone();
        match case {
            0 => d.retained_bytes = d.preparation_bytes + 1,
            1 => d.maximum_text_bytes = 10,
            2 => d.preparation_bytes = 0,
            3 => d.maximum_text_bytes = 0,
            _ => d.base.resources[0].capacity_units = 2,
        }
        assert!(validate_definition(&d, &r, &host, &boot).is_err());
    }
}
