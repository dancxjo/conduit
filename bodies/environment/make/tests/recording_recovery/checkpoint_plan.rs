//! Snapshot and State offers are independent admission fixtures.
use super::*;
pub(super) fn planned_checkpoint(
    source: &conduit_environment_make::RecordingCatalogSeedSource,
    reference: &c::BoundedResourceRef,
    boot: &str,
) -> (host::StdHost, c::Plan, Vec<u8>) {
    let mut advertisement = catalog_host::configured(source, boot);
    let present = source
        .plot
        .expanded
        .gears
        .iter()
        .find(|gear| {
            gear.kind_id.as_str() != c::STATE_VALUE_KIND
                && gear.kind_id.as_str() != conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND
        })
        .unwrap();
    let c::ConfigurationValue::Text(program) = &present.configuration[0].value else {
        panic!("program")
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap();
    advertisement.capabilities.push(
        conduit_std_offers::pure_expression_std_offer(&program, c::PortTemporal::Value).unwrap(),
    );
    let resource = c::ResourceOffer {
        pool_id: "fixture/catalog-files".into(),
        class_id: reference.access_class.clone(),
        capacity_units: 1,
        compute: None,
        content: Some(c::ResourceContentOffer {
            contract: c::ResourceContentRequirement {
                identity: reference.identity,
                version: reference.lifetime.version,
                content_profile: reference.content_profile.clone(),
                maximum_bytes: 4096,
                maximum_items: 1,
                retention: c::ResourceRetention::ExternalDurable,
                sharing: c::ResourceSharing::SingleWriterPublished,
                access: c::ResourceAccessMode::WriteCandidatePublish,
                generation_slots: 1,
                reader_leases: 1,
                publication_slots: 1,
                sensitive: false,
            },
            owner_host: advertisement.host_id.clone(),
            owner_boot: advertisement.boot_id.clone(),
            base_id: "fixture/catalog-base".into(),
            residence_profile: host::linux_snapshot::PROFILE.into(),
        }),
    };
    let offer = host::linux_snapshot::offer_for_resource(&resource).unwrap();
    let grant = c::AuthorityGrant {
        grant_id: "fixture/catalog-write".into(),
        contract_id: conduit_linux_snapshot_store::AUTHORITY_CONTRACT.into(),
        host_call_contract_id: conduit_linux_snapshot_store::PUBLISH_OPERATION.into(),
        subject_kind: offer.kind_id.clone(),
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        capability_id: offer.capability_id.clone(),
    };
    advertisement.bases.push(c::BaseProviderAdvertisement {
        base_id: "fixture/catalog-base".into(),
        provider_instance_id: "fixture/catalog-provider".into(),
        provider_generation: 1,
        implementation_id: "conduit.base/local@1".into(),
        mechanism_family: "fixture/files".into(),
        enforcement_class: c::BaseEnforcementClass::Cooperative,
        lifecycle: c::BaseLifecycle::Ready,
        capability_ids: vec![offer.capability_id.clone()],
        resource_pool_ids: vec![resource.pool_id.clone()],
    });
    advertisement.capabilities.push(offer);
    advertisement.resources.push(resource);
    let payload = c::StructuredInfoValue::leaf(
        c::StructuredInfoType::leaf(c::kind_id(c::RESOURCE_REFERENCE_INFO_ID)).unwrap(),
        reference.encode().unwrap(),
    )
    .unwrap();
    let expected = c::StructuredInfoValue::variant(source.value_type.clone(), "some", payload)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let (host, plan) = catalog_host::plan_source(source, advertisement, &[grant], 4096);
    (host, plan, expected)
}
