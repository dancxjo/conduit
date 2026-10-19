//! Explicit carrier widening keeps physical wire identity and denies bare scalars.
use conduit_core::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program(input: &str, body: &str) -> PortableExpressionProgram {
    let source = format!("plot expose (\n >> value: {input}\n result: Quantity >>\n) = ({body})\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "expose", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program");
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
#[test]
fn checked_physical_carrier_forwards_all_admitted_angle_wire_forms_without_growth() {
    let program = program("Angle", "quantity/value(.)");
    assert!(PreparedPortableExpressionEvaluator::new(&program).is_err());
    assert!(PreparedPortableExpressionEvaluator::new_with_exact_quantities(&program).is_err());
    let mut evaluator =
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&program).unwrap();
    let capacity = evaluator.output_capacity();
    let bounds = evaluator.mathematical_angle_profile_bounds().unwrap();
    assert_eq!((bounds.0, bounds.1), (2, 2));
    let native = Quantity::parse_plot_literal("90°")
        .unwrap()
        .encode()
        .to_vec();
    let mut rational = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(
        RationalQuantity::new(
            ExactRational::ratio(1, 4, &mut work()).unwrap(),
            Unit::Turn,
            QuantityRole::Linear,
        )
        .unwrap(),
        &mut rational,
    )
    .unwrap();
    rational.truncate(n);
    let conversion = PreparedAngleReferenceConversion::from_checked(
        Quantity::parse_plot_literal("90°").unwrap(),
        Unit::Radian,
        "90°",
        "rad",
        &mut work(),
    )
    .unwrap();
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = conversion.denotation().encode(&mut coordinate).unwrap();
    let view = MathematicalValueView::decode(&coordinate[..n], &mut work()).unwrap();
    let mut symbolic = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &view,
        Unit::Radian,
        QuantityRole::Linear,
        &mut symbolic,
        &mut work(),
    )
    .unwrap();
    symbolic.truncate(n);
    let wrong = Quantity::parse_plot_literal("1m").unwrap().encode();
    let (_, observed) = allocation_probe::observe(|| {
        for bytes in [&native, &rational, &symbolic] {
            for _ in 0..8 {
                assert_eq!(evaluator.evaluate(bytes).unwrap(), bytes);
            }
        }
        assert!(evaluator.evaluate(&wrong).is_err());
        assert_eq!(evaluator.evaluate(&symbolic).unwrap(), symbolic);
    });
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(evaluator.output_capacity(), capacity);
    assert_eq!(
        evaluator.mathematical_angle_profile_bounds().unwrap(),
        bounds
    );
}
#[test]
fn generic_carrier_does_not_create_a_physical_value_from_scalar_or_unit_data() {
    for (input, body) in [
        ("Scalar", "quantity/value(.)"),
        ("MathematicalValue", "quantity/value(.)"),
        ("Angle", "quantity/value(42)"),
        ("Angle", "quantity/value(rad)"),
    ] {
        let source =
            format!("plot expose (\n >> value: {input}\n result: Quantity >>\n) = ({body})\n");
        let refused =
            match check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()) {
                Err(_) => true,
                Ok(checked) => {
                    expand_canonical_plot_for_authoring(&checked, "expose", &ProfileCatalog::new())
                        .is_err()
                }
            };
        assert!(refused, "input={input} body={body}");
    }
}
