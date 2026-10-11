use conduit_core::{
    Quantity as Exact, Quantity, QuantityDimension, QuantityRefusal as Refusal, Unit,
    QUANTITY_ENCODED_LEN,
};

#[test]
fn reviewed_units_and_prefixes_retain_coordinates_and_physical_equality() {
    for source in [
        "1Qm", "1qm", "1Qm³", "1qm³", "3.2m", "0.001km", "1000mm", "-1.50km", "440Hz", "250ms",
        "21°C", "1m°C", "1MiB", "1MB", "1um²",
    ] {
        let quantity = Exact::parse_plot_literal(source).unwrap();
        assert_eq!(Exact::decode(&quantity.encode()), Ok(quantity));
        assert_eq!(
            quantity
                .canonical_literal()
                .and_then(|s| Exact::parse_plot_literal(&s)),
            Ok(quantity)
        );
    }
    let first = Exact::parse_plot_literal("1000mm").unwrap();
    let second = Exact::parse_plot_literal("1m").unwrap();
    assert_ne!(first.encode(), second.encode());
    assert_eq!(first.compare(second), Ok(core::cmp::Ordering::Equal));
    assert_eq!(
        second.compare(Exact::parse_plot_literal("0.001km").unwrap()),
        Ok(core::cmp::Ordering::Equal)
    );
}

#[test]
fn all_reviewed_scales_match_the_independent_arbitrary_precision_corpus() {
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/quantity_prefix_scales.json")).unwrap();
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 456);
    for case in cases {
        let source = case["source"].as_str().unwrap();
        let quantity = Exact::parse_plot_literal(source).unwrap();
        let base = Unit::resolve(case["base"].as_str().unwrap()).unwrap();
        let exponent =
            quantity.exponent() + quantity.unit().decimal_exponent() - base.decimal_exponent();
        let (numerator, denominator) = if exponent >= 0 {
            (
                format!(
                    "{}{}",
                    quantity.coefficient(),
                    "0".repeat(exponent as usize)
                ),
                "1".to_owned(),
            )
        } else {
            (
                quantity.coefficient().to_string(),
                format!("1{}", "0".repeat((-exponent) as usize)),
            )
        };
        assert_eq!(
            numerator,
            case["relative_numerator"].as_str().unwrap(),
            "{source}"
        );
        assert_eq!(
            denominator,
            case["relative_denominator"].as_str().unwrap(),
            "{source}"
        );
    }
}

#[test]
fn upper_exponent_literals_use_available_coefficient_capacity() {
    for zeroes in [38, 39, 40, 75] {
        for sign in [1_i128, -1] {
            let literal = format!(
                "{}1{}Qm³",
                if sign < 0 { "-" } else { "" },
                "0".repeat(zeroes)
            );
            let quantity = Exact::parse_plot_literal(&literal).unwrap();
            let exponent = zeroes.min(128) as i16;
            let coefficient = sign * 10_i128.pow((zeroes - exponent as usize) as u32);
            assert_eq!(
                quantity,
                Exact::from_decimal(coefficient, exponent, Unit::resolve("Qm³").unwrap()).unwrap()
            );
            assert_eq!(Exact::decode(&quantity.encode()), Ok(quantity));
            let receipt =
                conduit_core::ExactQuantityConversionReceipt::check(&literal, "Qm³").unwrap();
            assert_eq!(receipt.original(), literal);
            assert_eq!(receipt.source(), quantity);
            let projected = receipt.result().unwrap();
            assert_eq!(
                (projected.coefficient(), projected.exponent()),
                (sign, zeroes as i16)
            );
        }
    }
    assert_eq!(
        Exact::parse_plot_literal(&format!("1{}Qm³", "0".repeat(96))),
        Err(Refusal::NumberTooLong)
    );
    // Codec fields remain bounded even when a literal can be rescaled exactly.
    assert_eq!(
        Exact::from_decimal(1, 129, Unit::CubicMeter),
        Err(Refusal::ExponentOutOfRange)
    );
}

