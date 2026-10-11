use conduit_core::{
    kind_id, port_id, ConfigurationValue, InfoBool, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, Quantity, Unit, BOOL_INFO_ID, TEMPERATURE_INFO_ID,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot, parse_syntax_document, KindProjection,
    KindSignature, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
    PURE_EXPRESSION_REVISION,
};

#[test]
fn immutable_quantity_local_is_captured_by_the_lowered_expression() {
    let mut startup = StartupCatalog::new();
    for kind in ["test/temperature-source", "test/bool-sink"] {
        startup
            .insert(KindSignature {
                kind: kind.into(),
                startup_parameters: vec![],
            })
            .unwrap();
    }
    let port = |name, value_kind, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let mut profile = ProfileCatalog::new();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/temperature-source"),
            kind_contract_revision: KindIdentity::from("test/temperature-source@1"),
            inputs: vec![],
            outputs: vec![port("out", TEMPERATURE_INFO_ID, PortDirection::Output)],
            configuration: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: kind_id("test/bool-sink"),
            kind_contract_revision: KindIdentity::from("test/bool-sink@1"),
            inputs: vec![port("in", BOOL_INFO_ID, PortDirection::Input)],
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();
    let source = "plot classify {\n limit = 30°C\n source: test/temperature-source\n sink: test/bool-sink\n source >> (. > limit) >> sink\n}\n";
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_plot(&checked, "classify", &profile).unwrap();
    let expression = expanded
        .gears
        .iter()
        .find(|gear| gear.kind_contract_revision.as_str() == PURE_EXPRESSION_REVISION)
        .unwrap();
    let ConfigurationValue::Text(program) = &expression.configuration[0].value else {
        panic!("expression program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(program).unwrap();
    assert_eq!(
        program
            .evaluate(&Quantity::new(31, Unit::Celsius).encode())
            .unwrap(),
        InfoBool::TRUE.encode()
    );
}
