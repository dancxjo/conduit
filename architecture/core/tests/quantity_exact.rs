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
