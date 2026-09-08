use super::*;
use alloc::format;

fn mapping(id: &str, resource: &str, direction: InteropDirection) -> InteropMapping {
    InteropMapping {
        mapping_id: InteropMappingId::from(id),
        adapter_id: InteropAdapterId::from("adapter/external/one"),
        base_instance_id: BaseInstanceId::from("base/interop/one"),
        external_resource_id: ExternalResourceId::from(resource),
        semantic_kind: KindId::from("vision/image"),
        semantic_revision: KindContractRevision::from("vision/image@1"),
        direction,
        authority_grant_id: AuthorityGrantId::from(format!("grant/{id}")),
        maximum_payload_bytes: 128,
        maximum_queued_items: 2,
        external_delivery: ExternalDeliveryContract::BestEffort,
        declared_semantic_delivery: ExternalDeliveryContract::BestEffort,
        preserves_external_lifecycle: true,
        intentional_reimport_of: None,
    }
}

#[test]
fn directional_bridge_does_not_assimilate_reflections_or_siblings() {
    let mut membrane = InteropMembrane::new(InteropMembraneLimits {
        maximum_mappings: 4,
        maximum_manifestations: 2,
    })
    .unwrap();
    membrane
        .register_mapping(mapping(
            "mapping/import-a",
            "external/topic/a",
            InteropDirection::ExternalToConduit,
        ))
        .unwrap();
    membrane
        .register_mapping(mapping(
            "mapping/export-b",
            "external/topic/b",
            InteropDirection::ConduitToExternal,
        ))
        .unwrap();

    assert!(matches!(
        membrane.consider_import(&ExternalObservation {
            adapter_id: InteropAdapterId::from("adapter/external/one"),
            external_resource_id: ExternalResourceId::from("external/topic/a"),
            payload_bytes: 32,
            origin: None,
        }),
        ImportDecision::Admitted { mapping_id, .. }
            if mapping_id.as_str() == "mapping/import-a"
    ));
    let outward = membrane
        .manifest(&InteropMappingId::from("mapping/export-b"))
        .unwrap();
    assert_eq!(
        membrane.consider_import(&ExternalObservation {
            adapter_id: outward.origin.adapter_id.clone(),
            external_resource_id: outward.external_resource_id.clone(),
            payload_bytes: 32,
            origin: Some(outward.origin.clone()),
        }),
        ImportDecision::ReflectedManifestation
    );
    assert_eq!(
        membrane.consider_import(&ExternalObservation {
            adapter_id: InteropAdapterId::from("adapter/external/one"),
            external_resource_id: ExternalResourceId::from("external/topic/c"),
            payload_bytes: 32,
            origin: None,
        }),
        ImportDecision::Unconfigured
    );

    let mut reimport = mapping(
        "mapping/explicit-reimport-b",
        "external/topic/b",
        InteropDirection::ExternalToConduit,
    );
    reimport.intentional_reimport_of = Some(outward.origin.export_mapping_id.clone());
    membrane.register_mapping(reimport).unwrap();
    assert!(matches!(
        membrane.consider_import(&ExternalObservation {
            adapter_id: outward.origin.adapter_id.clone(),
            external_resource_id: outward.external_resource_id.clone(),
            payload_bytes: 32,
            origin: Some(outward.origin),
        }),
        ImportDecision::Admitted { mapping_id, authority_grant_id }
            if mapping_id.as_str() == "mapping/explicit-reimport-b"
                && authority_grant_id.as_str() == "grant/mapping/explicit-reimport-b"
    ));
}

#[test]
fn direction_bounds_and_mapping_identity_are_exact() {
    let mut membrane = InteropMembrane::new(InteropMembraneLimits {
        maximum_mappings: 1,
        maximum_manifestations: 1,
    })
    .unwrap();
    let import = mapping(
        "mapping/import",
        "external/resource",
        InteropDirection::ExternalToConduit,
    );
    assert_ne!(
        import.mapping_id.as_str(),
        import.external_resource_id.as_str()
    );
    assert_ne!(import.mapping_id.as_str(), import.semantic_kind.as_str());
    membrane.register_mapping(import).unwrap();
    assert_eq!(
        membrane.manifest(&InteropMappingId::from("mapping/import")),
        Err(InteropRefusal::WrongDirection)
    );
    assert_eq!(
        membrane.consider_import(&ExternalObservation {
            adapter_id: InteropAdapterId::from("adapter/external/one"),
            external_resource_id: ExternalResourceId::from("external/resource"),
            payload_bytes: 129,
            origin: None,
        }),
        ImportDecision::PayloadOverflow
    );
}
