//! Finite authored numerical bounds preserve mathematical meaning in transports.
use conduit_core::{kind_id, MathValue, PortableNumericProfile, StructuredInfoTypeShape};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};

#[test]
fn authored_scalar_alias_captures_an_intrinsic_bounded_representation() {
    let syntax = parse_syntax_document("type SampleScalar = Scalar <= 128\n");
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let value_type = &checked.native_types[0].value_type;
    let StructuredInfoTypeShape::Nominal { representation, .. } = value_type.shape() else {
        panic!("authored alias retains name custody");
    };
    let StructuredInfoTypeShape::Leaf(kind) = representation.shape() else {
        panic!("bounded numerical leaf");
    };
    let profile = PortableNumericProfile::from_info_id(kind.as_str()).unwrap();
    assert_eq!(profile.maximum_encoded_bytes, 128);
    let third = MathValue::rational("1", "3")
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert!(conduit_core::validate_primitive_info(kind.as_str(), &third).is_ok());
    let large = MathValue::integer(10)
        .checked_pow(1000)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    assert!(conduit_core::validate_primitive_info(kind.as_str(), &large).is_err());
    assert_eq!(kind, &kind_id(&profile.info_id().unwrap()));
}

#[test]
fn direct_scalar_front_bound_changes_the_intrinsic_port_contract() {
    let syntax = parse_syntax_document("plot sample (\n >> input: Scalar <= 128\n output: Scalar <= 128 >>\n) {\n input >> output\n}.\n");
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    for port in checked.plots[0]
        .runtime_front
        .inputs()
        .iter()
        .chain(&checked.plots[0].runtime_front.outputs())
    {
        let profile = PortableNumericProfile::from_info_id(port.value_kind.as_str()).unwrap();
        assert_eq!(profile.maximum_encoded_bytes, 128);
    }
}
