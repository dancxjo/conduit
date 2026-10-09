use conduit_core::{
    ExactDecimalQuantity as Exact, ExactDecimalQuantityRefusal as Refusal, Quantity,
    QuantityDimension, QuantityUnit, EXACT_DECIMAL_QUANTITY_ENCODED_LEN,
};

#[test]
fn extended_prefixes_admit_exact_coordinates_without_a_legacy_unit_tag() {
    for (source, coefficient, exponent, unit) in [
        ("1Qm", 1, 30, QuantityUnit::Meter),
        ("1qm", 1, -30, QuantityUnit::Meter),
        ("1dam", 1, 1, QuantityUnit::Meter),
        ("1Qm³", 1, 90, QuantityUnit::CubicMeter),
        ("1qm³", 1, -90, QuantityUnit::CubicMeter),
        ("3.2m", 32, -1, QuantityUnit::Meter),
        ("0.001km", 1, 0, QuantityUnit::Meter),
        ("1000mm", 1, 0, QuantityUnit::Meter),
        ("-1.50km", -15, 2, QuantityUnit::Meter),
        ("440Hz", 44, 1, QuantityUnit::Hertz),
        ("250ms", 25, -2, QuantityUnit::Second),
        ("21°C", 21, 0, QuantityUnit::Celsius),
        ("640px", 64, 1, QuantityUnit::Pixel),
        ("1m°C", 1, 0, QuantityUnit::MilliCelsius),
        ("1MiB", 1, 0, QuantityUnit::Mebibyte),
        ("1MB", 1, 6, QuantityUnit::Byte),
        ("1um2", 1, -12, QuantityUnit::SquareMeter),
    ] {
        let quantity = Exact::parse_plot_literal(source).unwrap();
        assert_eq!(
            (quantity.coefficient(), quantity.exponent(), quantity.unit()),
            (coefficient, exponent, unit),
            "{source}"
        );
        assert_eq!(quantity.dimension(), unit.dimension());
        assert_eq!(Exact::decode(&quantity.encode()), Ok(quantity));
    }
    assert_eq!(
        Exact::parse_plot_literal("1000mm"),
        Exact::parse_plot_literal("1m")
    );
    assert_eq!(
        Exact::parse_plot_literal("1m"),
        Exact::parse_plot_literal("0.001km")
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
        let (numerator, denominator) = if quantity.exponent() >= 0 {
            (
                format!(
                    "{}{}",
                    quantity.coefficient(),
                    "0".repeat(quantity.exponent() as usize)
                ),
                "1".to_owned(),
            )
        } else {
            (
                quantity.coefficient().to_string(),
                format!("1{}", "0".repeat((-quantity.exponent()) as usize)),
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
            let exponent = (90 + zeroes).min(128) as i16;
            let coefficient = sign * 10_i128.pow((90 + zeroes - exponent as usize) as u32);
            assert_eq!(
                quantity,
                Exact::new(coefficient, exponent, QuantityUnit::CubicMeter).unwrap()
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
        Exact::parse_plot_literal(&format!("1{}Qm³", "0".repeat(76))),
        Err(Refusal::SignificantDigitsExceeded)
    );
    // Codec fields remain bounded even when a literal can be rescaled exactly.
    assert_eq!(
        Exact::new(1, 129, QuantityUnit::CubicMeter),
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
        Err(Refusal::LiteralTooLong)
    );
    assert_eq!(
        Exact::parse_plot_literal(&format!("0.{}1qm³", "0".repeat(90))),
        Err(Refusal::ExponentOutOfRange)
    );
    for source in ["1e999999m", "1E+999m", "1e-999m"] {
        assert_eq!(
            Exact::parse_plot_literal(source),
            Err(Refusal::UnsupportedExponentNotation)
        );
    }
    assert_eq!(Exact::parse_plot_literal("1Em").unwrap().exponent(), 18);
    for source in ["", "-", "Hz", ".1m", "-.1m", "1.m", "1..2m"] {
        assert_eq!(
            Exact::parse_plot_literal(source),
            Err(Refusal::InvalidNumber),
            "{source}"
        );
    }
    assert_eq!(
        Exact::new(1, 129, QuantityUnit::Meter),
        Err(Refusal::ExponentOutOfRange)
    );
    assert_eq!(
        Exact::new(i128::MIN, 0, QuantityUnit::Meter),
        Err(Refusal::SignificantDigitsExceeded)
    );
}

#[test]
fn versioned_encoding_is_canonical_and_keeps_legacy_quantity_bytes_separate() {
    let exact = Exact::parse_plot_literal("-0.000m").unwrap();
    assert_eq!((exact.coefficient(), exact.exponent()), (0, 0));
    let encoded = exact.encode();
    assert_eq!(encoded.len(), EXACT_DECIMAL_QUANTITY_ENCODED_LEN);
    assert_eq!(Exact::decode(&encoded), Ok(exact));
    let legacy = Quantity::parse_plot_literal("3.2m").unwrap();
    assert_eq!(legacy.encode().len(), 9);
    assert_eq!(
        Exact::decode(&legacy.encode()),
        Err(Refusal::WrongEncodingLength)
    );
    assert_ne!(
        Exact::parse_plot_literal("3.2m").unwrap().semantic_digest(),
        legacy.semantic_digest()
    );
    let mut noncanonical = Exact::new(1, 0, QuantityUnit::Meter).unwrap().encode();
    noncanonical[4..].copy_from_slice(&10_i128.to_le_bytes());
    assert_eq!(
        Exact::decode(&noncanonical),
        Err(Refusal::NonCanonicalEncoding)
    );
    noncanonical[0] = 2;
    assert_eq!(
        Exact::decode(&noncanonical),
        Err(Refusal::UnsupportedEncodingVersion(2))
    );
    let upper = Exact::new(100, 127, QuantityUnit::Meter).unwrap();
    assert_eq!((upper.coefficient(), upper.exponent()), (10, 128));
    assert_eq!(Exact::decode(&upper.encode()), Ok(upper));
    assert_eq!(upper.dimension(), QuantityDimension::Length);
}

#[test]
fn physical_conversion_and_comparison_keep_exact_scale_and_affine_laws() {
    use conduit_core::QuantityConversionRefusal as Conversion;
    use core::cmp::Ordering;
    for (source, target, value) in [
        ("1kHz", QuantityUnit::Hertz, 1000),
        ("1µs", QuantityUnit::Nanosecond, 1000),
        ("1cm²", QuantityUnit::SquareMillimeter, 100),
        ("0°C", QuantityUnit::Millikelvin, 273150),
        ("30°C", QuantityUnit::Fahrenheit, 86),
        ("1m°C", QuantityUnit::Millikelvin, 273151),
        ("1MiB", QuantityUnit::Byte, 1048576),
        ("1MB", QuantityUnit::Byte, 1000000),
        ("-9223372036854775808m", QuantityUnit::Meter, i64::MIN),
    ] {
        assert_eq!(
            Exact::parse_plot_literal(source)
                .unwrap()
                .convert_to_legacy(target),
            Ok(Quantity::new(value, target)),
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
        ("1Qm", QuantityUnit::Meter, Conversion::Overflow),
        ("1qm", QuantityUnit::Meter, Conversion::Inexact),
        ("0°C", QuantityUnit::Kelvin, Conversion::Inexact),
        (
            "1m",
            QuantityUnit::Second,
            Conversion::IncompatibleDimensions,
        ),
        ("1rad", QuantityUnit::Degree, Conversion::Inexact),
    ] {
        assert_eq!(
            Exact::parse_plot_literal(source)
                .unwrap()
                .convert_to_legacy(target),
            Err(refusal),
            "{source}"
        );
    }
}

#[test]
fn extended_target_projection_admits_only_exact_bounded_decimals() {
    use conduit_core::QuantityConversionRefusal as Conversion;
    for (source, target, expected) in [
        ("0°C", QuantityUnit::Kelvin, "273.15K"),
        ("30°C", QuantityUnit::Fahrenheit, "86°F"),
        ("86°F", QuantityUnit::Celsius, "30°C"),
        ("1in", QuantityUnit::Meter, "0.0254m"),
        ("1qm³", QuantityUnit::CubicMeter, "1qm³"),
        ("1Qm³", QuantityUnit::CubicMeter, "1Qm³"),
        ("0m", QuantityUnit::Inch, "0in"),
        ("-273.15°C", QuantityUnit::Kelvin, "0K"),
    ] {
        assert_eq!(
            Exact::parse_plot_literal(source)
                .unwrap()
                .convert_to_decimal(target),
            Exact::parse_plot_literal(expected).map_err(|_| Conversion::Overflow),
            "{source}"
        );
    }
    assert_eq!(
        Exact::parse_plot_literal("1°F")
            .unwrap()
            .convert_to_decimal(QuantityUnit::Celsius),
        Err(Conversion::Inexact)
    );
    assert_eq!(
        Exact::parse_plot_literal("1m")
            .unwrap()
            .convert_to_decimal(QuantityUnit::Inch),
        Err(Conversion::Inexact)
    );
    let largest = Exact::new(10_i128.pow(38) - 1, 128, QuantityUnit::Meter).unwrap();
    assert_eq!(largest.convert_to_decimal(QuantityUnit::Meter), Ok(largest));
    assert_eq!(
        largest.convert_to_decimal(QuantityUnit::Millimeter),
        Err(Conversion::Overflow)
    );
    let smallest = Exact::new(1, -128, QuantityUnit::Meter).unwrap();
    assert_eq!(
        smallest.convert_to_decimal(QuantityUnit::Meter),
        Ok(smallest)
    );
    assert_eq!(
        smallest.convert_to_decimal(QuantityUnit::Kilometer),
        Err(Conversion::Overflow)
    );
}

#[test]
fn legacy_literal_target_distinguishes_known_scale_from_numeric_eligibility() {
    use conduit_core::{
        QuantityLiteralRefusal as Literal, QuantityRepresentationRefusal as Eligibility,
    };
    for source in ["1Qm", "1qm", "1Qm³", "1qm³", "1um2", "1uW"] {
        assert_eq!(
            Quantity::parse_plot_literal(source),
            Err(Literal::RepresentationIneligible {
                profile: conduit_core::QUANTITY_INFO_ID,
                reason: Eligibility::NoExactLegacyUnit,
            }),
            "{source}"
        );
        assert!(Exact::parse_plot_literal(source).is_ok());
    }
    for source in ["1dam", "1hm", "1Em", "1dam2", "1000uW", "1dg"] {
        let legacy = Quantity::parse_plot_literal(source).unwrap();
        let extended = Exact::parse_plot_literal(source).unwrap();
        assert_eq!(
            extended.convert_to_legacy(legacy.unit()),
            Ok(legacy),
            "{source}"
        );
    }
    // Coarser exact storage remains eligible even when the first fine unit
    // in the original search order would overflow.
    assert_eq!(
        Quantity::parse_plot_literal("1Em").unwrap().value(),
        1_000_000_000_000_000_000
    );
    for source in ["1mkg", "1kkm", "1μm", "1qpx"] {
        assert_eq!(
            Quantity::parse_plot_literal(source),
            Err(Literal::UnknownUnit)
        );
    }
}
