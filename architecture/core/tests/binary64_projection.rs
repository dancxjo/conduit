use conduit_core::{
    Binary64ErrorClaim as Claim, Binary64Refusal as Refusal, Binary64Rounding as Mode,
    ExactNumericRefusal as Numeric, ExactNumericWork as Work, ExactRational as Q, IeeeF64,
    PreparedBinary64Projection as Prepared,
};
fn work() -> Work {
    Work::new(100_000_000)
}

#[test]
fn tininess_after_rounding_checks_unbounded_exponent_precision_at_the_normal_boundary() {
    // Independently checked with SoftFloat 3e, after-rounding tininess:
    // min-normal * predecessor(1) rounds to min-normal in nearest-even,
    // but both underflow and inexact are still raised.
    let magnitude =
        Q::from_binary64(IeeeF64::from_bits(0x0010_0000_0000_0000), &mut work()).unwrap();
    let factor = Q::from_binary64(IeeeF64::from_bits(0x3fef_ffff_ffff_ffff), &mut work()).unwrap();
    let source = magnitude.checked_multiply(factor, &mut work()).unwrap();
    for negative in [false, true] {
        let source = if negative { source.negated() } else { source };
        for mode in [
            Mode::NearestTiesEven,
            Mode::TowardZero,
            Mode::TowardPositive,
            Mode::TowardNegative,
        ] {
            let p = point(source, mode, magnitude);
            let rounds_to_normal = mode == Mode::NearestTiesEven
                || (mode == Mode::TowardPositive && !negative)
                || (mode == Mode::TowardNegative && negative);
            let expected = if rounds_to_normal {
                0x0010_0000_0000_0000
            } else {
                0x000f_ffff_ffff_ffff
            } | if negative { 1 << 63 } else { 0 };
            assert_eq!(p.target().bits(), expected);
            assert!(p.status().inexact);
            assert!(p.status().tiny_inexact, "negative={negative} mode={mode:?}");
        }
    }
}

