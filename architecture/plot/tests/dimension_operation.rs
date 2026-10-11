use conduit_core::{Dimension, DIMENSION_ENCODED_LEN, DIMENSION_INFO_ID};
use conduit_plot::{
    check_syntax_document, dimension_operation as operation, expand_canonical_plot_for_authoring,
    parse_syntax_document, quantity_conversion, ProfileCatalog, StartupCatalog,
};

const DECLARATIONS: &str = "dimension travel\ndimension clock\ndimension velocity = travel / clock\ntype Motion = quantity {dimension:velocity}\nunit stepper : Motion = {reference:origin,scale:1}\nunit doublestep : Motion = {reference:stepper,scale:2}";

#[test]
fn typed_introspection_prepares_exact_dimension_without_text_or_ambient_catalogue() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    quantity_conversion::install(&mut startup, &mut profiles).unwrap();
    for (kind, ty, value) in [
        (operation::UNIT_KIND, "Unit", "doublestep"),
        (operation::QUANTITY_KIND, "Quantity", "3doublestep"),
    ] {
        let source = format!("{DECLARATIONS}\nplot forward (\n source: {ty}\n dimension: Dimension >>\n) {{\n operation: {kind}(source=source)\n operation.dimension >> dimension\n}}\nplot demo (\n dimension: Dimension >>\n) {{\n operation: forward(source={value})\n operation.dimension >> dimension\n}}.\n");
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &startup).unwrap();
        quantity_conversion::validate_source(&syntax, &checked).unwrap();
        let expanded = expand_canonical_plot_for_authoring(&checked, "demo", &profiles).unwrap();
        let gear = expanded
            .expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str() == kind)
            .unwrap();
        assert_eq!(gear.outputs[0].value_kind.as_str(), DIMENSION_INFO_ID);
        assert_eq!(
            operation::contract(kind).unwrap().limits.max_queue_bytes,
            DIMENSION_ENCODED_LEN as u32
        );
        let expected = Dimension::basis("travel")
            .unwrap()
            .quotient(Dimension::basis("clock").unwrap())
            .unwrap();
        assert_eq!(
            operation::prepare(kind, &gear.configuration).unwrap(),
            expected
        );
        let wrong = source.replace(&format!("source={value}"), "source=velocity");
        assert!(check_syntax_document(&parse_syntax_document(&wrong), &startup).is_err());
        let quoted = source.replace(&format!("source={value}"), &format!("source=\"{value}\""));
        assert!(check_syntax_document(&parse_syntax_document(&quoted), &startup).is_err());
    }
}

#[test]
fn dimension_kind_startup_configuration_keeps_intrinsic_fore_identity() {
    use conduit_core::{
        kind_id, ConfigurationValue, KindConfigurationRule, KindIdentity,
        StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
    };
    use conduit_plot::{KindSignature, StartupParameterSignature};
    let ty = StructuredInfoType::leaf(kind_id(DIMENSION_INFO_ID)).unwrap();
    let default =
        StructuredInfoValue::leaf(ty.clone(), Dimension::DIMENSIONLESS.encode().to_vec()).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let default =
        StructuredConfigurationValue::new(profile.clone(), default.canonical_bytes().unwrap())
            .unwrap();
    let mut contract = operation::contract(operation::UNIT_KIND).unwrap();
    contract.kind_id = kind_id("test/dimension-startup");
    contract.kind_contract_revision = KindIdentity::from("test/dimension-startup@1");
    contract.startup_parameters[0].value_type = kind_id(DIMENSION_INFO_ID);
    contract.configuration[0].rule = KindConfigurationRule::Structured { profile };
    contract.configuration[0].default_value = ConfigurationValue::Structured(default);
    let mut startup = StartupCatalog::new();
    startup
        .insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "source".into(),
                value_type: "Dimension".into(),
                default: None,
            }],
        })
        .unwrap();
    startup
        .insert_fore(contract.kind_id.as_str(), contract.checked_front())
        .unwrap();
    let mut profiles = ProfileCatalog::new();
    profiles.insert_kind(contract).unwrap();
    let source = format!("{DECLARATIONS}\nplot forward (\n value: Dimension\n dimension: Dimension >>\n) {{\n operation: test/dimension-startup(source=value)\n operation.dimension >> dimension\n}}\nplot demo (\n dimension: Dimension >>\n) {{\n operation: forward(value=velocity)\n operation.dimension >> dimension\n}}.\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "demo", &profiles).unwrap();
    let configured = &expanded.expanded.gears[0].configuration[0].value;
    assert_eq!(configured.semantic_kind().as_str(), DIMENSION_INFO_ID);
    let ConfigurationValue::Structured(configured) = configured else {
        panic!("ordinary finite structured configuration");
    };
    let concrete = StructuredInfoValue::from_canonical_bytes(configured.canonical_value()).unwrap();
    let conduit_core::StructuredInfoValueShape::Leaf(bytes) = concrete.shape() else {
        panic!("Dimension leaf");
    };
    assert_eq!(
        Dimension::decode(bytes).unwrap(),
        Dimension::basis("travel")
            .unwrap()
            .quotient(Dimension::basis("clock").unwrap())
            .unwrap()
    );
}
