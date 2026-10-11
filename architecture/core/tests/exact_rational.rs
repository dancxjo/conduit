//! Exact denotation, bounded work, hostile codec and independent small oracle.
use conduit_core::{
    ExactNumericRefusal as R, ExactNumericWork as Work, ExactRational as Q,
    EXACT_RATIONAL_MAX_ENCODED_BYTES,
};
use core::cmp::Ordering;
fn work() -> Work {
    Work::new(100_000_000)
}
fn ratio(n: i128, d: i128) -> Q {
    Q::ratio(n, d, &mut work()).unwrap()
}
fn encoded(q: Q) -> Vec<u8> {
    let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let length = q.encode(&mut bytes).unwrap();
    bytes[..length].to_vec()
}
#[test]
fn exact_thirds_and_decimals_do_not_inherit_binary_rounding() {
    let third = ratio(1, 3);
    let result = third
        .checked_add(third, &mut work())
        .unwrap()
        .checked_add(third, &mut work())
        .unwrap();
    assert_eq!(result, Q::ONE);
    let tenth = Q::parse_decimal("0.1", &mut work()).unwrap();
    let fifth = Q::parse_decimal("0.2", &mut work()).unwrap();
    assert_eq!(
        tenth.checked_add(fifth, &mut work()).unwrap(),
        Q::parse_decimal("0.3", &mut work()).unwrap()
    );
    assert_eq!(Q::parse_decimal("-000.000", &mut work()).unwrap(), Q::ZERO);
    assert_eq!(Q::parse_decimal("1.00", &mut work()).unwrap(), Q::ONE);
    assert_eq!(ratio(2, -6), third.negated());
    assert_eq!(ratio(4, 12).semantic_digest(), third.semantic_digest());
    assert_eq!(ratio(1, 3).checked_i128(), Err(R::InexactProjection));
    assert_eq!(Q::from_i128(i128::MIN).checked_i128().unwrap(), i128::MIN);
}
#[test]
fn adequate_extent_preserves_ten_to_thousand_plus_one() {
    let power = format!("1{}", "0".repeat(1000));
    let integer = Q::parse_decimal(&power, &mut work()).unwrap();
    let sum = integer.checked_add(Q::ONE, &mut work()).unwrap();
    let literal = format!("1{}1", "0".repeat(999));
    assert_eq!(sum, Q::parse_decimal(&literal, &mut work()).unwrap());
    assert_eq!(sum.checked_subtract(integer, &mut work()).unwrap(), Q::ONE);
    assert_eq!(sum.checked_i128(), Err(R::OutOfRange));
    let bytes = encoded(sum);
    assert_eq!(Q::decode(&bytes, &mut work()).unwrap(), sum);
    assert_eq!(
        sum.checked_compare(integer, &mut work()).unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        integer.checked_divide(integer, &mut work()).unwrap(),
        Q::ONE
    );
}
#[test]
fn small_arithmetic_and_order_match_an_independent_integer_oracle() {
    for a in -5_i128..=5 {
        for b in 1_i128..=5 {
            for c in -5_i128..=5 {
                for d in 1_i128..=5 {
                    let left = ratio(a, b);
                    let right = ratio(c, d);
                    assert_eq!(
                        left.checked_add(right, &mut work()).unwrap(),
                        ratio(a * d + c * b, b * d)
                    );
                    assert_eq!(
                        left.checked_subtract(right, &mut work()).unwrap(),
                        ratio(a * d - c * b, b * d)
                    );
                    assert_eq!(
                        left.checked_multiply(right, &mut work()).unwrap(),
                        ratio(a * c, b * d)
                    );
                    assert_eq!(
                        left.checked_compare(right, &mut work()).unwrap(),
                        (a * d).cmp(&(c * b))
                    );
                    if c != 0 {
                        assert_eq!(
                            left.checked_divide(right, &mut work()).unwrap(),
                            ratio(a * d, b * c)
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn work_extent_and_domains_refuse_without_changing_operands() {
    let third = ratio(1, 3);
    assert_eq!(
        third.checked_add(Q::ONE, &mut Work::new(0)),
        Err(R::WorkLimit)
    );
    assert_eq!(third, ratio(1, 3));
    assert_eq!(
        Q::ONE.checked_divide(Q::ZERO, &mut work()),
        Err(R::DivisionByZero)
    );
    assert_eq!(Q::ratio(0, 0, &mut work()), Err(R::DivisionByZero));
    assert_eq!(
        Q::parse_decimal(&"9".repeat(1234), &mut work()),
        Err(R::ExtentExceeded)
    );
    for literal in ["", "-", "+1", "1e3", ".1", "1.", "1.2.3", " 1", "NaN"] {
        assert_eq!(
            Q::parse_decimal(literal, &mut work()),
            Err(R::InvalidLiteral)
        );
    }
    let value = Q::parse_decimal(&format!("1{}", "0".repeat(1000)), &mut work()).unwrap();
    assert_eq!(
        value.checked_multiply(value, &mut work()),
        Err(R::ExtentExceeded)
    );
    let mut charged = work();
    assert_eq!(third.checked_add(third, &mut charged).unwrap(), ratio(2, 3));
    assert!(charged.spent() > 0);
}
#[test]
fn decoder_rejects_unreduced_negative_zero_padding_and_trailing_data() {
    let third = encoded(ratio(1, 3));
    assert_eq!(Q::decode(&third, &mut work()).unwrap(), ratio(1, 3));
    for bad in [
        vec![1, 0, 1, 0, 1, 0, 2, 6],
        vec![1, 1, 0, 0, 1, 0, 1],
        vec![1, 0, 2, 0, 1, 0, 1, 0, 3],
        vec![1, 0, 1, 0, 0, 0, 1],
        vec![1, 2, 1, 0, 1, 0, 1, 3],
    ] {
        assert!(Q::decode(&bad, &mut work()).is_err(), "{bad:?}");
    }
    for length in 0..third.len() {
        assert!(Q::decode(&third[..length], &mut work()).is_err());
    }
    let mut extra = third.clone();
    extra.push(0);
    assert_eq!(Q::decode(&extra, &mut work()), Err(R::NonCanonicalEncoding));
    let mut version = third;
    version[0] = 2;
    assert_eq!(Q::decode(&version, &mut work()), Err(R::UnsupportedVersion));
    assert_eq!(
        ratio(1, 3).encode(&mut [0; 1]),
        Err(R::EncodingBufferTooSmall)
    );
}

#[test]
fn comparison_and_cancellation_do_not_require_oversized_cross_products() {
    let power = Q::parse_decimal(&format!("1{}", "0".repeat(1000)), &mut work()).unwrap();
    let plus = power.checked_add(Q::ONE, &mut work()).unwrap();
    let minus = power.checked_subtract(Q::ONE, &mut work()).unwrap();
    let left = power.checked_divide(plus, &mut work()).unwrap();
    let right = minus.checked_divide(power, &mut work()).unwrap();
    assert_eq!(
        left.checked_compare(right, &mut work()).unwrap(),
        Ordering::Greater
    );
    assert_eq!(
        left.negated()
            .checked_compare(right.negated(), &mut work())
            .unwrap(),
        Ordering::Less
    );
    let inverse = plus.checked_divide(power, &mut work()).unwrap();
    assert_eq!(left.checked_multiply(inverse, &mut work()).unwrap(), Q::ONE);
    assert_eq!(Q::decode(&encoded(left), &mut work()).unwrap(), left);
}
