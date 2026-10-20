//! Inverse circle-reference conversion must cancel a supported rational Pi law.
use conduit_core::*;
#[path = "common/numerical_allocation.rs"]
mod allocation_probe;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(200_000_000)
}
#[test]
fn pi_radians_convert_exactly_to_half_a_turn_or_180_degrees_without_growth() {
    let pi = MathematicalValue::parse_literal("pi", &mut work()).unwrap();
    let mut coordinate = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = pi.encode(&mut coordinate).unwrap();
    let pi = MathematicalValueView::decode(&coordinate[..n], &mut work()).unwrap();
    let mut source = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let n = encode_mathematical_quantity_value(
        &pi,
        Unit::Radian,
        QuantityRole::Linear,
        &mut source,
        &mut work(),
    )
    .unwrap();
    source.truncate(n);
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let retained = converter.retained_payload_bytes();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let cases = [
        (Unit::Turn, ExactRational::ratio(1, 2, &mut work()).unwrap()),
        (
            Quantity::parse_plot_literal("1°").unwrap().unit(),
            ExactRational::from_i128(180),
        ),
    ];
    let (_, allocations) = allocation_probe::observe(|| {
        for (unit, expected) in cases {
            let receipt = converter
                .execute(&source, unit, &mut output, &mut work())
                .unwrap();
            assert_eq!(receipt.reference_law(), Some(TURN_RADIAN_REFERENCE_LAW));
            assert_eq!(receipt.source().encoded(), source);
            let target =
                MathematicalQuantityView::decode(&output[..receipt.encoded_bytes()], &mut work())
                    .unwrap();
            assert_eq!(target.unit(), unit);
            assert_eq!(target.role(), QuantityRole::Linear);
            assert_eq!(
                target.coordinate().as_rational(&mut work()).unwrap(),
                Some(expected)
            );
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(converter.retained_payload_bytes(), retained);
}

#[test]
fn one_radian_retains_an_exact_reciprocal_pi_expression_until_certified_enclosure() {
    let source = Quantity::parse_plot_literal("1rad").unwrap().encode();
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let ratio = |n, d| ExactRational::ratio(n, d, &mut work()).unwrap();
    let requirements = MathematicalEnclosureRequirements {
        maximum_width: ratio(1, 100_000_000_000_000),
        maximum_leaf_width: ratio(1, 100_000_000_000_000_000),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let mut numeric =
        PreparedMathematicalQuantityEnclosure::new(requirements, 200_000_000).unwrap();
    let (_, allocations) = allocation_probe::observe(|| {
        let receipt = converter
            .execute(&source, Unit::Turn, &mut output, &mut work())
            .unwrap();
        assert_eq!(receipt.reference_law(), Some(TURN_RADIAN_REFERENCE_LAW));
        let bytes = &output[..receipt.encoded_bytes()];
        let target = MathematicalQuantityView::decode(bytes, &mut work()).unwrap();
        assert_eq!(target.unit(), Unit::Turn);
        assert!(target
            .coordinate()
            .as_rational(&mut work())
            .unwrap()
            .is_none());
        let enclosed = numeric.execute(bytes, &mut work()).unwrap();
        assert!(enclosed
            .lower()
            .coordinate()
            .checked_compare(ratio(15915494309, 100_000_000_000), &mut work())
            .unwrap()
            .is_gt());
        assert!(enclosed
            .upper()
            .coordinate()
            .checked_compare(ratio(15915494310, 100_000_000_000), &mut work())
            .unwrap()
            .is_lt());
    });
    assert_eq!(allocations, 0);
}

#[test]
fn a_coarse_pi_enclosure_crossing_zero_refuses_reciprocal_without_inventing_a_bound() {
    let source = Quantity::parse_plot_literal("1rad").unwrap().encode();
    let mut converter = PreparedQuantityCoordinateConversion::new(500000, 200_000_000).unwrap();
    let mut output = vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES];
    let requirements = MathematicalEnclosureRequirements {
        maximum_width: ExactRational::from_i128(100),
        maximum_leaf_width: ExactRational::from_i128(100),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    };
    let mut numeric =
        PreparedMathematicalQuantityEnclosure::new(requirements, 200_000_000).unwrap();
    let (_, allocations) = allocation_probe::observe(|| {
        let receipt = converter
            .execute(&source, Unit::Turn, &mut output, &mut work())
            .unwrap();
        let mut meter = work();
        assert!(matches!(
            numeric.execute(&output[..receipt.encoded_bytes()], &mut meter),
            Err(MathematicalQuantityEnclosureRefusal::Enclosure(
                MathematicalEnclosureRefusal::DivisorIntervalContainsZero
            ))
        ));
        assert!(meter.spent() > 0);
    });
    assert_eq!(allocations, 0);
}
