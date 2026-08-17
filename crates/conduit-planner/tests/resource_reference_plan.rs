use conduit_core::{
    authority_grant, kind_id, port_id, ArtifactId, AuthorityContractId, AuthorityRequirement,
    BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase, ExecutionProfileId,
    HostAdvertisement, HostId, HostOperationContractId, HostOperationRequirement, HostProfileId,
    ImplementationId, KindContractRevision, OfferGeneration, PortDescriptor, PortDirection,
    PortTemporal, PROTOCOL_VERSION, RESOURCE_REFERENCE_INFO_ID,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};
use std::collections::BTreeMap;

const SOURCE_KIND: &str = "content/reference-source";
const SINK_KIND: &str = "content/reference-consumer";
const MAXIMUM_REFERENCE_BYTES: u32 = 512;
const READ_OPERATION: &str = "conduit.host/content-read@1";
const READ_AUTHORITY: &str = "conduit.authority/content-read@1";

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(RESOURCE_REFERENCE_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn definition(kind: &str, direction: PortDirection) -> KindDefinition {
    let descriptor = port(
        match direction {
            PortDirection::Input => "reference",
            PortDirection::Output => "reference",
        },
        direction,
    );
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
        inputs: (direction == PortDirection::Input)
            .then_some(descriptor.clone())
            .into_iter()
            .collect(),
        outputs: (direction == PortDirection::Output)
            .then_some(descriptor)
            .into_iter()
            .collect(),
        configuration: vec![],
    }
}

fn offer(definition: &KindDefinition) -> CapabilityOffer {
    let slug = definition.kind_id.as_str().replace('/', "-");
    let dereferences = definition.kind_id.as_str() == SINK_KIND;
    let host_operations = dereferences
        .then(|| HostOperationRequirement {
            contract_id: HostOperationContractId::from(READ_OPERATION),
            target_kind: Some(kind_id(SINK_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_REFERENCE_BYTES,
            maximum_output_bytes: 1,
        })
        .into_iter()
        .collect();
    let authority_requirements = dereferences
        .then(|| AuthorityRequirement {
            contract_id: AuthorityContractId::from(READ_AUTHORITY),
            host_operation_contract_id: HostOperationContractId::from(READ_OPERATION),
            subject_kind: kind_id(SINK_KIND),
        })
        .into_iter()
        .collect();
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(format!("capability/{slug}")),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("test/resource-reference@1"),
            implementation_id: ImplementationId::from(format!("test/{slug}@1")),
            artifact_id: ArtifactId::from(format!("test/{slug}-artifact@1")),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations,
        resource_requirements: vec![],
        authority_requirements,
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MAXIMUM_REFERENCE_BYTES,
        },
    }
}

#[test]
fn exact_resource_reference_kind_survives_checked_form_and_plan_without_locator_or_content() {
    let definitions = [
        definition(SOURCE_KIND, PortDirection::Output),
        definition(SINK_KIND, PortDirection::Input),
    ];
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    for definition in &definitions {
        startup
            .insert(KindSignature {
                kind: definition.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profile.insert(definition.clone()).unwrap();
    }
    let source = "form content_pipeline {\n source: content/reference-source\n sink: content/reference-consumer\n source > sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "content_pipeline", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/content"),
        boot_id: BootId::from("boot/content/1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("host/content@1"),
        resources: vec![],
        capabilities: definitions.iter().map(offer).collect(),
        planner_capabilities: vec![],
    };
    let consumer = host
        .capabilities
        .iter()
        .find(|capability| capability.kind_id.as_str() == SINK_KIND)
        .unwrap();
    let read_grant = authority_grant(
        "grant/content-read/1",
        &consumer.authority_requirements[0],
        host.host_id.clone(),
        host.boot_id.clone(),
        consumer.capability_id.clone(),
    );
    let placements = default_expanded_placements(&expanded, core::slice::from_ref(&host)).unwrap();
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        core::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: MAXIMUM_REFERENCE_BYTES,
            authority_grants: &[read_grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();

    assert_eq!(plan.fragments[0].connections.len(), 1);
    let connection = &plan.fragments[0].connections[0];
    assert_eq!(connection.value_kind.as_str(), RESOURCE_REFERENCE_INFO_ID);
    assert_eq!(connection.item_capacity, 4);
    assert_eq!(connection.byte_capacity, MAXIMUM_REFERENCE_BYTES);
    let consumer = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == SINK_KIND)
        .unwrap();
    assert_eq!(consumer.host_operations.len(), 1);
    assert_eq!(
        consumer.host_operations[0].contract_id.as_str(),
        READ_OPERATION
    );
    assert_eq!(consumer.host_operations[0].maximum_in_flight, 1);
    assert_eq!(consumer.authority.len(), 1);
    assert_eq!(consumer.authority[0].contract_id.as_str(), READ_AUTHORITY);
    let debug = format!("{plan:?}");
    for forbidden in ["file://", "https://", "/home/", "host-a/fd-7"] {
        assert!(!debug.contains(forbidden));
    }
}
