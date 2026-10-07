use conduit_core::*;
use conduit_plot::*;
fn scalar(ty: &StructuredInfoType, number: f32) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), scalar(representation, number)).unwrap()
        }
        StructuredInfoTypeShape::Leaf(_) => {
            StructuredInfoValue::leaf(ty.clone(), number.to_le_bytes().to_vec()).unwrap()
        }
        _ => panic!("scalar"),
    }
}
fn pair(ty: &StructuredInfoType, left: f32, right: f32) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("pair")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    scalar(
                        f.value_type(),
                        if f.name() == "left" { left } else { right },
                    ),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}
fn program(operator: &str, output: &str) -> PortableExpressionProgram {
    let source=format!("type Finite = F32 finite\ntype Pair = {{\n left: Finite\n right: Finite\n}}\nplot proof (\n >> value: Pair\n result: {output} >>\n) = (.left {operator} .right)\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "proof", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
#[test]
fn finite_f32_arithmetic_preserves_selected_inputs_and_returns_bare_result() {
    for (op, expected) in [
        ("+", 0.75f32),
        ("-", 0.25),
        ("*", 0.125),
        ("/", 2.),
        ("%", 0.),
    ] {
        let p = program(op, "F32");
        assert_eq!(
            p.output_type,
            StructuredInfoType::leaf(kind_id(F32_INFO_ID)).unwrap()
        );
        let input = pair(&p.input_type, 0.5, 0.25);
        let retained = input.clone();
        let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
        assert_eq!(prepared.evaluate(&input).unwrap(), expected.to_le_bytes());
        assert_eq!(p.evaluate(&input).unwrap(), expected.to_le_bytes());
        assert_eq!(input, retained);
        for (left, right) in [(f32::INFINITY, 1.), (1., f32::NEG_INFINITY), (f32::NAN, 1.)] {
            let bad = pair(&p.input_type, left, right);
            assert!(prepared.evaluate(&bad).is_err());
            assert!(p.evaluate(&bad).is_err());
            assert_eq!(prepared.evaluate(&input).unwrap(), expected.to_le_bytes());
        }
    }
    for (op, left, right) in [
        ("+", f32::MAX, f32::MAX),
        ("*", f32::MAX, 2.),
        ("/", 1., 0.),
        ("/", 1., -0.),
        ("%", 1., 0.),
    ] {
        let p = program(op, "F32");
        let bad = pair(&p.input_type, left, right);
        let retained = bad.clone();
        assert!(PreparedPortableExpressionEvaluator::new(&p)
            .unwrap()
            .evaluate(&bad)
            .is_err());
        assert!(p.evaluate(&bad).is_err());
        assert_eq!(bad, retained);
    }
}
#[test]
fn finite_f32_ordering_and_encoded_equality_have_reference_prepared_parity() {
    for (op, left, right, expected) in [
        ("<", -1., 1., true),
        ("<=", -0., 0., true),
        (">", 2., 1., true),
        (">=", 1., 1., true),
        ("==", -0., 0., false),
        ("!=", -0., 0., true),
    ] {
        let p = program(op, "Boolean");
        let input = pair(&p.input_type, left, right);
        let expected = InfoBool::new(expected).encode();
        assert_eq!(
            PreparedPortableExpressionEvaluator::new(&p)
                .unwrap()
                .evaluate(&input)
                .unwrap(),
            expected
        );
        assert_eq!(p.evaluate(&input).unwrap(), expected);
        if matches!(op, "<" | "<=" | ">" | ">=") {
            let invalid = pair(&p.input_type, f32::NAN, right);
            assert!(p.evaluate(&invalid).is_err());
            assert!(PreparedPortableExpressionEvaluator::new(&p)
                .unwrap()
                .evaluate(&invalid)
                .is_err());
        }
    }
}