#[test]
fn precision_rounding_midpoint_distinguishes_tiny_and_non_tiny_normal_results() {
    // Binary128 -> binary64 SoftFloat oracle, all signs and supported modes.
    let normal = dyadic(-1022);
    for power in [-1075, -1076, -1077] {
        let positive = normal.checked_subtract(dyadic(power), &mut work()).unwrap();
        for negative in [false, true] {
            let source = if negative {
                positive.negated()
            } else {
                positive
            };
            for mode in [
                Mode::NearestTiesEven,
                Mode::TowardZero,
                Mode::TowardPositive,
                Mode::TowardNegative,
            ] {
                let p = point(source, mode, normal);
                let rounds_to_normal = mode == Mode::NearestTiesEven
                    || (mode == Mode::TowardPositive && !negative)
                    || (mode == Mode::TowardNegative && negative);
                let magnitude = if rounds_to_normal {
                    1 << 52
                } else {
                    (1 << 52) - 1
                };
                assert_eq!(
                    p.target().bits(),
                    magnitude | if negative { 1 << 63 } else { 0 }
                );
                assert!(p.status().inexact);
                assert_eq!(
                    p.status().tiny_inexact,
                    power == -1075 || !rounds_to_normal,
                    "power={power} negative={negative} mode={mode:?}"
                );
            }
        }
    }
}
fn q(s: &str) -> Q {
    Q::parse_decimal(s, &mut work()).unwrap()
}
fn dyadic(exponent: i16) -> Q {
    Q::from_i128(2)
        .checked_power(exponent, &mut work())
        .unwrap()
}
fn point(source: Q, mode: Mode, tolerance: Q) -> Prepared {
    Prepared::prepare(source, mode, tolerance, &mut work()).unwrap()
}
#[test]
fn exact_decimal_projection_carries_the_actual_rounding_error_and_direction() {
    let p = point(q("0.3"), Mode::NearestTiesEven, q("0.00000000000000002"));
    assert_eq!(p.target().bits(), 0x3fd3_3333_3333_3333);
    assert_eq!(
        p.error(),
        Claim::ExactDistance(Q::ratio(1, 90_071_992_547_409_920, &mut work()).unwrap())
    );
    assert!(p.status().inexact);
    assert!(!p.status().tiny_inexact);
    assert!(p.spent_limb_work() > 0);
    assert!(p.spent_limb_work() <= p.admitted_limb_work());
    for (source, mode, bits) in [
        ("0.3", Mode::TowardZero, 0x3fd3_3333_3333_3333),
        ("0.3", Mode::TowardPositive, 0x3fd3_3333_3333_3334),
        ("0.3", Mode::TowardNegative, 0x3fd3_3333_3333_3333),
        ("-0.3", Mode::TowardZero, 0xbfd3_3333_3333_3333),
        ("-0.3", Mode::TowardPositive, 0xbfd3_3333_3333_3333),
        ("-0.3", Mode::TowardNegative, 0xbfd3_3333_3333_3334),
    ] {
        assert_eq!(point(q(source), mode, Q::ONE).target().bits(), bits);
    }
}
#[test]
fn ties_even_exact_points_subnormals_and_negative_zero_are_distinct() {
    let half_ulp = dyadic(-53);
    for (offset, bits) in [(1, 0x3ff0_0000_0000_0000), (3, 0x3ff0_0000_0000_0002)] {
        let value = Q::ONE
            .checked_add(
                half_ulp
                    .checked_multiply(Q::from_i128(offset), &mut work())
                    .unwrap(),
                &mut work(),
            )
            .unwrap();
        assert_eq!(
            point(value, Mode::NearestTiesEven, Q::ONE).target().bits(),
            bits
        );
    }
    let tiny = dyadic(-1074);
    let exact = point(tiny, Mode::NearestTiesEven, Q::ZERO);
    assert_eq!(exact.target().bits(), 1);
    assert!(!exact.status().inexact);
    assert!(!exact.status().tiny_inexact);
    let half = tiny.checked_divide(Q::from_i128(2), &mut work()).unwrap();
    let negative = point(half.negated(), Mode::NearestTiesEven, half);
    assert_eq!(negative.target().bits(), 1 << 63);
    assert!(negative.status().tiny_inexact);
    assert_eq!(point(half, Mode::TowardPositive, half).target().bits(), 1);
    assert_eq!(
        point(half.negated(), Mode::TowardNegative, half)
            .target()
            .bits(),
        (1 << 63) | 1
    );
    let normal = Q::from_binary64(IeeeF64::from_bits(1 << 52), &mut work()).unwrap();
    let boundary = normal.checked_subtract(half, &mut work()).unwrap();
    let p = point(boundary, Mode::NearestTiesEven, half);
    assert_eq!(p.target().bits(), 1 << 52);
    assert!(p.status().inexact);
    assert!(p.status().tiny_inexact);
    for bits in [0, 1 << 63, 1, (1 << 63) | 1, 1 << 52, 0x7fef_ffff_ffff_ffff] {
        let source = Q::from_binary64(IeeeF64::from_bits(bits), &mut work()).unwrap();
        let result = point(source, Mode::NearestTiesEven, Q::ZERO);
        assert_eq!(
            result.target().bits(),
            if bits == 1 << 63 { 0 } else { bits }
        );
    }
}
#[test]
fn full_extent_denotation_can_get_a_certified_bound_without_oversized_error_scratch() {
    let base = dyadic(4094);
    let numerator = base
        .checked_multiply(Q::from_i128(3), &mut work())
        .unwrap()
        .checked_add(Q::ONE, &mut work())
        .unwrap();
    let denominator = base
        .checked_multiply(Q::from_i128(2), &mut work())
        .unwrap()
        .checked_subtract(Q::ONE, &mut work())
        .unwrap();
    let source = numerator.checked_divide(denominator, &mut work()).unwrap();
    let tolerance = dyadic(-52);
    let p = point(source, Mode::NearestTiesEven, tolerance);
    assert_eq!(p.target().bits(), 0x3ff8_0000_0000_0000);
    assert_eq!(p.error(), Claim::CertifiedBound(tolerance));
}
#[test]
fn precision_range_work_and_nonfinite_refuse_explicitly() {
    assert_eq!(
        Prepared::prepare(q("0.3"), Mode::NearestTiesEven, Q::ZERO, &mut work()),
        Err(Refusal::ToleranceExceeded)
    );
    assert_eq!(
        Prepared::prepare(Q::ONE, Mode::NearestTiesEven, Q::ONE.negated(), &mut work()),
        Err(Refusal::NegativeTolerance)
    );
    assert_eq!(
        Prepared::prepare(Q::ONE, Mode::NearestTiesEven, Q::ZERO, &mut Work::new(0)),
        Err(Refusal::Numeric(Numeric::WorkLimit))
    );
    assert_eq!(
        Prepared::prepare(dyadic(1024), Mode::NearestTiesEven, Q::ONE, &mut work()),
        Err(Refusal::Numeric(Numeric::OutOfRange))
    );
    for bits in [
        0x7ff0_0000_0000_0000,
        0x7ff8_0000_0000_0001,
        0xfff0_0000_0000_0000,
    ] {
        assert_eq!(
            Q::from_binary64(IeeeF64::from_bits(bits), &mut work()),
            Err(Numeric::OutOfRange)
        );
        assert_eq!(
            IeeeF64::decode(&IeeeF64::from_bits(bits).encode())
                .unwrap()
                .bits(),
            bits
        );
    }
}
#[test]
fn many_finite_ieee_bits_round_trip_without_a_native_float_conversion() {
    let mut state = 0xcafebabe12345678u64;
    for _ in 0..256 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        if (state >> 52) & 0x7ff == 0x7ff {
            continue;
        }
        let source = Q::from_binary64(IeeeF64::from_bits(state), &mut work()).unwrap();
        let p = point(source, Mode::NearestTiesEven, Q::ZERO);
        assert_eq!(p.target().bits(), state);
        assert_eq!(p.error(), Claim::ExactDistance(Q::ZERO));
    }
}

#[test]
fn independent_small_integer_ratios_match_single_native_division_rounding() {
    let mut state = 0x61a45eb79cf02318u64;
    for _ in 0..128 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let numerator = (state & ((1 << 20) - 1)) + 1;
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let denominator = (state & ((1 << 20) - 1)) + 1;
        let source = Q::ratio(numerator as i128, denominator as i128, &mut work()).unwrap();
        let target = point(source, Mode::NearestTiesEven, Q::ONE).target();
        // Both native integer conversions are exact; division is the one rounding.
        assert_eq!(
            target.bits(),
            ((numerator as f64) / (denominator as f64)).to_bits()
        );
    }
}
