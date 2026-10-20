//! A composed consumer cannot reset its caller's numerical allowance.
use conduit_core::*;

#[test]
fn child_allowances_are_clamped_and_success_and_refusal_both_charge_the_caller() {
    let mut reference = ExactNumericWork::new(100_000);
    let expected = ExactRational::parse_decimal("0.1", &mut reference).unwrap();
    let cost = reference.spent();
    assert!(cost > 1);
    let mut parent = ExactNumericWork::new(2 * cost);
    let result = parent.with_limit(u64::MAX, |child| {
        assert_eq!(child.remaining(), 2 * cost);
        ExactRational::parse_decimal("0.1", child)
    });
    assert_eq!(result, Ok(expected));
    assert_eq!((parent.spent(), parent.remaining()), (cost, cost));
    let failed: Result<(), ExactNumericRefusal> = parent.with_limit(cost, |child| {
        ExactRational::parse_decimal("0.1", child)?;
        Err(ExactNumericRefusal::DivisionByZero)
    });
    assert_eq!(failed, Err(ExactNumericRefusal::DivisionByZero));
    assert_eq!((parent.spent(), parent.remaining()), (2 * cost, 0));
    assert_eq!(
        parent.with_limit(u64::MAX, |child| ExactRational::parse_decimal("1", child)),
        Err(ExactNumericRefusal::WorkLimit)
    );
    assert_eq!(parent.spent(), 2 * cost);
}

#[test]
fn a_child_cap_does_not_consume_unspent_parent_work() {
    let mut parent = ExactNumericWork::new(100_000);
    let remaining = parent.remaining();
    let result = parent.with_limit(1, |child| {
        ExactRational::parse_decimal("0.123456789", child)
    });
    assert_eq!(result, Err(ExactNumericRefusal::WorkLimit));
    assert!(parent.spent() <= 1);
    assert_eq!(parent.remaining() + parent.spent(), remaining);
    assert_eq!(
        parent.with_limit(0, |_| Ok::<_, ExactNumericRefusal>(42)),
        Ok(42)
    );
}
