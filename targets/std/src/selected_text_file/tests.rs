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
            implementation_id: offer.implementation.implementation_id.as_str().into(),
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

#[test]
fn cold_image_initializes_real_directory_and_retains_original_operation_grant() {
    use conduit_host_make::*;
    let packages = conduit_workspace_make::package_set();
    let catalog = conduit_workspace_make::catalog();
    let source = include_str!("../../profiles/raspberry-pi-3-tick-workset.host.conduit")
        .lines()
        .filter(|line| !line.contains("serial/text") && !line.contains("storage/protected-file"))
        .collect::<Vec<_>>()
        .join("\n")
        .replace("  limits =", r#"  base = {kind: "storage/text-generation", implementation: "std/file-data-save-text@1", initialization: {maximum_text_bytes: 128, preparation_bytes: 32768, retained_bytes: 16384}}
  capability = {kind: "data/save/text", pinned_implementation: "std/file-data-save-text@1"}
  need = {id: "recording-generation", class: "data/immutable-generation@1", slots: 1, bytes: 128}
  limits ="#);
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/cold-file".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let selected_base = image
        .manifest
        .base_selections
        .iter()
        .find(|b| b.kind == DURABLE_TEXT_BASE_KIND)
        .unwrap();
    let (mut definition, reference) = selected(true);
    definition.base.base_id = selected_base.id.as_str().into();
    definition.base.resources[0].pool_id = "recording-generation".into();
    definition.base.resources[0]
        .content
        .as_mut()
        .unwrap()
        .base_id = definition.base.base_id.clone();
    let config = crate::StdHostConfig {
        host_id: definition.authority.host_id.clone(),
        boot_id: definition.authority.boot_id.clone(),
        offer_generation: OfferGeneration(1),
    };
    let directory = std::env::temp_dir().join(format!("conduit-cold-file-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let (mut enlarged, _) = selected(true);
    enlarged.base.base_id = definition.base.base_id.clone();
    enlarged.base.resources[0].pool_id = definition.base.resources[0].pool_id.clone();
    enlarged.base.resources[0].content.as_mut().unwrap().base_id = definition.base.base_id.clone();
    enlarged.retained_bytes += 1;
    assert!(crate::StdHost::from_selected_timer_and_text_file_image(
        config.clone(),
        &image,
        &catalog,
        &checked,
        TextFileInitialization {
            definition: enlarged,
            directory: std::fs::File::open(&directory).unwrap(),
            reference: reference.clone(),
        }
    )
    .is_err());
    let expected_grant = definition.authority.clone();
    let host = crate::StdHost::from_selected_timer_and_text_file_image(
        config,
        &image,
        &catalog,
        &checked,
        TextFileInitialization {
            definition,
            directory: std::fs::File::open(&directory).unwrap(),
            reference,
        },
    )
    .unwrap();
    assert_eq!(host.text_file_authority_grant(), Some(&expected_grant));
    assert!(host
        .selected_text_file
        .as_ref()
        .unwrap()
        .image_memory
        .is_some());
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    drop(host);
    std::fs::remove_dir(directory).unwrap();
}
