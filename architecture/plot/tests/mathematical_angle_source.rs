//! Private explicit Angle profile; not an installed Host admission proof.
use conduit_core::*;
use conduit_plot::*;
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn program(input: &str, output: &str, body: &str) -> PortableExpressionProgram {
    let source = format!("plot bridge (\n >> angle: {input}\n result: {output} >>\n) = ({body})\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "bridge", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("program");
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
#[test]
fn authored_angle_survives_a_checked_conversion_plot_carry_plot_and_exact_sine_consumer() {
    let conversion = program("Angle", "Angle", "math/angle-radians(.)");
    assert_eq!(
        conversion.input_type,
        StructuredInfoType::leaf(kind_id(ANGLE_INFO_ID)).unwrap()
    );
    assert_eq!(
        conversion.output_type,
        StructuredInfoType::leaf(kind_id(ANGLE_INFO_ID)).unwrap()
    );
    let carry = program("Angle", "Angle", ".");
    let sine = program("Angle", "MathematicalValue", "math/sin(.)");
    let combined = program(
        "Angle",
        "MathematicalValue",
        "math/sin(math/angle-radians(.))",
    );
    let mut conversion =
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&conversion).unwrap();
    let mut carry =
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&carry).unwrap();
    let mut sine =
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&sine).unwrap();
    let mut combined =
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&combined).unwrap();
    let capacities = [
        conversion.output_capacity(),
        carry.output_capacity(),
        sine.output_capacity(),
        combined.output_capacity(),
    ];
    let bounds = [
        conversion.mathematical_angle_profile_bounds().unwrap(),
        carry.mathematical_angle_profile_bounds().unwrap(),
        sine.mathematical_angle_profile_bounds().unwrap(),
        combined.mathematical_angle_profile_bounds().unwrap(),
    ];
    assert_eq!(bounds.map(|b| (b.0, b.1)), [(2, 2), (1, 1), (2, 2), (3, 3)]);
    // All independently prepared buffers must be covered, including the
    // intermediate physical frame and its numerical coordinate scratch.
    assert!(
        bounds[0].2
            >= capacities[0]
                + MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES
                + 10
                + EXACT_RATIONAL_MAX_ENCODED_BYTES
    );
    assert!(
        bounds[3].2
            >= capacities[3]
                + MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES
                + 18
                + 2 * EXACT_RATIONAL_MAX_ENCODED_BYTES
    );
    assert!(conversion.exact_quantity_profile_bounds().is_none());
    for b in bounds {
        assert!(b.3 > 0);
    }
    for (spelling, expected) in [
        ("0.25turn", ExactRational::ONE),
        ("90°", ExactRational::ONE),
        ("0.5turn", ExactRational::ZERO),
    ] {
        let input = Quantity::parse_plot_literal(spelling).unwrap().encode();
        let receipt = PreparedAngleReferenceConversion::from_checked(
            Quantity::decode(&input).unwrap(),
            Unit::Radian,
            spelling,
            "rad",
            &mut work(),
        )
        .unwrap();
        let mut expected_coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
        let n = receipt
            .denotation()
            .encode(&mut expected_coordinate)
            .unwrap();
        expected_coordinate.truncate(n);
        let (_, storage) = allocation_probe::observe(|| {
            for _ in 0..8 {
                let converted = conversion.evaluate(&input).unwrap();
                let view = MathematicalQuantityView::decode(converted, &mut work()).unwrap();
                assert_eq!(view.unit(), Unit::Radian);
                assert_eq!(view.role(), QuantityRole::Linear);
                assert_eq!(view.family(), Unit::Turn.family());
                assert_eq!(view.coordinate().encoded(), expected_coordinate);
                let transported = carry.evaluate(converted).unwrap();
                assert_eq!(transported, converted);
                let result = sine.evaluate(transported).unwrap();
                assert_eq!(
                    MathematicalValueView::decode(result, &mut work())
                        .unwrap()
                        .as_rational(&mut work())
                        .unwrap(),
                    Some(expected)
                );
                assert_eq!(combined.evaluate(&input).unwrap(), result);
            }
        });
        assert_eq!((storage.allocations, storage.reallocations), (0, 0));
        assert_eq!(
            bounds,
            [
                conversion.mathematical_angle_profile_bounds().unwrap(),
                carry.mathematical_angle_profile_bounds().unwrap(),
                sine.mathematical_angle_profile_bounds().unwrap(),
                combined.mathematical_angle_profile_bounds().unwrap()
            ]
        );
        assert_eq!(
            capacities,
            [
                conversion.output_capacity(),
                carry.output_capacity(),
                sine.output_capacity(),
                combined.output_capacity()
            ]
        );
    }
}
#[test]
fn old_profiles_and_unreviewed_operations_refuse_before_execution() {
    let p = program("Angle", "Angle", "math/angle-radians(.)");
    for attempt in [
        PreparedPortableExpressionEvaluator::new(&p),
        PreparedPortableExpressionEvaluator::new_with_exact_quantities(&p),
    ] {
        assert!(matches!(
            attempt,
            Err(PortableExpressionEvaluationRefusal::UnsupportedType(_))
        ));
    }
    let binary = program("Angle", "Angle", ". + .");
    assert!(matches!(
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&binary),
        Err(PortableExpressionEvaluationRefusal::UnsupportedType(_))
    ));
    for (input, body) in [
        ("Scalar", "math/angle-radians(.)"),
        ("Angle", "math/angle-radians(42)"),
        ("Angle", "math/angle-radians(1m)"),
    ] {
        let source =
            format!("plot bridge (\n >> value: {input}\n result: Angle >>\n) = ({body})\n");
        let checked =
            check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
        assert!(
            expand_canonical_plot_for_authoring(&checked, "bridge", &ProfileCatalog::new())
                .is_err()
        );
    }
}
#[test]
fn selected_profile_rejects_scalar_family_and_recovers_after_unsupported_radian_input() {
    let p = program("Angle", "MathematicalValue", "math/sin(.)");
    let mut selected =
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&p).unwrap();
    let mut old = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let bridge = program("Angle", "Angle", "math/angle-radians(.)");
    let mut bridge =
        PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&bridge).unwrap();
    let input = Quantity::parse_plot_literal("90°").unwrap().encode();
    let encoded = bridge.evaluate(&input).unwrap().to_vec();
    let rational = RationalQuantity::new(
        ExactRational::ratio(1, 4, &mut work()).unwrap(),
        Unit::Turn,
        QuantityRole::Linear,
    )
    .unwrap();
    let mut v2 = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(rational, &mut v2).unwrap();
    v2.truncate(n);
    let scalar = Quantity::from_decimal(1, 0, Unit::One).unwrap().encode();
    let radians = Quantity::parse_plot_literal("1rad").unwrap().encode();
    let (_, storage) = allocation_probe::observe(|| {
        assert!(old.evaluate(&encoded).is_err());
        assert!(old.evaluate(&v2).is_err());
        for bytes in [input.as_slice(), v2.as_slice()] {
            let result = selected.evaluate(bytes).unwrap();
            assert_eq!(
                MathematicalValueView::decode(result, &mut work())
                    .unwrap()
                    .as_rational(&mut work())
                    .unwrap(),
                Some(ExactRational::ONE)
            );
        }

        assert!(selected.evaluate(&scalar).is_err());
        assert!(selected.evaluate(&radians).is_err());
        let result = selected.evaluate(&encoded).unwrap();
        assert_eq!(
            MathematicalValueView::decode(result, &mut work())
                .unwrap()
                .as_rational(&mut work())
                .unwrap(),
            Some(ExactRational::ONE)
        );
    });
    assert_eq!((storage.allocations, storage.reallocations), (0, 0));
}

