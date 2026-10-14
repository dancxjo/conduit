use conduit_core::{
    ConfigurationValue, ExactNumericWork as Work, ExactRational as Q, MathematicalNode as Node,
    MathematicalValue as Value, MathematicalValueBuilder as Builder,
    MATHEMATICAL_VALUE_MAX_ENCODED_BYTES, MATHEMATICAL_VALUE_VALIDATION_WORK,
};
use conduit_plot::rust_binding::{primitive_from_structured, primitive_into_structured};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, PreparedPortableExpressionEvaluator, ProfileCatalog, StartupCatalog,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn programs() -> [PortableExpressionProgram; 3] {
    let source = "type Envelope = {\n value: MathematicalValue\n}\n\
        plot pack (\n >> input: MathematicalValue\n output: Envelope >>\n) = ({ value: . })\n\
        plot relay (\n >> input: Envelope\n output: Envelope >>\n) {\n input >> (.) >> output\n}\n\
        plot unpack (\n >> input: Envelope\n output: MathematicalValue >>\n) = (.value)\n";
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    ["pack", "relay", "unpack"].map(|entry| {
        let expanded =
            expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!()
        };
        PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
    })
}
#[test]
fn symbolic_roots_and_pi_cross_typed_prepared_ports_without_allocating() {
    let mut builder = Builder::new();
    let mut work = Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
    let two = builder
        .insert(Node::Rational(Q::from_i128(2)), &mut work)
        .unwrap();
    let root = builder
        .insert(Node::PrincipalSquareRoot(two), &mut work)
        .unwrap();
    let root = builder.finish(root, &mut work).unwrap();
    let pi = Value::parse_literal("π", &mut work).unwrap();
    let [pack, relay, unpack] = programs();
    for value in [
        root,
        Value::parse_literal(&format!("1{}1", "0".repeat(999)), &mut work).unwrap(),
        pi,
        Value::rational(Q::ratio(1, 3, &mut work).unwrap()),
    ] {
        let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let length = value.encode(&mut bytes).unwrap();
        bytes.truncate(length);
        let native = primitive_into_structured(pack.input_type.clone(), &value).unwrap();
        assert_eq!(primitive_from_structured::<Value>(&native).unwrap(), value);
        let mut pack = PreparedPortableExpressionEvaluator::new(&pack).unwrap();
        let mut relay = PreparedPortableExpressionEvaluator::new(&relay).unwrap();
        let mut unpack = PreparedPortableExpressionEvaluator::new(&unpack).unwrap();
        let (correct, allocations) = allocation_probe::observe(|| {
            (0..100).all(|_| {
                let packed = pack.evaluate(&bytes).unwrap();
                let relayed = relay.evaluate(packed).unwrap();
                unpack.evaluate(relayed).unwrap() == bytes
            })
        });
        assert!(correct);
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
        let mut forged = bytes.clone();
        forged[0] = 255;
        assert!(pack.evaluate(&forged).is_err());
    }
}
#[test]
fn mathematical_values_do_not_become_quantities_or_ieee_values() {
    for other in ["Quantity", "Unit", "Dimension", "Angle", "Scalar", "F64"] {
        let syntax = parse_syntax_document(&format!("plot wrong (\n >> input: MathematicalValue\n output: {other} >>\n) {{\n input >> output\n}}\n"));
        assert!(check_syntax_document(&syntax, &StartupCatalog::new()).is_err());
    }
}

#[test]
fn authored_closed_literals_keep_exact_rational_and_named_constant_encodings() {
    for literal in ["0.1", "π", "pi", "τ", "tau"] {
        let source = format!(
            "plot literal (\n >> input: Empty\n output: MathematicalValue >>\n) = ({literal})\n"
        );
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "literal", &ProfileCatalog::new())
                .unwrap();
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!()
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let output = program.evaluate(&[]).unwrap();
        let actual =
            Value::decode(&output, &mut Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK)).unwrap();
        assert_eq!(
            actual,
            Value::parse_literal(literal, &mut Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK))
                .unwrap()
        );
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let (same, allocations) =
            allocation_probe::observe(|| evaluator.evaluate(&[]).unwrap() == output);
        assert!(same);
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    }
}

