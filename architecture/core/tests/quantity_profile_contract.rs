//! Selected codec admission composes the existing physical range contract.
use conduit_core::*;
fn wire(numerator: i128, denominator: i128) -> Vec<u8> {
    let mut work = ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK);
    let native = Quantity::parse_plot_literal("1m").unwrap();
    let coordinate = ExactRational::from_i128(numerator)
        .checked_divide(ExactRational::from_i128(denominator), &mut work)
        .unwrap();
    let value = RationalQuantity::new(coordinate, native.unit(), QuantityRole::Linear).unwrap();
    let mut bytes = vec![0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = encode_rational_quantity_value(value, &mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
fn contract() -> CheckedValueContract {
    CheckedValueContract::new(
        kind_id(DISTANCE_INFO_ID),
        QUANTITY_VALUE_MAX_ENCODED_BYTES as u32,
        vec![ValueConstraint::QuantityRange {
            minimum: Some(Box::new(Quantity::parse_plot_literal("25cm").unwrap())),
            maximum: Some(Box::new(Quantity::parse_plot_literal("50cm").unwrap())),
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Exclusive,
        }],
    )
    .unwrap()
}
#[test]
fn explicit_profile_checks_physical_ranges_across_unit_coordinates() {
    let contract = contract();
    assert!(contract
        .validate_with_quantity_profile(&wire(1, 3), QuantityValueProfile::Exact)
        .is_ok());
    assert!(contract
        .validate_with_quantity_profile(&wire(1, 4), QuantityValueProfile::Exact)
        .is_ok());
    for (n, d) in [(1, 6), (1, 2)] {
        assert_eq!(
            contract.validate_with_quantity_profile(&wire(n, d), QuantityValueProfile::Exact),
            Err(QuantityProfileContractRefusal::Contract(
                ValueConstraintRefusal::QuantityRange
            ))
        );
    }
    assert!(contract
        .validate(&Quantity::parse_plot_literal("30cm").unwrap().encode())
        .is_ok());
}
#[test]
fn native_validation_is_preserved_and_explicit_codec_refusal_is_distinct() {
    let bytes = wire(1, 3);
    assert!(matches!(
        contract().validate(&bytes),
        Err(ValueConstraintRefusal::MalformedPrimitive(_))
    ));
    assert_eq!(
        contract().validate_with_quantity_profile(&bytes, QuantityValueProfile::Decimal),
        Err(QuantityProfileContractRefusal::Quantity(
            QuantityValueContractRefusal::UnsupportedRepresentation {
                profile: QuantityValueProfile::Decimal,
                version: 2
            }
        ))
    );
    let wrong = CheckedValueContract::new(
        kind_id(FREQUENCY_INFO_ID),
        QUANTITY_VALUE_MAX_ENCODED_BYTES as u32,
        vec![],
    )
    .unwrap();
    assert!(matches!(
        wrong.validate_with_quantity_profile(&bytes, QuantityValueProfile::Exact),
        Err(QuantityProfileContractRefusal::Quantity(
            QuantityValueContractRefusal::WrongFamily
        ))
    ));
    let bounded = CheckedValueContract::new(kind_id(DISTANCE_INFO_ID), 20, vec![]).unwrap();
    assert!(matches!(
        bounded.validate_with_quantity_profile(&bytes, QuantityValueProfile::Exact),
        Err(QuantityProfileContractRefusal::Contract(
            ValueConstraintRefusal::Oversize { .. }
        ))
    ));
    let scalar =
        CheckedValueContract::new(kind_id(SCALAR_INFO_ID), SCALAR_ENCODED_LEN as u32, vec![])
            .unwrap();
    assert!(scalar
        .validate_with_quantity_profile(
            &Scalar::from_raw_microunits(3).encode(),
            QuantityValueProfile::Exact
        )
        .is_ok());
}
