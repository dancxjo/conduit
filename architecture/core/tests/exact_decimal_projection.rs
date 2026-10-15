use conduit_core::{ExactNumericRefusal as R, ExactNumericWork, ExactRational};
fn work() -> ExactNumericWork {
    ExactNumericWork::new(60_000_000)
}
#[test]
fn decimal_projection_retains_scientific_parts_and_signed_extremes() {
    let mut work = work();
    for (coefficient, exponent) in [(1, 128), (-1, -128), (125, -3), (i128::MIN, 0)] {
        let value = ExactRational::from_decimal_parts(coefficient, exponent, &mut work).unwrap();
        assert_eq!(
            value.checked_decimal_parts(&mut work).unwrap(),
            (coefficient, exponent)
        );
    }
}
#[test]
fn decimal_projection_refuses_nonterminating_values_and_exhausted_work() {
    let mut work = work();
    let third = ExactRational::ratio(1, 3, &mut work).unwrap();
    assert_eq!(
        third.checked_decimal_parts(&mut work),
        Err(R::InexactProjection)
    );
    assert_eq!(
        ExactRational::ZERO.checked_decimal_parts(&mut ExactNumericWork::new(0)),
        Err(R::WorkLimit)
    );
    assert_eq!(
        ExactRational::ONE.checked_decimal_parts(&mut ExactNumericWork::new(0)),
        Err(R::WorkLimit)
    );
}
