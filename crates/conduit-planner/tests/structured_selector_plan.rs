use conduit_core::{
    port_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId, KindId,
    OfferGeneration, PortDescriptor, PortDirection, PortTemporal, StructuredFieldType,
    StructuredInfoType, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document,
    structured_selector_definition, CheckedCordStage, KindDefinition, KindSignature,
    ProfileCatalog, StartupCatalog,
};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};

fn checked_and_definitions() -> (
    conduit_form::CheckedSyntaxDocument,
    KindDefinition,
    KindDefinition,
    KindDefinition,
) {
    let text = StructuredInfoType::leaf(KindId::from("value/text@1")).unwrap();
    let feedback = StructuredInfoType::record(
        KindId::from("product/feedback@1"),
        vec![StructuredFieldType::new("status", text.clone()).unwrap()],
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    startup
        .insert_structured_type("Feedback", feedback.clone())
        .unwrap();
    for kind in ["test/source", "test/sink"] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let source = "form pipeline {\n source: test/source\n sink: test/sink\n source > project(Feedback.status) > sink\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &startup).expect("Form checks");
    let CheckedCordStage::StructuredSelector { selector, .. } =
        &checked.forms[0].cords[0].stages[1]
    else {
        unreachable!()
    };
    let selector = selector.clone();
    (
        checked,
        primitive(
            "test/source",
            PortDirection::Output,
            feedback.profile().unwrap().value_kind().clone(),
        ),
        structured_selector_definition(&selector, PortTemporal::Value),
        primitive(
            "test/sink",
            PortDirection::Input,
            text.profile().unwrap().value_kind().clone(),
        ),
    )
}

fn primitive(kind: &str, direction: PortDirection, value_kind: KindId) -> KindDefinition {
    let port = PortDescriptor {
        port_id: port_id(match direction {
            PortDirection::Input => "input",
            PortDirection::Output => "output",
        }),
        value_kind,
        direction,
        temporal: PortTemporal::Value,
    };
    KindDefinition {
        kind_id: KindId::from(kind),
        kind_contract_revision: conduit_core::KindContractRevision::from(format!("{kind}@1")),
        inputs: (direction == PortDirection::Input)
            .then_some(port.clone())
            .into_iter()
            .collect(),
        outputs: (direction == PortDirection::Output)
            .then_some(port)
            .into_iter()
            .collect(),
        configuration: vec![],
    }
}

fn offer(definition: &KindDefinition) -> CapabilityOffer {
    let slug = definition.kind_id.as_str().replace('/', "-");
    CapabilityOffer {
        startup_parameters: definition
            .configuration
            .iter()
            .map(|field| conduit_core::FaceStartupParameter {
                name: field.key.clone(),
                value_type: "Text".into(),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(slug.as_str()),
        kind_id: definition.kind_id.clone(),
        kind_contract_revision: definition.kind_contract_revision.clone(),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("test/profile"),
            implementation_id: ImplementationId::from(format!("std/{slug}")),
            artifact_id: ArtifactId::from(format!("test/{slug}")),
        },
        inputs: definition.inputs.clone(),
        outputs: definition.outputs.clone(),
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 8,
            max_queue_bytes: 1_024,
        },
    }
}

fn host(definitions: &[KindDefinition]) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("std-host"),
        boot_id: BootId::from("std-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/host"),
        resources: vec![],
        capabilities: definitions.iter().map(offer).collect(),
        planner_capabilities: vec![],
    }
}

#[test]
fn selector_is_an_ordinary_exact_planned_leaf_and_wrong_profile_refuses() {
    let (checked, source, selector, sink) = checked_and_definitions();
    let definitions = [source, selector.clone(), sink];
    let mut catalog = ProfileCatalog::new();
    for definition in &definitions {
        catalog.insert(definition.clone()).unwrap();
    }
    let expanded = expand_canonical_form(&checked, "pipeline", &catalog).unwrap();
    let exact_host = host(&definitions);
    let placements =
        default_expanded_placements(&expanded, std::slice::from_ref(&exact_host)).unwrap();
    let plan = plan_expanded_canonical(
        &expanded,
        &[exact_host],
        &placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    assert!(plan.fragments[0]
        .placements
        .iter()
        .any(|placement| placement.kind_id == selector.kind_id));

    let mut wrong = definitions;
    wrong[1].outputs[0].value_kind = KindId::from("structured-info/wrong-profile@1");
    assert!(default_expanded_placements(&expanded, &[host(&wrong)]).is_err());
}
