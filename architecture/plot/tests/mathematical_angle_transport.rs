use conduit_core::*;
use conduit_plot::rust_binding::{primitive_from_structured, primitive_into_structured};
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn program(source: &str, entry: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn angle(value: MathematicalValue) -> MathematicalAngle {
    MathematicalAngle::from_turn_coordinate(value)
}
fn bytes(value: &MathematicalAngle) -> Vec<u8> {
    let mut bytes = vec![0; MATHEMATICAL_ANGLE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
#[test]
fn exact_angle_crosses_checked_structured_ports_with_zero_play_allocations() {
    let source = "type Envelope = {\n angle: MathematicalAngle\n}\n\
        plot pack (\n >> input: MathematicalAngle\n output: Envelope >>\n) = ({ angle: . })\n\
        plot unpack (\n >> input: Envelope\n output: MathematicalAngle >>\n) = (.angle)\n";
    let pack = program(source, "pack");
    let unpack = program(source, "unpack");
    let mut b = MathematicalValueBuilder::new();
    let q = b
        .insert(
            MathematicalNode::Rational(ExactRational::ratio(3, 4, &mut work()).unwrap()),
            &mut work(),
        )
        .unwrap();
    let root = b
        .insert(MathematicalNode::ArctangentTurns(q), &mut work())
        .unwrap();
    let original = angle(b.finish(root, &mut work()).unwrap());
    let encoded = bytes(&original);
    let native = primitive_into_structured(pack.input_type.clone(), &original).unwrap();
    assert_eq!(
        primitive_from_structured::<MathematicalAngle>(&native).unwrap(),
        original
    );
    let mut pack = PreparedPortableExpressionEvaluator::new(&pack).unwrap();
    let mut unpack = PreparedPortableExpressionEvaluator::new(&unpack).unwrap();
    let (correct, allocations) = allocation_probe::observe(|| {
        (0..100).all(|_| {
            let packed = pack.evaluate(&encoded).unwrap();
            unpack.evaluate(packed).unwrap() == encoded
        })
    });
    assert!(correct);
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    let mut forged = encoded.clone();
    forged[1..1 + UNIT_ENCODED_LEN].copy_from_slice(&Unit::Radian.encode());
    assert!(pack.evaluate(&forged).is_err());
}
#[test]
fn angle_equality_is_proved_or_explicitly_unknown_in_both_evaluators() {
    for operator in ["==", "!="] {
        let source = format!(
            "type Pair = {{\n left: MathematicalAngle\n right: MathematicalAngle\n}}\n\
            plot equal (\n >> input: Pair\n output: Boolean >>\n) = (.left {operator} .right)\n"
        );
        let p = program(&source, "equal");
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = p.input_type.shape() else {
            panic!()
        };
        let input = |left: MathematicalValue, right: MathematicalValue| {
            StructuredInfoValue::record(
                p.input_type.clone(),
                vec![
                    StructuredFieldValue::new(
                        "left",
                        primitive_into_structured(fields[0].value_type().clone(), &angle(left))
                            .unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new(
                        "right",
                        primitive_into_structured(fields[1].value_type().clone(), &angle(right))
                            .unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .canonical_bytes()
            .unwrap()
        };
        let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
        for (left, right, equal) in [
            (pi.clone(), pi.clone(), true),
            (
                MathematicalValue::rational(ExactRational::ZERO),
                MathematicalValue::rational(ExactRational::ONE),
                false,
            ),
        ] {
            let input = input(left, right);
            let expected = InfoBool::new(equal != (operator == "!=")).encode();
            assert_eq!(p.evaluate(&input).unwrap(), expected);
            assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        }
        let input = input(pi, MathematicalValue::rational(ExactRational::from_i128(3)));
        assert_eq!(
            p.evaluate(&input),
            Err(PortableExpressionEvaluationRefusal::UnprovedMathematicalComparison)
        );
        assert_eq!(
            prepared.evaluate(&input),
            Err(PortableExpressionEvaluationRefusal::UnprovedMathematicalComparison)
        );
    }
}

#[test]
fn exact_angle_cannot_implicitly_narrow_or_lose_its_unit() {
    for output in ["Angle", "Quantity", "MathematicalValue", "Scalar", "F64"] {
        let source=format!("plot wrong (\n >> input: MathematicalAngle\n output: {output} >>\n) {{\n input >> output\n}}\n");
        let syntax = parse_syntax_document(&source);
        assert!(
            check_syntax_document(&syntax, &StartupCatalog::new()).is_err(),
            "unexpected implicit projection to {output}"
        );
    }
    let source="plot wrong (\n >> input: MathematicalValue\n output: MathematicalAngle >>\n) {\n input >> output\n}\n";
    assert!(check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).is_err());
}
