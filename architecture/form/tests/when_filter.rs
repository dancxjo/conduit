use conduit_core::{
    kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal, SCALAR_INFO_ID,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindProjection,
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
    let source = "form filter {\n source: test/source\n sink: test/sink\n source >> when(. > 1) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "filter", &profile).unwrap();
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
    let source = "form filter {\n source: test/source\n sink: test/sink\n source >> when(. > 1) >> sink\n}\n";
    let source = source.replace("sink: test/sink", "sink: test/optional-sink");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "filter", &profile).unwrap();
    let filter = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == PURE_FILTER_REVISION)
        .unwrap();
    assert_eq!(filter.outputs[0].value_kind, optional);
    expanded.validate_expansion().unwrap();
}
