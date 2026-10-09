use conduit_core::{
    DecimalPrefix, PrefixableUnit, QuantityUnit, DECIMAL_PREFIXES, PREFIXABLE_UNITS,
};

#[test]
fn official_catalogue_and_complete_compatibility_matrix_are_exact() {
    // Independent fixture order differs from the catalogue; every official
    // exponent is exercised against every reviewed prefix position.
    let reference = [
        ("da", 1),
        ("h", 2),
        ("k", 3),
        ("M", 6),
        ("G", 9),
        ("T", 12),
        ("P", 15),
        ("E", 18),
        ("Z", 21),
        ("Y", 24),
        ("R", 27),
        ("Q", 30),
        ("d", -1),
        ("c", -2),
        ("m", -3),
        ("µ", -6),
        ("n", -9),
        ("p", -12),
        ("f", -15),
        ("a", -18),
        ("z", -21),
        ("y", -24),
        ("r", -27),
        ("q", -30),
    ];
    assert_eq!(DECIMAL_PREFIXES.len(), reference.len());
    for (symbol, exponent) in reference {
        let prefix = DecimalPrefix::from_symbol(symbol).unwrap();
        assert_eq!(prefix.exponent(), exponent);
        assert_eq!(prefix.symbol(), symbol);
        assert!(!prefix.name().is_empty());
        for base in PREFIXABLE_UNITS {
            assert_eq!(base.dimension(), base.unit().dimension());
            assert_eq!(
                base.composed_exponent(prefix),
                i16::from(exponent) * i16::from(base.prefix_power())
            );
        }
    }
    for (index, prefix) in DECIMAL_PREFIXES.iter().enumerate() {
        assert!(DECIMAL_PREFIXES[index + 1..]
            .iter()
            .all(
                |other| prefix.symbol() != other.symbol() && prefix.exponent() != other.exponent()
            ));
    }
}

#[test]
fn prefix_positions_handle_mass_powers_and_compounds_without_stacking() {
    let kilo = DecimalPrefix::from_symbol("k").unwrap();
    let centi = DecimalPrefix::from_symbol("c").unwrap();
    let quetta = DecimalPrefix::from_symbol("Q").unwrap();
    assert_eq!(
        PrefixableUnit::for_unit(QuantityUnit::Gram)
            .unwrap()
            .composed_exponent(kilo),
        3
    );
    assert_eq!(
        PrefixableUnit::for_unit(QuantityUnit::SquareMeter)
            .unwrap()
            .composed_exponent(centi),
        -4
    );
    assert_eq!(
        PrefixableUnit::for_unit(QuantityUnit::CubicMeter)
            .unwrap()
            .composed_exponent(quetta),
        90
    );
    assert_eq!(
        PrefixableUnit::for_unit(QuantityUnit::MeterPerSecondSquared)
            .unwrap()
            .composed_exponent(kilo),
        3
    );
    for unit in [
        QuantityUnit::Kilogram,
        QuantityUnit::Millimeter,
        QuantityUnit::Minute,
        QuantityUnit::Hour,
        QuantityUnit::JulianYear,
        QuantityUnit::Degree,
        QuantityUnit::Pixel,
        QuantityUnit::Percent,
        QuantityUnit::One,
        QuantityUnit::Celsius,
        QuantityUnit::MilliCelsius,
        QuantityUnit::Fahrenheit,
        QuantityUnit::Kibibyte,
        QuantityUnit::Mebibyte,
        QuantityUnit::Inch,
    ] {
        assert_eq!(PrefixableUnit::for_unit(unit), None, "{unit:?}");
    }
}

#[test]
fn canonical_prefix_symbols_do_not_admit_aliases_or_confusables() {
    for invalid in [
        "",
        "u",
        "μ",
        "K",
        "D",
        "mc",
        "kk",
        "Mi",
        "mµ",
        " µ",
        "µ ",
        "m\u{200b}",
    ] {
        assert_eq!(DecimalPrefix::from_symbol(invalid), None, "{invalid:?}");
    }
    assert_ne!(
        DecimalPrefix::from_symbol("m"),
        DecimalPrefix::from_symbol("M")
    );
    // Existing aliases remain an independent legacy source policy.
    assert_eq!(
        QuantityUnit::from_plot_suffix("us"),
        Ok(QuantityUnit::Microsecond)
    );
    assert_eq!(
        QuantityUnit::from_plot_suffix("µs"),
        Ok(QuantityUnit::Microsecond)
    );
    assert_eq!(
        QuantityUnit::from_plot_suffix("MiB"),
        Ok(QuantityUnit::Mebibyte)
    );
    assert_eq!(
        QuantityUnit::from_plot_suffix("m°C"),
        Ok(QuantityUnit::MilliCelsius)
    );
}
