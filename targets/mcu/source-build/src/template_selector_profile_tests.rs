use super::*;
fn fixture() -> (
    e::GeneratedStaticNode,
    conduit_plot::CheckedGear,
    [c::StructuredInfoType; 2],
) {
    let output = c::StructuredInfoType::leaf(c::KindId::from("value/u8")).unwrap();
    let input = c::StructuredInfoType::record(
        c::KindId::from("test/envelope"),
        vec![
            c::StructuredFieldType::new("reading", output.clone()).unwrap(),
            c::StructuredFieldType::new(
                "guard",
                c::StructuredInfoType::leaf(c::KindId::from("value/bool")).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let selector = c::StructuredSelector::field(input.clone(), "reading").unwrap();
    let kind = conduit_semantic_catalog::structured_selector_contract(
        &selector,
        c::PortTemporal::Flow { closes: true },
    );
    let configuration = vec![c::ConfigurationEntry {
        key: "selector".into(),
        value: c::ConfigurationValue::Text(
            selector
                .canonical_bytes()
                .unwrap()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        ),
    }];
    let gear = conduit_plot::CheckedGear {
        gear_id: "selector".into(),
        kind_id: kind.kind_id.clone(),
        kind_contract_revision: kind.kind_contract_revision.clone(),
        startup_parameters: kind.startup_parameters.clone(),
        shorthand: kind.shorthand.clone(),
        inputs: kind.inputs.clone(),
        outputs: kind.outputs.clone(),
        semantic_contract: kind.semantic_contract(),
        terminal_transductions: vec![],
        resource_ports: vec![],
        configuration,
        pool_references: vec![],
    };
    let node = e::GeneratedStaticNode {
        node: 0,
        placement_id: "selector-placement".into(),
        kind_id: gear.kind_id.as_str().into(),
        implementation_id: software::implementation_identity(&gear.kind_id, &gear.configuration)
            .unwrap()
            .as_str()
            .into(),
        artifact_id: software::ARTIFACT.into(),
        input_cords: [None; conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE],
        terminal_transductions: [None;
            conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE],
        maximum_step_fuel: 1,
    };
    (node, gear, [input, output])
}
#[test]
fn exact_standard_selector_pins_both_whole_schemas_and_never_a_foreign_realization() {
    let (node, gear, types) = fixture();
    assert_eq!(
        selected_types(&node, &gear.configuration, &gear).unwrap(),
        Some(types)
    );
    let mut foreign = node.clone();
    foreign.artifact_id = "foreign/runtime".into();
    assert_eq!(
        selected_types(&foreign, &gear.configuration, &gear).unwrap(),
        None
    );
    foreign = node.clone();
    foreign.implementation_id = "foreign/implementation".into();
    assert_eq!(
        selected_types(&foreign, &gear.configuration, &gear).unwrap(),
        None
    );
    foreign = node;
    foreign.kind_id = "foreign/kind".into();
    assert!(selected_types(&foreign, &gear.configuration, &gear).is_err());
}
#[test]
fn configuration_contract_and_selector_material_cannot_borrow_the_original_pins() {
    let (node, mut gear, _) = fixture();
    assert!(selected_types(&node, &[], &gear).is_err());
    gear.outputs.clear();
    assert!(selected_types(&node, &gear.configuration, &gear).is_err());
    let (node, mut gear, _) = fixture();
    gear.configuration.push(gear.configuration[0].clone());
    assert!(selected_types(&node, &gear.configuration, &gear).is_err());
    let (node, mut gear, _) = fixture();
    gear.configuration[0].value = c::ConfigurationValue::Text("00".into());
    assert!(selected_types(&node, &gear.configuration, &gear).is_err());
}