#[test]
fn ordinary_equality_refuses_unknown_in_both_evaluators() {
    use conduit_core::{
        InfoBool, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
    };
    for operator in ["==", "!="] {
        let source = format!(
            "type Pair = {{\n left: MathematicalValue\n right: MathematicalValue\n}}\n\
            plot compare (\n >> input: Pair\n output: Boolean >>\n) = (.left {operator} .right)\n"
        );
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "compare", &ProfileCatalog::new())
                .unwrap();
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!()
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
            panic!()
        };
        let make_input = |left: Value, right: Value| {
            StructuredInfoValue::record(
                program.input_type.clone(),
                vec![
                    StructuredFieldValue::new(
                        "left",
                        primitive_into_structured(fields[0].value_type().clone(), &left).unwrap(),
                    )
                    .unwrap(),
                    StructuredFieldValue::new(
                        "right",
                        primitive_into_structured(fields[1].value_type().clone(), &right).unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
            .canonical_bytes()
            .unwrap()
        };
        let pi =
            Value::parse_literal("π", &mut Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK)).unwrap();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        for (left, right, equal) in [
            (pi.clone(), pi.clone(), true),
            (Value::rational(Q::ONE), Value::rational(Q::ZERO), false),
        ] {
            let input = make_input(left, right);
            let expected = InfoBool::new(equal != (operator == "!=")).encode();
            assert_eq!(program.evaluate(&input).unwrap(), expected);
            assert_eq!(prepared.evaluate(&input).unwrap(), expected);
        }
        let input = make_input(pi, Value::rational(Q::from_i128(3)));
        let expected =
            conduit_plot::PortableExpressionEvaluationRefusal::UnprovedMathematicalComparison;
        assert_eq!(program.evaluate(&input), Err(expected.clone()));
        assert_eq!(prepared.evaluate(&input), Err(expected));
    }
}

#[test]
fn checked_source_preserves_large_integers_and_exact_thirds() {
    let huge = format!("1{}1", "0".repeat(999));
    let power = format!("1{}", "0".repeat(1000));
    let mut work = Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
    let third = Value::rational(Q::ratio(1, 3, &mut work).unwrap());
    for (expression, input, expected) in [
        (". - .".to_owned(), third.clone(), Value::rational(Q::ZERO)),
        (". * 3".to_owned(), third.clone(), Value::rational(Q::ONE)),
        (". / .".to_owned(), third.clone(), Value::rational(Q::ONE)),
        (
            format!("{power} + 1"),
            Value::rational(Q::ZERO),
            Value::parse_literal(&huge, &mut work).unwrap(),
        ),
        (
            "1 / 3 + 1 / 3 + 1 / 3".to_owned(),
            Value::rational(Q::ZERO),
            Value::rational(Q::ONE),
        ),
        (
            huge.clone(),
            Value::rational(Q::ZERO),
            Value::parse_literal(&huge, &mut work).unwrap(),
        ),
        (
            ". + . + .".to_owned(),
            third.clone(),
            Value::rational(Q::ONE),
        ),
        (".".to_owned(), third.clone(), third),
    ] {
        let source = format!("plot exact (\n >> input: MathematicalValue\n output: MathematicalValue >>\n) = ({expression})\n");
        let syntax = parse_syntax_document(&source);
        assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "exact", &ProfileCatalog::new()).unwrap();
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!()
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let mut input_bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let length = input.encode(&mut input_bytes).unwrap();
        input_bytes.truncate(length);
        let bytes = program.evaluate(&input_bytes).unwrap();
        let value =
            Value::decode(&bytes, &mut Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK)).unwrap();
        assert_eq!(value, expected);
        if expression.contains(&power) {
            use conduit_core::{NumericalAlgorithm, PreparedNumericalRealization};
            assert!(PreparedNumericalRealization::prepare(
                &value,
                NumericalAlgorithm::OutwardI64,
                Q::ZERO,
                &mut work
            )
            .is_err());
            assert!(PreparedNumericalRealization::prepare(
                &value,
                NumericalAlgorithm::ExactRational,
                Q::ZERO,
                &mut work
            )
            .is_ok());
        }
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let (same, allocations) = allocation_probe::observe(|| {
            (0..100).all(|_| prepared.evaluate(&input_bytes).unwrap() == bytes)
        });
        assert!(same);
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    }
}

#[test]
fn exact_arithmetic_refuses_zero_division_extent_overflow_and_symbolic_operands() {
    use conduit_plot::PortableExpressionEvaluationRefusal as R;
    for (expression, literal) in [
        (". / 0", "1".to_owned()),
        (". * .", format!("1{}", "0".repeat(1000))),
        (". + .", "π".to_owned()),
    ] {
        let source = format!("plot refuse (\n >> input: MathematicalValue\n output: MathematicalValue >>\n) = ({expression})\n");
        let syntax = parse_syntax_document(&source);
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        let expanded =
            expand_canonical_plot_for_authoring(&checked, "refuse", &ProfileCatalog::new())
                .unwrap();
        let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!()
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let value =
            Value::parse_literal(&literal, &mut Work::new(MATHEMATICAL_VALUE_VALIDATION_WORK))
                .unwrap();
        let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let length = value.encode(&mut bytes).unwrap();
        bytes.truncate(length);
        assert_eq!(program.evaluate(&bytes), Err(R::Arithmetic));
        let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        let (refused, allocations) =
            allocation_probe::observe(|| prepared.evaluate(&bytes) == Err(R::Arithmetic));
        assert!(refused);
        assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    }
}
