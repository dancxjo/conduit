//! Compatible difference roles retain exact scale and never acquire point offsets.
use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn ratio(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn inputs<'a>(y: &'a str, x: &'a str, unit: &'a str) -> Atan2QuantityInputs<'a> {
    Atan2QuantityInputs {
        y: Quantity::parse_plot_literal(y).unwrap(),
        original_y: y,
        x: Quantity::parse_plot_literal(x).unwrap(),
        original_x: x,
        common_unit: Unit::resolve(unit).unwrap(),
        common_unit_source: unit,
    }
}
fn denotation<'a>(
    input: Atan2QuantityInputs<'a>,
) -> Result<QuantityAtan2Denotation<'a>, Atan2Refusal> {
    QuantityAtan2Denotation::from_checked(input, Atan2Branch::PrincipalSignedHalfTurn, &mut work())
}
#[test]
fn difference_units_use_scale_without_celsius_or_fahrenheit_offsets() {
    let input = inputs("TemperatureDelta(9, °F)", "TemperatureDelta(5, K)", "°C");
    let exact = denotation(input).unwrap();
    assert_eq!(exact.coordinate().as_rational(), Some(ratio(1, 8)));
    for receipt in [exact.y_conversion(), exact.x_conversion()] {
        let value = receipt.result().unwrap();
        assert_eq!(value.coordinate(), ExactRational::from_i128(5));
        assert_eq!(value.role(), QuantityRole::Delta);
        assert_eq!(value.unit(), Unit::resolve("°C").unwrap());
    }
    assert_eq!(exact.y_conversion().original(), input.original_y);
    assert_eq!(exact.x_conversion().original(), input.original_x);
    let mut out = [0xA5; INVERSE_ANGLE_MAX_ENCODED_BYTES];
    let n = encode_quantity_atan2_denotation(input.y, input.x, &mut out, &mut work()).unwrap();
    let angle = MathematicalAngle::decode(&out[..n], &mut work()).unwrap();
    assert_eq!(angle.coordinate(), exact.coordinate());
    assert!(out[n..].iter().all(|byte| *byte == 0xA5));
}
#[test]
fn differences_preserve_exact_axes_and_all_signed_quadrants() {
    for (y, x, n, d) in [
        (0, 5, 0, 1),
        (9, 0, 1, 4),
        (-9, 0, -1, 4),
        (0, -5, 1, 2),
        (9, 5, 1, 8),
        (9, -5, 3, 8),
        (-9, -5, -3, 8),
        (-9, 5, -1, 8),
    ] {
        let y = format!("TemperatureDelta({y}, °F)");
        let x = format!("TemperatureDelta({x}, K)");
        let input = inputs(&y, &x, "K");
        let exact = denotation(input).unwrap();
        assert_eq!(exact.coordinate().as_rational(), Some(ratio(n, d)));
        let mut out = [0xA5; INVERSE_ANGLE_MAX_ENCODED_BYTES];
        let len =
            encode_quantity_atan2_denotation(input.y, input.x, &mut out, &mut work()).unwrap();
        assert_eq!(
            MathematicalAngle::decode(&out[..len], &mut work())
                .unwrap()
                .coordinate(),
            exact.coordinate()
        );
        assert!(out[len..].iter().all(|byte| *byte == 0xA5));
    }
}
#[test]
fn difference_projection_matches_the_same_exact_linear_ratio_and_turn_bound() {
    let req = Atan2Requirements {
        branch: Atan2Branch::PrincipalSignedHalfTurn,
        numeric: ArctangentTurnsRequirements {
            maximum_width_turns: ratio(1, 1_000_000_000),
            maximum_series_width: ratio(1, 100_000_000_000),
            maximum_terms: 32,
        },
    };
    let difference = PreparedQuantityAtan2::from_checked(
        inputs("TemperatureDelta(3, °C)", "TemperatureDelta(4, K)", "K"),
        req,
        &mut work(),
    )
    .unwrap();
    let linear =
        PreparedQuantityAtan2::from_checked(inputs("3m", "4m", "m"), req, &mut work()).unwrap();
    assert_eq!(
        difference.denotation().coordinate(),
        linear.denotation().coordinate()
    );
    assert_eq!(difference.lower(), linear.lower());
    assert_eq!(difference.upper(), linear.upper());
    assert_eq!(difference.lower().unit(), Unit::Turn);
    assert_eq!(difference.lower().role(), QuantityRole::Linear);
    assert!(
        difference
            .width_turns()
            .checked_compare(req.numeric.maximum_width_turns, &mut work())
            .unwrap()
            != core::cmp::Ordering::Greater
    );
}
#[test]
fn points_and_mixed_roles_still_refuse_before_touching_writer_output() {
    for (y, x) in [
        ("21°C", "22°C"),
        ("TemperatureDelta(1, K)", "21°C"),
        ("21°C", "TemperatureDelta(1, K)"),
    ] {
        let input = inputs(y, x, "K");
        assert_eq!(denotation(input), Err(Atan2Refusal::UnsupportedRole));
        let mut out = [0xA5; INVERSE_ANGLE_MAX_ENCODED_BYTES];
        assert_eq!(
            encode_quantity_atan2_denotation(input.y, input.x, &mut out, &mut work()),
            Err(InverseAngleEncodingRefusal::Quantity(
                Atan2Refusal::UnsupportedRole
            ))
        );
        assert!(out.iter().all(|byte| *byte == 0xA5));
    }
}
#[test]
fn difference_zero_pair_and_forged_source_keep_distinct_refusals() {
    let zero = inputs("TemperatureDelta(0, °F)", "TemperatureDelta(0, K)", "K");
    assert_eq!(denotation(zero), Err(Atan2Refusal::UndefinedZeroPair));
    let mut out = [0xA5; INVERSE_ANGLE_MAX_ENCODED_BYTES];
    assert_eq!(
        encode_quantity_atan2_denotation(zero.y, zero.x, &mut out, &mut work()),
        Err(InverseAngleEncodingRefusal::Quantity(
            Atan2Refusal::UndefinedZeroPair
        ))
    );
    assert!(out.iter().all(|byte| *byte == 0xA5));
    let mut forged = inputs("TemperatureDelta(9, °F)", "TemperatureDelta(5, K)", "K");
    forged.original_y = "TemperatureDelta(8, °F)";
    assert!(matches!(
        denotation(forged),
        Err(Atan2Refusal::SourceEvidence(_))
    ));
}