#[test]
fn bounded_precision_and_input_work_refuse_without_rounding() {
    let largest = "9".repeat(38);
    assert!(Exact::parse_plot_literal(&format!("{largest}m")).is_ok());
    assert_eq!(
        Exact::parse_plot_literal(&format!("{}m", "9".repeat(39))),
        Err(Refusal::SignificantDigitsExceeded)
    );
    // Trailing zero padding consumes input work but not significand precision.
    assert_eq!(
        Exact::parse_plot_literal(&format!("1.{}m", "0".repeat(90))),
        Exact::parse_plot_literal("1m")
    );
    assert_eq!(
        Exact::parse_plot_literal(&format!("1.{}m", "0".repeat(95))),
        Err(Refusal::NumberTooLong)
    );
    assert_eq!(
        Exact::parse_plot_literal(&format!("{}m", "0".repeat(128))),
        Err(Refusal::NumberTooLong)
    );
    assert_eq!(
        Exact::from_decimal(1, -129, Unit::resolve("qm³").unwrap()),
        Err(Refusal::ExponentOutOfRange)
    );
    for source in ["1e999999m", "1E+999m", "1e-999m"] {
        assert_eq!(
            Exact::parse_plot_literal(source),
            Err(Refusal::UnsupportedExponentNotation)
        );
    }
    assert_eq!(
        Exact::parse_plot_literal("1Em")
            .unwrap()
            .unit()
            .decimal_exponent(),
        18
    );
    for source in ["", "-", "Hz", ".1m", "-.1m", "1.m", "1..2m"] {
        assert_eq!(
            Exact::parse_plot_literal(source),
            Err(Refusal::InvalidNumber),
            "{source}"
        );
    }
    assert_eq!(
        Exact::from_decimal(1, 129, Unit::Meter),
        Err(Refusal::ExponentOutOfRange)
    );
    assert_eq!(
        Exact::from_decimal(i128::MIN, 0, Unit::Meter),
        Err(Refusal::SignificantDigitsExceeded)
    );
}

#[test]
fn versioned_encoding_is_canonical_and_rejects_other_codec_widths() {
    let exact = Exact::parse_plot_literal("-0.000m").unwrap();
    assert_eq!((exact.coefficient(), exact.exponent()), (0, 0));
    let encoded = exact.encode();
    assert_eq!(encoded.len(), QUANTITY_ENCODED_LEN);
    assert_eq!(Exact::decode(&encoded), Ok(exact));
    assert_eq!(Exact::decode(&[0; 9]), Err(Refusal::WrongEncodingLength));
    let mut noncanonical = Exact::from_decimal(1, 0, Unit::Meter).unwrap().encode();
    noncanonical[4 + conduit_core::UNIT_ENCODED_LEN..].copy_from_slice(&10_i128.to_le_bytes());
    assert_eq!(
        Exact::decode(&noncanonical),
        Err(Refusal::NonCanonicalEncoding)
    );
    noncanonical[0] = 2;
    assert_eq!(
        Exact::decode(&noncanonical),
        Err(Refusal::UnsupportedEncodingVersion(2))
    );
    let upper = Exact::from_decimal(100, 127, Unit::Meter).unwrap();
    assert_eq!((upper.coefficient(), upper.exponent()), (10, 128));
    assert_eq!(Exact::decode(&upper.encode()), Ok(upper));
    assert_eq!(upper.dimension(), QuantityDimension::Length);
}

