//! Checked protocol conversion and the exact Source-authored native Forms.
use conduit_core::{
    ConfigurationValue, Quantity, StructuredFieldValue as Field, StructuredInfoType as Type,
    StructuredInfoTypeShape as Shape, StructuredInfoValue as Value,
    StructuredInfoValueShape as ValueShape,
};
use conduit_plot::*;
const SOURCE: &str = concat!(
    include_str!("../../../plots/device-protocols/main.conduit"),
    "\n",
    include_str!("../../../plots/device-protocols/bme280-quantity-coordinates.conduit"),
);
fn checked() -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap()
}
fn program(source: &CheckedSyntaxDocument) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(
        source,
        "bme280-quantity-coordinates",
        &ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("checked program");
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn variant_type<'a>(ty: &'a Type, tag: &str) -> &'a Type {
    let Shape::Variant { cases, .. } = ty.shape() else {
        panic!("variant");
    };
    cases
        .iter()
        .find(|v| v.tag() == tag)
        .unwrap()
        .payload_type()
}
fn record(ty: &Type, numbers: &[(&str, i128)]) -> Value {
    let Shape::Record { fields, .. } = ty.shape() else {
        panic!("record");
    };
    Value::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                let number = numbers
                    .iter()
                    .find(|(name, _)| *name == f.name())
                    .unwrap()
                    .1;
                Field::new(
                    f.name(),
                    Value::leaf(f.value_type().clone(), number.to_le_bytes().to_vec()).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn input(program: &PortableExpressionProgram, pressure: i128) -> Vec<u8> {
    Value::variant(
        program.input_type.clone(),
        "observation",
        record(
            variant_type(&program.input_type, "observation"),
            &[
                ("temperature_centidegrees", 2508),
                ("pressure_q24_8_pa", pressure),
                ("humidity_q22_10_percent", 47500),
            ],
        ),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
#[test]
fn compensated_values_reach_standard_quantities_without_rounding() {
    let source = checked();
    let program = program(&source);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let result =
        Value::from_canonical_bytes(evaluator.evaluate(&input(&program, 25767233)).unwrap())
            .unwrap();
    let ValueShape::Variant { tag, payload } = result.shape() else {
        panic!("result");
    };
    assert_eq!(tag, "coordinates");
    let ValueShape::Record(fields) = payload.shape() else {
        panic!("coordinates");
    };
    for (field, form, expected) in [
        ("temperature", "device/bme280-temperature", "25.08°C"),
        ("pressure", "device/bme280-pressure", "100653.25390625Pa"),
        ("humidity", "device/bme280-humidity", "46.38671875%"),
    ] {
        let value = fields.iter().find(|f| f.name() == field).unwrap().value();
        let ValueShape::Leaf(bytes) = value.shape() else {
            panic!("coefficient");
        };
        let CheckedTypeFormStorage::DecimalQuantity(codec) = &source
            .type_forms
            .iter()
            .find(|f| f.name == form)
            .unwrap()
            .storage
        else {
            panic!("quantity Form");
        };
        assert_eq!(
            codec.decode(bytes).unwrap(),
            Quantity::parse_plot_literal(expected).unwrap()
        );
    }
}
#[test]
fn invalid_coordinates_refuse_and_unavailable_retains_its_exact_failure() {
    let source = checked();
    let program = program(&source);
    let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    assert!(evaluator.evaluate(&input(&program, i128::MAX)).is_err());
    let failure_type = variant_type(&program.input_type, "unavailable");
    let empty_type = variant_type(failure_type, "sample-disabled");
    let failure = Value::variant(
        failure_type.clone(),
        "sample-disabled",
        Value::leaf(empty_type.clone(), vec![]).unwrap(),
    )
    .unwrap();
    let input = Value::variant(program.input_type.clone(), "unavailable", failure.clone())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let output = Value::from_canonical_bytes(evaluator.evaluate(&input).unwrap()).unwrap();
    let ValueShape::Variant { tag, payload } = output.shape() else {
        panic!("failure result");
    };
    assert_eq!(tag, "unavailable");
    assert_eq!(payload, &failure);
}
