use conduit_core::*;
use conduit_plot::rust_binding::primitive_into_structured;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn program(source: &str) -> PortableExpressionProgram {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "inverse", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!()
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
fn source(input: &str, body: &str) -> String {
    format!("plot inverse (\n >> input: {input}\n output: MathematicalAngle >>\n) = ({body})\n")
}
fn bytes(value: MathematicalValue) -> Vec<u8> {
    let a = MathematicalAngle::from_turn_coordinate(value);
    let mut out = vec![0; MATHEMATICAL_ANGLE_MAX_ENCODED_BYTES];
    let n = a.encode(&mut out).unwrap();
    out.truncate(n);
    out
}
fn pair(program: &PortableExpressionProgram, y: &str, x: &str) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!()
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "y",
                primitive_into_structured(
                    fields[0].value_type().clone(),
                    &Quantity::parse_plot_literal(y).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
            StructuredFieldValue::new(
                "x",
                primitive_into_structured(
                    fields[1].value_type().clone(),
                    &Quantity::parse_plot_literal(x).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn pair_source(kind: &str) -> String {
    format!(
        "type Pair = {{\n y: {kind}\n x: {kind}\n}}\n{}",
        source("Pair", "math/atan2(.y, .x)")
    )
}
#[test]
fn authored_unary_inverse_calls_retain_exact_nodes_without_play_growth() {
    for (name, node) in [
        ("math/atan", MathematicalNode::ArctangentTurns as fn(_) -> _),
        ("math/asin", MathematicalNode::ArcsineTurns),
        ("math/acos", MathematicalNode::ArccosineTurns),
    ] {
        let p = program(&source("MathematicalValue", &format!("{name}(.)")));
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        let capacity = prepared.output_capacity();
        for n in [-3, 0, 3] {
            let q = ExactRational::ratio(n, 5, &mut work()).unwrap();
            let input = MathematicalValue::rational(q);
            let mut encoded = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
            let len = input.encode(&mut encoded).unwrap();
            encoded.truncate(len);
            let mut b = MathematicalValueBuilder::new();
            let a = b
                .insert(MathematicalNode::Rational(q), &mut work())
                .unwrap();
            let root = b.insert(node(a), &mut work()).unwrap();
            let expected = bytes(b.finish(root, &mut work()).unwrap());
            assert_eq!(p.evaluate(&encoded).unwrap(), expected);
            let (correct, allocations) = allocation_probe::observe(|| {
                (0..100).all(|_| prepared.evaluate(&encoded).unwrap() == expected)
            });
            assert!(correct);
            assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
            assert_eq!(prepared.output_capacity(), capacity);
        }
    }
}
#[test]
fn authored_atan2_converts_length_units_and_refuses_zero_or_changed_dimensions() {
    let p = program(&pair_source("Distance"));
    let input = pair(&p, "3m", "400cm");
    let expected = QuantityAtan2Denotation::from_checked(
        Atan2QuantityInputs {
            y: Quantity::parse_plot_literal("3m").unwrap(),
            original_y: "3m",
            x: Quantity::parse_plot_literal("400cm").unwrap(),
            original_x: "400cm",
            common_unit: Unit::resolve("m").unwrap(),
            common_unit_source: "m",
        },
        Atan2Branch::PrincipalSignedHalfTurn,
        &mut work(),
    )
    .unwrap();
    let expected = bytes(expected.coordinate().clone());
    assert_eq!(p.evaluate(&input).unwrap(), expected);
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let (correct, allocations) = allocation_probe::observe(|| {
        (0..100).all(|_| prepared.evaluate(&input).unwrap() == expected)
    });
    assert!(correct);
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
    let zero = pair(&p, "0m", "0cm");
    let refusal = PortableExpressionEvaluationRefusal::InverseAngle(
        InverseAngleEncodingRefusal::Quantity(Atan2Refusal::UndefinedZeroPair),
    );
    assert_eq!(p.evaluate(&zero), Err(refusal.clone()));
    assert_eq!(prepared.evaluate(&zero), Err(refusal));
    assert_eq!(prepared.evaluate(&input).unwrap(), expected);
    let generic = program(&pair_source("Quantity"));
    let wrong = pair(&generic, "3m", "4s");
    let refusal = PortableExpressionEvaluationRefusal::InverseAngle(
        InverseAngleEncodingRefusal::Quantity(Atan2Refusal::MismatchedDimensions),
    );
    assert_eq!(generic.evaluate(&wrong), Err(refusal.clone()));
    assert_eq!(
        PreparedPortableExpressionEvaluator::new(&generic)
            .unwrap()
            .evaluate(&wrong),
        Err(refusal)
    );
}
#[test]
fn literal_calls_are_prepared_exactly_and_known_domain_failures_refuse() {
    for (body, n, d) in [
        ("math/atan(1)", 1, 8),
        ("math/asin(0.5)", 1, 12),
        ("math/acos(0)", 1, 4),
    ] {
        let p = program(&source("Empty", body));
        let expected = bytes(MathematicalValue::rational(
            ExactRational::ratio(n, d, &mut work()).unwrap(),
        ));
        assert_eq!(p.evaluate(&[]).unwrap(), expected);
        assert_eq!(
            PreparedPortableExpressionEvaluator::new(&p)
                .unwrap()
                .evaluate(&[])
                .unwrap(),
            expected
        );
    }
    let p = program(&source("Empty", "math/atan2(3m,400cm)"));
    let actual = p.evaluate(&[]).unwrap();
    assert_eq!(
        PreparedPortableExpressionEvaluator::new(&p)
            .unwrap()
            .evaluate(&[])
            .unwrap(),
        actual
    );
    for (body, reason) in [
        (
            "math/asin(2)",
            InverseAngleEncodingRefusal::Denotation(
                MathematicalValueRefusal::InverseTrigonometricDomain,
            ),
        ),
        (
            "math/atan2(0m,0m)",
            InverseAngleEncodingRefusal::Quantity(Atan2Refusal::UndefinedZeroPair),
        ),
    ] {
        let p = program(&source("Empty", body));
        let expected = PortableExpressionEvaluationRefusal::InverseAngle(reason);
        assert_eq!(p.evaluate(&[]), Err(expected.clone()));
        match PreparedPortableExpressionEvaluator::new(&p) {
            Err(why) => assert_eq!(why, expected),
            Ok(_) => panic!("known domain failure was admitted"),
        }
    }
}
#[test]
fn checker_refuses_incompatible_dimensions_bare_scalars_units_and_arity() {
    for body in [
        "math/atan2(3m,4s)",
        "math/atan2(3,4)",
        "math/atan2(3m)",
        "math/asin(3m)",
        "math/acos()",
    ] {
        let syntax = parse_syntax_document(&source("Empty", body));
        assert!(
            syntax.diagnostics.is_empty(),
            "{body}: {:?}",
            syntax.diagnostics
        );
        let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "inverse", &ProfileCatalog::new())
                .is_err(),
            "{body}"
        );
    }
}

#[test]
fn bare_literal_zero_pair_has_undefined_domain_without_inferred_units() {
    for body in ["math/atan2(0,0)", "math/atan2(0.0,0)"] {
        let checked = check_syntax_document(
            &parse_syntax_document(&source("Empty", body)),
            &StartupCatalog::new(),
        )
        .unwrap();
        let error =
            expand_canonical_plot_for_authoring(&checked, "inverse", &ProfileCatalog::new())
                .unwrap_err();
        assert!(
            error.message.contains("undefined for the zero pair"),
            "{}",
            error.message
        );
    }
}
