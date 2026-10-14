use conduit_core::{ExactNumericRefusal, ExactNumericWork, ExactRational};

#[test]
fn unsigned_fraction_projection_retains_exact_parts_and_refuses_loss() {
    let mut work = ExactNumericWork::new(1_000_000);
    assert_eq!(
        ExactRational::ratio(1, 3, &mut work)
            .unwrap()
            .checked_unsigned_ratio(),
        Ok((1, 3))
    );
    assert_eq!(
        ExactRational::from_u128(0).checked_unsigned_ratio(),
        Ok((0, 1))
    );
    assert_eq!(
        ExactRational::from_i128(-1).checked_unsigned_ratio(),
        Err(ExactNumericRefusal::OutOfRange)
    );
    let wide = ExactRational::from_u128(u128::MAX)
        .checked_add(ExactRational::from_u128(1), &mut work)
        .unwrap();
    assert_eq!(
        wide.checked_unsigned_ratio(),
        Err(ExactNumericRefusal::OutOfRange)
    );
    let tiny = ExactRational::from_decimal_parts(1, -40, &mut work).unwrap();
    assert_eq!(
        tiny.checked_unsigned_ratio(),
        Err(ExactNumericRefusal::OutOfRange)
    );
}