#[test]
fn physical_conversion_and_comparison_keep_exact_scale_and_affine_laws() {
    use conduit_core::QuantityConversionRefusal as Conversion;
    use core::cmp::Ordering;
    for (source, target, value) in [
        ("1kHz", Unit::Hertz, 1000),
        ("1µs", Unit::Nanosecond, 1000),
        ("1cm²", Unit::SquareMillimeter, 100),
        ("0°C", Unit::Millikelvin, 273150),
        ("30°C", Unit::Fahrenheit, 86),
        ("1m°C", Unit::Millikelvin, 273151),
        ("1MiB", Unit::Byte, 1048576),
        ("1MB", Unit::Byte, 1000000),
        ("-9223372036854775808m", Unit::Meter, i64::MIN),
    ] {
        assert_eq!(
            Exact::parse_plot_literal(source).unwrap().to_i64(target),
            Ok(value),
            "{source}"
        );
    }
    for (left, right, ordering) in [
        ("1000mm", "1m", Ordering::Equal),
        ("0°C", "273.15K", Ordering::Equal),
        ("30°C", "86°F", Ordering::Equal),
        ("1Qm³", "1qm³", Ordering::Greater),
        ("-1Qm³", "-1qm³", Ordering::Less),
        ("1MB", "1MiB", Ordering::Less),
    ] {
        assert_eq!(
            Exact::parse_plot_literal(left)
                .unwrap()
                .compare(Exact::parse_plot_literal(right).unwrap()),
            Ok(ordering),
            "{left} vs {right}"
        );
    }
    for (source, target, refusal) in [
        ("1Qm", Unit::Meter, Conversion::Overflow),
        ("1qm", Unit::Meter, Conversion::Inexact),
        ("0°C", Unit::Kelvin, Conversion::Inexact),
        ("1m", Unit::Second, Conversion::IncompatibleDimensions),
        ("1rad", Unit::Degree, Conversion::Inexact),
    ] {
        assert_eq!(
            Exact::parse_plot_literal(source).unwrap().to_i64(target),
            Err(refusal),
            "{source}"
        );
    }
}

#[test]
fn extended_target_projection_admits_only_exact_bounded_decimals() {
    use conduit_core::QuantityConversionRefusal as Conversion;
    for (source, target, expected) in [
        ("0°C", Unit::Kelvin, "273.15K"),
        ("30°C", Unit::Fahrenheit, "86°F"),
        ("86°F", Unit::Celsius, "30°C"),
        ("1in", Unit::Meter, "0.0254m"),
        ("1qm³", Unit::CubicMeter, "1qm³"),
        ("1Qm³", Unit::CubicMeter, "1Qm³"),
        ("0m", Unit::Inch, "0in"),
        ("-273.15°C", Unit::Kelvin, "0K"),
    ] {
        let converted = Exact::parse_plot_literal(source)
            .unwrap()
            .convert_to_decimal(target)
            .unwrap();
        assert_eq!(converted.unit(), target);
        assert_eq!(
            converted.compare(Exact::parse_plot_literal(expected).unwrap()),
            Ok(core::cmp::Ordering::Equal),
            "{source}"
        );
    }
    assert_eq!(
        Exact::parse_plot_literal("1°F")
            .unwrap()
            .convert_to_decimal(Unit::Celsius),
        Err(Conversion::Inexact)
    );
    assert_eq!(
        Exact::parse_plot_literal("1m")
            .unwrap()
            .convert_to_decimal(Unit::Inch),
        Err(Conversion::Inexact)
    );
    let largest = Exact::from_decimal(10_i128.pow(38) - 1, 128, Unit::Meter).unwrap();
    assert_eq!(largest.convert_to_decimal(Unit::Meter), Ok(largest));
    assert_eq!(
        largest.convert_to_decimal(Unit::Millimeter),
        Err(Conversion::Overflow)
    );
    let smallest = Exact::from_decimal(1, -128, Unit::Meter).unwrap();
    assert_eq!(smallest.convert_to_decimal(Unit::Meter), Ok(smallest));
    assert_eq!(
        smallest.convert_to_decimal(Unit::Kilometer),
        Err(Conversion::Overflow)
    );
}

#[test]
fn recognized_scales_are_values_and_integer_projection_is_explicit() {
    for source in ["1Qm", "1qm", "1Qm³", "1qm³", "1um²", "1uW"] {
        assert!(Quantity::parse_plot_literal(source).is_ok(), "{source}");
    }
    assert_eq!(
        Quantity::parse_plot_literal("1Qm")
            .unwrap()
            .to_i64(Unit::Meter),
        Err(conduit_core::QuantityConversionRefusal::Overflow)
    );
    assert_eq!(
        Quantity::parse_plot_literal("1qm")
            .unwrap()
            .to_i64(Unit::Meter),
        Err(conduit_core::QuantityConversionRefusal::Inexact)
    );
    for source in ["1mkg", "1kkm", "1μm", "1qpx"] {
        assert!(Quantity::parse_plot_literal(source).is_err());
    }
}