#[test]
fn rational_angle_stage_and_repeated_symbolic_conversion_use_the_declared_profile() {
    let mut exact = PreparedPortableExpressionEvaluator::new_with_exact_quantities(&program(
        "Angle", "Angle", ". / 2",
    ))
    .unwrap();
    let mut bridge = PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&program(
        "Angle",
        "Angle",
        "math/angle-radians(math/angle-radians(.))",
    ))
    .unwrap();
    let mut sine = PreparedPortableExpressionEvaluator::new_with_mathematical_angles(&program(
        "Angle",
        "MathematicalValue",
        "math/sin(.)",
    ))
    .unwrap();
    let input = Quantity::parse_plot_literal("180°").unwrap().encode();
    let bounds = bridge.mathematical_angle_profile_bounds().unwrap();
    let (_, storage) = allocation_probe::observe(|| {
        for _ in 0..8 {
            let rational = exact.evaluate(&input).unwrap();
            assert_eq!(rational[0], 2);
            let converted = bridge.evaluate(rational).unwrap();
            let angle = MathematicalQuantityView::decode(converted, &mut work()).unwrap();
            assert_eq!(angle.unit(), Unit::Radian);
            assert_eq!(
                angle
                    .coordinate()
                    .rational_pi_coefficient(&mut work())
                    .unwrap(),
                Some(ExactRational::ratio(1, 2, &mut work()).unwrap())
            );
            let result = sine.evaluate(converted).unwrap();
            assert_eq!(
                MathematicalValueView::decode(result, &mut work())
                    .unwrap()
                    .as_rational(&mut work())
                    .unwrap(),
                Some(ExactRational::ONE)
            );
        }
    });
    assert_eq!((storage.allocations, storage.reallocations), (0, 0));
    assert_eq!(bridge.mathematical_angle_profile_bounds().unwrap(), bounds);
}
