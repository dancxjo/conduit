use conduit_core::*;
fn work() -> ExactNumericWork {
    ExactNumericWork::new(100_000_000)
}
fn q(n: i128, d: i128) -> ExactRational {
    ExactRational::ratio(n, d, &mut work()).unwrap()
}
fn requirements() -> Atan2Requirements {
    Atan2Requirements {
        branch: Atan2Branch::PrincipalSignedHalfTurn,
        numeric: ArctangentTurnsRequirements {
            maximum_width_turns: q(1, 1_000_000_000),
            maximum_series_width: q(1, 100_000_000_000),
            maximum_terms: 32,
        },
    }
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
#[test]
fn different_length_units_keep_source_and_match_equal_unit_result() {
    let p = PreparedQuantityAtan2::from_checked(
        inputs("3m", "400cm", "m"),
        requirements(),
        &mut work(),
    )
    .unwrap();
    let same =
        PreparedQuantityAtan2::from_checked(inputs("3m", "4m", "m"), requirements(), &mut work())
            .unwrap();
    assert_eq!(p.lower(), same.lower());
    assert_eq!(p.upper(), same.upper());
    assert_eq!(p.lower().dimension(), QuantityDimension::Angle);
    assert_eq!(p.lower().unit(), Unit::Turn);
    assert_eq!(p.x_conversion().original(), "400cm");
    assert_eq!(
        p.x_conversion().result().unwrap().coordinate(),
        ExactRational::from_i128(4)
    );
    assert_eq!(p.y_conversion().original(), "3m");
}
#[test]
fn axes_and_quadrants_use_explicit_signed_half_turn_branch() {
    for (y, x, turn) in [
        ("0m", "1m", q(0, 1)),
        ("1m", "0m", q(1, 4)),
        ("-1m", "0m", q(-1, 4)),
        ("0m", "-1m", q(1, 2)),
        ("1m", "1m", q(1, 8)),
        ("1m", "-1m", q(3, 8)),
        ("-1m", "-1m", q(-3, 8)),
        ("-1m", "1m", q(-1, 8)),
    ] {
        let p = PreparedQuantityAtan2::from_checked(inputs(y, x, "m"), requirements(), &mut work())
            .unwrap();
        assert_eq!(p.lower().coordinate(), turn);
        assert_eq!(p.upper().coordinate(), turn);
        assert_eq!(p.width_turns(), ExactRational::ZERO);
    }
}
#[test]
fn dimensions_zero_pair_and_forged_source_refuse_distinctly() {
    assert_eq!(
        PreparedQuantityAtan2::from_checked(inputs("3m", "4s", "m"), requirements(), &mut work()),
        Err(Atan2Refusal::MismatchedDimensions)
    );
    assert_eq!(
        PreparedQuantityAtan2::from_checked(inputs("0m", "0cm", "m"), requirements(), &mut work()),
        Err(Atan2Refusal::UndefinedZeroPair)
    );
    let mut forged = inputs("3m", "400cm", "m");
    forged.original_x = "4cm";
    assert!(matches!(
        PreparedQuantityAtan2::from_checked(forged, requirements(), &mut work()),
        Err(Atan2Refusal::SourceEvidence(_))
    ));
    let mut req = requirements();
    req.numeric.maximum_terms = 0;
    assert!(matches!(
        PreparedQuantityAtan2::from_checked(inputs("3m", "400cm", "m"), req, &mut work()),
        Err(Atan2Refusal::Enclosure(
            MathematicalEnclosureRefusal::ConstantTermLimit
        ))
    ));
}

#[test]
fn exact_denotation_precedes_profile_and_survives_common_unit_and_codec() {
    let branch = Atan2Branch::PrincipalSignedHalfTurn;
    let exact =
        QuantityAtan2Denotation::from_checked(inputs("3m", "400cm", "m"), branch, &mut work())
            .unwrap();
    let centimeters =
        QuantityAtan2Denotation::from_checked(inputs("3m", "400cm", "cm"), branch, &mut work())
            .unwrap();
    assert_eq!(exact.coordinate(), centimeters.coordinate());
    assert_eq!(exact.coordinate().as_rational(), None);
    assert_eq!(exact.unit(), Unit::Turn);
    assert_eq!(exact.unit().dimension(), QuantityDimension::Angle);
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = exact.coordinate().encode(&mut bytes).unwrap();
    bytes.truncate(length);
    assert_eq!(
        MathematicalValue::decode(&bytes, &mut work()).unwrap(),
        *exact.coordinate()
    );
    let mut validation = ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
    MathematicalValueView::decode(&bytes, &mut validation).unwrap();
    let projected = PreparedQuantityAtan2::from_checked(
        inputs("3m", "400cm", "m"),
        requirements(),
        &mut work(),
    )
    .unwrap();
    assert_eq!(projected.denotation(), &exact);
    assert_eq!(projected.projection().source(), exact.coordinate());
    let mut impossible = requirements();
    impossible.numeric.maximum_width_turns = ExactRational::ZERO;
    assert!(PreparedQuantityAtan2::from_checked(
        inputs("3m", "400cm", "m"),
        impossible,
        &mut work()
    )
    .is_err());
    assert_eq!(exact.x_conversion().original(), "400cm");
    assert_eq!(
        MathematicalValue::decode(&bytes, &mut work()).unwrap(),
        *exact.coordinate()
    );
}
