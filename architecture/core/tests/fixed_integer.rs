use core::cmp::Ordering;

use conduit_core::{FixedInteger, FixedIntegerRefusal, PrimitiveInfoKind};

#[test]
fn exact_width_values_round_trip_little_endian() {
    let value = FixedInteger::from_unsigned(PrimitiveInfoKind::U16, 0xabcd).unwrap();
    let (encoded, length) = value.encode();
    assert_eq!(length, 2);
    assert_eq!(&encoded[..length], &[0xcd, 0xab]);
    assert_eq!(
        FixedInteger::decode(PrimitiveInfoKind::U16, &encoded[..length]).unwrap(),
        value
    );

    let negative = FixedInteger::from_signed(PrimitiveInfoKind::I8, -2).unwrap();
    let (encoded, length) = negative.encode();
    assert_eq!(&encoded[..length], &[0xfe]);
    assert_eq!(negative.signed(), Ok(-2));
}

#[test]
fn arithmetic_refuses_overflow_zero_division_and_cross_type_promotion() {
    let maximum = FixedInteger::from_unsigned(PrimitiveInfoKind::U8, 255).unwrap();
    let one = FixedInteger::from_unsigned(PrimitiveInfoKind::U8, 1).unwrap();
    let zero = FixedInteger::from_unsigned(PrimitiveInfoKind::U8, 0).unwrap();
    assert_eq!(maximum.checked_add(one), Err(FixedIntegerRefusal::Overflow));
    assert_eq!(
        one.checked_div(zero),
        Err(FixedIntegerRefusal::DivisionByZero)
    );

    let wider = FixedInteger::from_unsigned(PrimitiveInfoKind::U16, 1).unwrap();
    assert_eq!(one.checked_add(wider), Err(FixedIntegerRefusal::Signedness));
    assert_eq!(
        FixedInteger::from_unsigned(PrimitiveInfoKind::U8, 256),
        Err(FixedIntegerRefusal::OutOfRange)
    );
}

#[test]
fn shifts_have_one_portable_range_law_and_signed_right_shift_is_arithmetic() {
    let high = FixedInteger::from_unsigned(PrimitiveInfoKind::U8, 0x80).unwrap();
    assert_eq!(
        high.checked_shift_left(1),
        Err(FixedIntegerRefusal::Overflow)
    );
    assert_eq!(
        high.checked_shift_right(8),
        Err(FixedIntegerRefusal::ShiftOutOfRange { width: 8, count: 8 })
    );

    let negative = FixedInteger::from_signed(PrimitiveInfoKind::I8, -4).unwrap();
    assert_eq!(negative.checked_shift_right(1).unwrap().signed(), Ok(-2));
}

#[test]
fn comparison_and_bitwise_operations_follow_checked_signedness() {
    let negative = FixedInteger::from_signed(PrimitiveInfoKind::I8, -1).unwrap();
    let positive = FixedInteger::from_signed(PrimitiveInfoKind::I8, 1).unwrap();
    assert_eq!(negative.compare(positive), Ok(Ordering::Less));

    let mask = FixedInteger::from_unsigned(PrimitiveInfoKind::U8, 0x0f).unwrap();
    let value = FixedInteger::from_unsigned(PrimitiveInfoKind::U8, 0xa5).unwrap();
    assert_eq!(value.bit_and(mask).unwrap().unsigned(), Ok(5));
}
