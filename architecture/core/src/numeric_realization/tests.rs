use super::*;
use crate::{ExactRational, MathValue, NumericRefusal};
fn project(value: MathValue, rounding: Binary64Rounding) -> Binary64Result {
    Binary64Value::project(&value, rounding, &mut NumericRealizationWork::new(4096)).unwrap()
}
fn operation(
    op: Binary64Operation,
    a: u64,
    b: Option<u64>,
    rounding: Binary64Rounding,
) -> Binary64Result {
    op.apply(
        Binary64Value::from_bits(a),
        b.map(Binary64Value::from_bits),
        rounding,
        &mut NumericRealizationWork::new(4096),
    )
    .unwrap()
}
#[test]
fn each_literal_and_ordered_operation_rounds_instead_of_only_the_final_answer() {
    let mode = Binary64Rounding::NearestEven;
    let tenth = project(MathValue::parse_decimal("0.1").unwrap(), mode);
    let fifth = project(MathValue::parse_decimal("0.2").unwrap(), mode);
    assert_eq!(tenth.value.bits(), 0x3fb9_9999_9999_999a);
    assert_eq!(fifth.value.bits(), 0x3fc9_9999_9999_999a);
    let sum = operation(
        Binary64Operation::Add,
        tenth.value.bits(),
        Some(fifth.value.bits()),
        mode,
    );
    assert_eq!(sum.value.bits(), 0x3fd3_3333_3333_3334);
    assert_ne!(
        sum.value,
        project(MathValue::parse_decimal("0.3").unwrap(), mode).value
    );
    let large = project(
        MathValue::from_integer_decimal("10000000000000000").unwrap(),
        mode,
    )
    .value;
    let intermediate = operation(
        Binary64Operation::Add,
        large.bits(),
        Some(0x3ff0_0000_0000_0000),
        mode,
    );
    assert!(intermediate.flags.inexact);
    let cancelled = operation(
        Binary64Operation::Subtract,
        intermediate.value.bits(),
        Some(large.bits()),
        mode,
    );
    assert_eq!(cancelled.value.bits(), 0);
    assert_eq!(
        MathValue::from_integer_decimal("10000000000000000")
            .unwrap()
            .checked_add(&MathValue::ONE)
            .unwrap()
            .checked_sub(&MathValue::from_integer_decimal("10000000000000000").unwrap())
            .unwrap(),
        MathValue::ONE
    );
}
#[test]
fn midpoint_ties_and_directed_modes_have_exact_bit_contracts() {
    let midpoint = MathValue::ONE
        .checked_add(&MathValue::from_rational(
            ExactRational::dyadic(1, -53).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        project(midpoint.clone(), Binary64Rounding::NearestEven)
            .value
            .bits(),
        0x3ff0_0000_0000_0000
    );
    assert_eq!(
        project(midpoint.clone(), Binary64Rounding::NearestAway)
            .value
            .bits(),
        0x3ff0_0000_0000_0001
    );
    assert_eq!(
        project(midpoint.clone(), Binary64Rounding::TowardPositive)
            .value
            .bits(),
        0x3ff0_0000_0000_0001
    );
    assert_eq!(
        project(midpoint.clone(), Binary64Rounding::TowardNegative)
            .value
            .bits(),
        0x3ff0_0000_0000_0000
    );
    let negative = midpoint.checked_neg().unwrap();
    assert_eq!(
        project(negative.clone(), Binary64Rounding::TowardPositive)
            .value
            .bits(),
        0xbff0_0000_0000_0000
    );
    assert_eq!(
        project(negative, Binary64Rounding::TowardNegative)
            .value
            .bits(),
        0xbff0_0000_0000_0001
    );
}
#[test]
fn subnormal_underflow_and_overflow_are_observed_after_rounding() {
    let tiny = MathValue::from_rational(ExactRational::dyadic(1, -1075).unwrap());
    let rounded = project(tiny, Binary64Rounding::NearestEven);
    assert_eq!(rounded.value.bits(), 0);
    assert!(rounded.flags.inexact && rounded.flags.underflow);
    let exact = project(
        MathValue::from_rational(ExactRational::dyadic(1, -1074).unwrap()),
        Binary64Rounding::NearestEven,
    );
    assert_eq!(exact.value.bits(), 1);
    assert_eq!(exact.flags, Binary64Flags::default());
    let huge = MathValue::from_rational(ExactRational::dyadic(1, 1024).unwrap());
    let infinity = project(huge.clone(), Binary64Rounding::NearestEven);
    assert_eq!(infinity.value.bits(), 0x7ff0_0000_0000_0000);
    assert!(infinity.flags.overflow && infinity.flags.inexact);
    let finite = project(huge, Binary64Rounding::TowardZero);
    assert_eq!(finite.value.bits(), 0x7fef_ffff_ffff_ffff);
    assert!(finite.flags.overflow && finite.flags.inexact);
}
#[test]
fn ieee_operational_special_values_preserve_custody_and_flags() {
    let mode = Binary64Rounding::NearestEven;
    let signaling = operation(
        Binary64Operation::Add,
        0x7ff0_0000_0000_0001,
        Some(0x3ff0_0000_0000_0000),
        mode,
    );
    assert_eq!(signaling.value.bits(), 0x7ff8_0000_0000_0001);
    assert!(signaling.flags.invalid);
    let invalid = operation(
        Binary64Operation::Add,
        0x7ff0_0000_0000_0000,
        Some(0xfff0_0000_0000_0000),
        mode,
    );
    assert_eq!(invalid.value.bits(), 0x7ff8_0000_0000_0000);
    assert!(invalid.flags.invalid);
    let division = operation(
        Binary64Operation::Divide,
        0x3ff0_0000_0000_0000,
        Some(0),
        mode,
    );
    assert_eq!(division.value.bits(), 0x7ff0_0000_0000_0000);
    assert!(division.flags.division_by_zero);
    assert_eq!(
        operation(
            Binary64Operation::Multiply,
            1 << 63,
            Some(0x3ff0_0000_0000_0000),
            mode
        )
        .value
        .bits(),
        1 << 63
    );
    assert_eq!(
        operation(Binary64Operation::Add, 1 << 63, Some(0), mode)
            .value
            .bits(),
        0
    );
    assert_eq!(
        operation(
            Binary64Operation::Add,
            1 << 63,
            Some(0),
            Binary64Rounding::TowardNegative
        )
        .value
        .bits(),
        1 << 63
    );
    assert!(
        operation(Binary64Operation::Divide, 0, Some(0), mode)
            .flags
            .invalid
    );
}
#[test]
fn square_root_is_rounded_by_exact_squared_midpoint_comparisons() {
    let rounded = operation(
        Binary64Operation::SquareRoot,
        0x4000_0000_0000_0000,
        None,
        Binary64Rounding::NearestEven,
    );
    assert_eq!(rounded.value.bits(), 0x3ff6_a09e_667f_3bcd);
    assert!(rounded.flags.inexact);
    assert_eq!(
        operation(
            Binary64Operation::SquareRoot,
            1 << 63,
            None,
            Binary64Rounding::NearestEven
        )
        .value
        .bits(),
        1 << 63
    );
    assert!(
        operation(
            Binary64Operation::SquareRoot,
            0xbff0_0000_0000_0000,
            None,
            Binary64Rounding::NearestEven
        )
        .flags
        .invalid
    );
}
#[test]
fn mandatory_work_is_admitted_before_the_next_primitive() {
    let mut work = NumericRealizationWork::new(0);
    assert_eq!(
        Binary64Value::project(&MathValue::ONE, Binary64Rounding::NearestEven, &mut work),
        Err(NumericRefusal::WorkBudget)
    );
    assert_eq!(work.used(), 0);
}
