//! Coordinate policy and caller-owned storage, below Source/Back admission.
use super::*;
use conduit_core::{
    QuantityValueProfile as Profile, QuantityValueView, QUANTITY_VALUE_MAX_ENCODED_BYTES,
};

fn run(
    op: O,
    kind: &str,
    left: &[u8],
    scalar: Scalar,
    profile: Profile,
    output: &mut [u8],
) -> Result<usize, R> {
    evaluate_into(
        op,
        kind,
        SCALAR_INFO_ID,
        left,
        &scalar.encode(),
        Output {
            profile,
            bytes: output,
        },
    )
}

#[test]
fn exact_third_crosses_independent_buffers_without_decimal_projection() {
    let one = Quantity::parse_plot_literal("1m").unwrap();
    let three = Scalar::from_raw_microunits(3_000_000);
    let mut first = [0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let first_len = run(
        O::Divide,
        conduit_core::DISTANCE_INFO_ID,
        &one.encode(),
        three,
        Profile::Exact,
        &mut first,
    )
    .unwrap();
    assert_eq!(first[0], 2);
    let mut work = ExactNumericWork::new(conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let third = QuantityValueView::decode(&first[..first_len], &mut work).unwrap();
    assert_eq!(third.unit(), one.unit());
    let expected = ExactRational::from_i128(1)
        .checked_divide(ExactRational::from_i128(3), &mut work)
        .unwrap();
    assert_eq!(third.rational(&mut work).unwrap().coordinate(), expected);
    let mut second = [0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let second_len = run(
        O::Multiply,
        conduit_core::DISTANCE_INFO_ID,
        &first[..first_len],
        three,
        Profile::Exact,
        &mut second,
    )
    .unwrap();
    let restored = QuantityValueView::decode(&second[..second_len], &mut work).unwrap();
    let original_bytes = one.encode();
    let original = QuantityValueView::decode(&original_bytes, &mut work).unwrap();
    assert_eq!(
        restored.compare(original, &mut work).unwrap(),
        core::cmp::Ordering::Equal
    );
    // A small rational can fit inside the decimal profile's byte ceiling.
    assert!(first_len < QUANTITY_ENCODED_LEN);
    assert!(run(
        O::Multiply,
        conduit_core::DISTANCE_INFO_ID,
        &first[..first_len],
        three,
        Profile::Decimal,
        &mut second
    )
    .is_err());
    assert!(matches!(
        run(
            O::Divide,
            conduit_core::DISTANCE_INFO_ID,
            &one.encode(),
            three,
            Profile::Decimal,
            &mut second
        ),
        Err(R::QuantityProjection(
            conduit_core::ExactNumericRefusal::InexactProjection
        ))
    ));
}

#[test]
fn wide_exact_coordinate_preserves_unit_role_and_finite_extent() {
    let mut work = ExactNumericWork::new(4 * conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK);
    let integer = alloc::format!("1{}1", "0".repeat(500));
    let coordinate = ExactRational::parse_decimal(&integer, &mut work).unwrap();
    let native = Quantity::parse_plot_literal("1Qm³").unwrap();
    let value = RationalQuantity::new(coordinate, native.unit(), QuantityRole::Linear).unwrap();
    let mut input = [0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = conduit_core::encode_rational_quantity_value(value, &mut input).unwrap();
    let mut output = [0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    let n = run(
        O::Multiply,
        conduit_core::QUANTITY_INFO_ID,
        &input[..n],
        Scalar::from_raw_microunits(1_000_000),
        Profile::Exact,
        &mut output,
    )
    .unwrap();
    let result = QuantityValueView::decode(&output[..n], &mut work).unwrap();
    assert_eq!(result.unit(), native.unit());
    assert_eq!(result.role(), QuantityRole::Linear);
    assert_eq!(result.rational(&mut work).unwrap().coordinate(), coordinate);
}

#[test]
fn exact_policy_does_not_weaken_physical_typing_or_output_capacity() {
    let one = Quantity::parse_plot_literal("1m").unwrap().encode();
    let factor = Scalar::from_raw_microunits(3_000_000);
    let mut output = [0; QUANTITY_VALUE_MAX_ENCODED_BYTES];
    assert!(run(
        O::Divide,
        conduit_core::FREQUENCY_INFO_ID,
        &one,
        factor,
        Profile::Exact,
        &mut output
    )
    .is_err());
    let point = Quantity::parse_plot_literal("21°C").unwrap().encode();
    assert!(matches!(
        run(
            O::Multiply,
            conduit_core::QUANTITY_INFO_ID,
            &point,
            factor,
            Profile::Exact,
            &mut output
        ),
        Err(R::QuantityProduct(
            conduit_core::RationalQuantityRefusal::InvalidRole
        ))
    ));
    for capacity in [0, 1, 20, 767] {
        assert!(run(
            O::Divide,
            conduit_core::DISTANCE_INFO_ID,
            &one,
            factor,
            Profile::Exact,
            &mut output[..capacity]
        )
        .is_err());
    }
}
