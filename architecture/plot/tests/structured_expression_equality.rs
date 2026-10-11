#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
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
    let expected = InfoBool::decode(&p.evaluate(&input).unwrap())
        .unwrap()
        .get();
    let mut prepared = conduit_plot::PreparedPortableExpressionEvaluator::new(p).unwrap();
    let capacity = prepared.output_capacity();
    let (actual, storage) = allocation_probe::observe(|| {
        InfoBool::decode(prepared.evaluate(&input).unwrap())
            .unwrap()
            .get()
    });
    assert_eq!(actual, expected);
    assert_eq!(storage.allocations, 0);
    assert_eq!(storage.reallocations, 0);
    assert_eq!(storage.peak_bytes, 0);
    assert_eq!(prepared.output_capacity(), capacity);
    expected
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
fn malformed_structured_input_refuses_without_allocation() {
    let mut prepared =
        conduit_plot::PreparedPortableExpressionEvaluator::new(&program("==")).unwrap();
    let (refused, storage) = allocation_probe::observe(|| prepared.evaluate(&[0, 1, 2]).is_err());
    assert!(refused);
    assert_eq!(storage.allocations, 0);
    assert_eq!(storage.reallocations, 0);
}

#[test]
fn nested_quantity_equality_keeps_exact_unit_conversion_law() {
    use conduit_core::{Quantity, Unit};
    let source = "type Pair = {\n left: collection Quantity = 1\n right: collection Quantity = 1\n}\nplot equal (\n >> input: Pair\n result: Boolean >>\n) = (.left == .right)";
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
    let StructuredInfoTypeShape::Collection { element, .. } = ty.shape() else {
        panic!()
    };
    let value = |quantity: Quantity| {
        StructuredInfoValue::collection(
            ty.clone(),
            vec![StructuredInfoValue::leaf(element.clone(), quantity.encode().to_vec()).unwrap()],
        )
        .unwrap()
    };
    assert!(evaluate(
        &p,
        value(Quantity::new(1, Unit::Second)),
        value(Quantity::new(1000, Unit::Millisecond))
    ));
    assert!(!evaluate(
        &p,
        value(Quantity::new(1, Unit::Second)),
        value(Quantity::new(999, Unit::Millisecond))
    ));
}

#[test]
fn foreign_operand_type_and_non_boolean_structured_comparison_refuse_before_play() {
    use conduit_plot::PortableExpressionOperation;
    let mut foreign = program("==");
    let PortableExpressionOperation::Binary { right, .. } = &mut foreign.root.operation else {
        panic!()
    };
    right.value_type = foreign.input_type.clone();
    assert!(conduit_plot::PreparedPortableExpressionEvaluator::new(&foreign).is_err());
    let mut wrong_output = program("==");
    wrong_output.output_type = StructuredInfoType::leaf("value/count".into()).unwrap();
    wrong_output.root.value_type = wrong_output.output_type.clone();
    assert!(conduit_plot::PreparedPortableExpressionEvaluator::new(&wrong_output).is_err());
}
