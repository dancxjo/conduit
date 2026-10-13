use conduit_core::{
    kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal, SCALAR_INFO_ID,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindProjection,
    KindSignature, ProfileCatalog, StartupCatalog, PURE_FILTER_REVISION,
};

fn port(name: &str, direction: PortDirection, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn catalogs(temporal: PortTemporal) -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    for kind in ["test/source", "test/sink"] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/source"),
            kind_contract_revision: KindIdentity::from("test/source@1"),
            inputs: vec![],
            outputs: vec![port("out", PortDirection::Output, temporal)],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/sink"),
            kind_contract_revision: KindIdentity::from("test/sink@1"),
            inputs: vec![port("in", PortDirection::Input, temporal)],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    (startup, profile)
}

#[test]
fn when_lowers_to_one_exact_flow_preserving_filter_gear() {
    let temporal = PortTemporal::Flow { closes: true };
    let (startup, profile) = catalogs(temporal);
    let source = "plot filter {\n source: test/source\n sink: test/sink\n source >> when(. > 1) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "filter", &profile).unwrap();
    let filter = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == PURE_FILTER_REVISION)
        .unwrap();
    assert_eq!(filter.inputs[0].temporal, temporal);
    assert_eq!(filter.outputs[0].temporal, temporal);
    assert_eq!(filter.inputs[0].value_kind, filter.outputs[0].value_kind);
    assert_eq!(expanded.connections.len(), 2);
    expanded.validate_expansion().unwrap();
}

#[test]
fn one_value_when_outputs_the_canonical_finite_optional_profile() {
    let (startup, mut profile) = catalogs(PortTemporal::Value);
    let optional = conduit_core::optional_info_type(
        conduit_core::StructuredInfoType::leaf(kind_id(SCALAR_INFO_ID)).unwrap(),
    )
    .unwrap()
    .profile()
    .unwrap()
    .value_kind()
    .clone();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/optional-sink"),
            kind_contract_revision: KindIdentity::from("test/optional-sink@1"),
            inputs: vec![PortDescriptor {
                port_id: port_id("in"),
                value_kind: optional.clone(),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let mut startup = startup;
    startup
        .insert(KindSignature {
            kind: "test/optional-sink".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let source = "plot filter {\n source: test/source\n sink: test/sink\n source >> when(. > 1) >> sink\n}\n";
    let source = source.replace("sink: test/sink", "sink: test/optional-sink");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "filter", &profile).unwrap();
    let filter = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == PURE_FILTER_REVISION)
        .unwrap();
    assert_eq!(filter.outputs[0].value_kind, optional);
    expanded.validate_expansion().unwrap();
}

#[test]
fn finite_flow_filter_preserves_both_input_and_output_contracts() {
    use conduit_core::{
        CapabilityLimits, CheckedValueContract, FrontValueContract, FrontValueLocation, Kind,
        KindSemanticLaw,
    };
    use conduit_plot::{pure_expression_semantic_laws, PortableExpressionProgram};
    let temporal = PortTemporal::Flow { closes: true };
    let (startup, mut profile) = catalogs(temporal);
    let mut output = port("out", PortDirection::Output, temporal);
    output.value_kind = kind_id("value/bytes");
    profile
        .insert_kind(Kind {
            kind_id: kind_id("test/bounded-source"),
            kind_contract_revision: KindIdentity::from("test/bounded-source@1"),
            startup_parameters: vec![],
            shorthand: None,
            inputs: vec![],
            outputs: vec![output],
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Output(port_id("out")),
                contract: CheckedValueContract::new(kind_id("value/bytes"), 2048, vec![]).unwrap(),
            }])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 2048,
            },
        })
        .unwrap();
    let source =
        "plot filter {\n source: test/bounded-source\n source >> when(bytes/length(.) > 0)\n}\n";
    let mut startup = startup;
    startup
        .insert(KindSignature {
            kind: "test/bounded-source".into(),
            startup_parameters: vec![],
        })
        .unwrap();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "filter", &profile).unwrap();
    let filter = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == PURE_FILTER_REVISION)
        .unwrap();
    let conduit_core::ConfigurationValue::Text(encoded) = &filter
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .unwrap()
        .value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    assert_eq!(program.input_maximum_bytes, Some(2048));
    let contracts = filter.semantic_contract.value_contracts();
    assert_eq!(contracts.len(), 2);
    assert!(contracts
        .iter()
        .all(|contract| contract.contract.maximum_bytes == 2048));
    assert_eq!(
        filter.semantic_contract.laws,
        program.filter_semantic_laws(temporal).unwrap()
    );
    assert_ne!(
        filter.semantic_contract.laws,
        pure_expression_semantic_laws()
    );
}
