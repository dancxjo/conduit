#[path = "prepared_structured_payload/allocation.rs"]
mod allocation;
use conduit_core::{
    ConfigurationValue, InfoBool, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
fn program(operator: &str) -> PortableExpressionProgram {
    let source = format!("type Details = {{\n    code: I32\n}}\ntype Feature =\n    boolean Boolean\n    | number F64\n    | vector sequence F32 in 1..=8\n    | text Text <= 128B\n    | category Text <= 64B\n    | details Details\ntype Pair = {{\n    left: Feature\n    right: Feature\n}}\nplot equal (\n    >> input: Pair\n    result: Boolean >>\n) = (.left {operator} .right)");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "equal", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn field<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn payload<'a>(ty: &'a StructuredInfoType, tag: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type()
}
fn value(ty: &StructuredInfoType, tag: &str, bytes: &[u8]) -> StructuredInfoValue {
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        StructuredInfoValue::leaf(payload(ty, tag).clone(), bytes.to_vec()).unwrap(),
    )
    .unwrap()
}
fn evaluate(
    p: &PortableExpressionProgram,
    left: StructuredInfoValue,
    right: StructuredInfoValue,
) -> bool {
    let input = StructuredInfoValue::record(
        p.input_type.clone(),
        vec![
            StructuredFieldValue::new("left", left).unwrap(),
            StructuredFieldValue::new("right", right).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let expected = p.evaluate(&input).unwrap();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(p).unwrap();
    assert_eq!(prepared.evaluate(&input).unwrap(), expected);
    InfoBool::decode(&expected).unwrap().get()
}
#[test]
fn variants_compare_tags_and_ieee_bits_instead_of_float_arithmetic() {
    for operator in ["==", "!="] {
        let p = program(operator);
        let ty = field(&p.input_type, "left");
        let values = [
            value(ty, "boolean", &[1]),
            value(ty, "text", b"true"),
            value(ty, "category", b"true"),
            value(ty, "number", &0_u64.to_le_bytes()),
            value(ty, "number", &(1_u64 << 63).to_le_bytes()),
            value(ty, "number", &0x7ff8_0000_0000_0001_u64.to_le_bytes()),
            value(ty, "number", &0x7ff8_0000_0000_0002_u64.to_le_bytes()),
        ];
        for (i, left) in values.iter().enumerate() {
            for (j, right) in values.iter().enumerate() {
                assert_eq!(
                    evaluate(&p, left.clone(), right.clone()),
                    (i == j) == (operator == "==")
                );
            }
        }
    }
}

#[test]
fn vectors_compare_actual_length_order_and_exact_f32_members() {
    let p = program("==");
    let ty = field(&p.input_type, "left");
    let vector_type = payload(ty, "vector");
    let StructuredInfoTypeShape::Sequence { element, .. } = vector_type.shape() else {
        panic!("sequence")
    };
    let vector = |bits: &[u32]| {
        StructuredInfoValue::variant(
            ty.clone(),
            "vector",
            StructuredInfoValue::sequence(
                vector_type.clone(),
                bits.iter()
                    .map(|bits| {
                        StructuredInfoValue::leaf(element.clone(), bits.to_le_bytes().to_vec())
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let vectors = [
        vector(&[0]),
        vector(&[1 << 31]),
        vector(&[0, 1]),
        vector(&[1, 0]),
        vector(&[0; 8]),
    ];
    for (i, left) in vectors.iter().enumerate() {
        for (j, right) in vectors.iter().enumerate() {
            assert_eq!(evaluate(&p, left.clone(), right.clone()), i == j);
        }
    }
    assert!(StructuredInfoValue::sequence(vector_type.clone(), vec![]).is_err());
    assert!(StructuredInfoValue::sequence(
        vector_type.clone(),
        (0..9)
            .map(
                |_| StructuredInfoValue::leaf(element.clone(), 0_u32.to_le_bytes().to_vec())
                    .unwrap()
            )
            .collect()
    )
    .is_err());
}

#[test]
fn nested_records_compare_each_exact_member() {
    let p = program("==");
    let ty = field(&p.input_type, "left");
    let details_type = payload(ty, "details");
    let details = |code: i32| {
        StructuredInfoValue::variant(
            ty.clone(),
            "details",
            StructuredInfoValue::record(
                details_type.clone(),
                vec![StructuredFieldValue::new(
                    "code",
                    StructuredInfoValue::leaf(
                        field(details_type, "code").clone(),
                        code.to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                )
                .unwrap()],
            )
            .unwrap(),
        )
        .unwrap()
    };
    assert!(evaluate(&p, details(1), details(1)));
    assert!(!evaluate(&p, details(1), details(2)));
}

#[test]
fn structured_equality_reuses_complete_input_and_refuses_foreign_or_malformed_bytes() {
    let p = program("==");
    let ty = field(&p.input_type, "left");
    let make = |left, right| {
        StructuredInfoValue::record(
            p.input_type.clone(),
            vec![
                StructuredFieldValue::new("left", value(ty, "boolean", &[left])).unwrap(),
                StructuredFieldValue::new("right", value(ty, "boolean", &[right])).unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    };
    let foreign = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap(),
        77u64.to_le_bytes().to_vec(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let inputs = [make(1, 1), make(1, 0), foreign, vec![0]];
    let expected = inputs
        .iter()
        .map(|input| p.evaluate(input))
        .collect::<Vec<_>>();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = prepared.output_capacity();
    assert_eq!(
        allocation::allocations(|| {
            for _ in 0..1000 {
                for (input, expected) in inputs.iter().zip(&expected) {
                    assert_eq!(
                        prepared.evaluate(input),
                        expected.as_ref().map(Vec::as_slice).map_err(Clone::clone)
                    );
                }
            }
        }),
        0
    );
    assert_eq!(prepared.output_capacity(), capacity);
}

#[test]
fn structured_quantity_equality_compares_meaning_across_units() {
    let source = "type Measurement = {\n amount: Distance\n}\ntype Pair = {\n left: Measurement\n right: Measurement\n}\nplot equal (\n value: Pair >> result: Boolean\n) = (.left == .right)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "equal", &ProfileCatalog::new())
        .unwrap()
        .expanded;
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!()
    };
    let p = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let ty = field(&p.input_type, "left");
    let make = |quantity: conduit_core::Quantity| {
        StructuredInfoValue::record(
            ty.clone(),
            vec![StructuredFieldValue::new(
                "amount",
                StructuredInfoValue::leaf(field(ty, "amount").clone(), quantity.encode().to_vec())
                    .unwrap(),
            )
            .unwrap()],
        )
        .unwrap()
    };
    let meter = make(conduit_core::Quantity::new(
        1,
        conduit_core::QuantityUnit::Meter,
    ));
    let millimeters = make(conduit_core::Quantity::new(
        1000,
        conduit_core::QuantityUnit::Millimeter,
    ));
    assert_ne!(
        meter.canonical_bytes().unwrap(),
        millimeters.canonical_bytes().unwrap()
    );
    assert!(evaluate(&p, meter, millimeters));
}
