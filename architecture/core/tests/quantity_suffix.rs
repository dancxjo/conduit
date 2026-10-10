use conduit_core::{
    QuantityLiteralRefusal, QuantitySuffixAlias, QuantitySuffixRefusal, ResolvedQuantitySuffix,
    Unit, DECIMAL_PREFIXES, PREFIXABLE_UNITS,
};

#[test]
fn every_reviewed_prefix_position_resolves_with_rich_descriptors() {
    for base in PREFIXABLE_UNITS {
        let unprefixed =
            ResolvedQuantitySuffix::resolve(base.catalogue_unit().plot_suffix()).unwrap();
        assert_eq!(unprefixed.base(), Some(base));
        assert_eq!(unprefixed.prefix(), None);
        assert_eq!(unprefixed.decimal_exponent(), Some(0));
        for prefix in DECIMAL_PREFIXES {
            let source = format!("{}{}", prefix.symbol(), base.catalogue_unit().plot_suffix());
            let resolved = ResolvedQuantitySuffix::resolve(&source).unwrap();
            assert_eq!(resolved.source(), source);
            assert_eq!(resolved.base(), Some(base));
            assert_eq!(resolved.prefix(), Some(prefix));
            assert_eq!(
                resolved.decimal_exponent(),
                Some(base.composed_exponent(prefix))
            );
            assert_eq!(resolved.alias(), QuantitySuffixAlias::Canonical);
            assert_eq!(resolved.unit(), Unit::from_plot_suffix(&source).unwrap());
        }
    }
}

#[test]
fn reviewed_catalogue_and_aliases_resolve_rich_units() {
    for source in [
        "us", "uV", "uA", "uAh", "um", "angstrom", "udeg", "mdeg", "deg", "urad", "permille", "ug",
        "mm2", "cm2", "m2", "km2", "uL", "m3", "m/s2", "g0",
    ] {
        assert_eq!(
            ResolvedQuantitySuffix::resolve(source).unwrap().unit(),
            Unit::from_plot_suffix(source).unwrap()
        );
    }
    let micro_square = ResolvedQuantitySuffix::resolve("um2").unwrap();
    assert_eq!(
        micro_square.alias(),
        QuantitySuffixAlias::AsciiMicroAndPower
    );
    assert_eq!(micro_square.decimal_exponent(), Some(-12));
    assert_eq!(micro_square.source().as_bytes(), b"um2");
}

#[test]
fn whole_suffixes_preserve_case_and_refuse_stacking_and_stealth_tokens() {
    for source in [
        "mkg",
        "kkg",
        "mµm",
        "kkHz",
        "kmin",
        "kh",
        "kpx",
        "k%",
        "kone",
        "μm",
        " mm",
        "mm ",
        "m\u{200b}m",
        "ｍm",
        "Km",
        "kMiB",
        "µ°C",
    ] {
        assert_eq!(
            ResolvedQuantitySuffix::resolve(source),
            Err(QuantitySuffixRefusal::Literal(
                QuantityLiteralRefusal::UnknownUnit
            )),
            "{source:?}"
        );
    }
    assert_eq!(
        ResolvedQuantitySuffix::resolve("C"),
        Err(QuantitySuffixRefusal::Literal(
            QuantityLiteralRefusal::NonCanonicalUnit { canonical: "°C" }
        ))
    );
    assert_eq!(
        ResolvedQuantitySuffix::resolve(""),
        Err(QuantitySuffixRefusal::Literal(
            QuantityLiteralRefusal::MissingUnit
        ))
    );
    let milli = ResolvedQuantitySuffix::resolve("mW").unwrap();
    let mega = ResolvedQuantitySuffix::resolve("MW").unwrap();
    assert_eq!(milli.decimal_exponent(), Some(-3));
    assert_eq!(mega.decimal_exponent(), Some(6));
    assert_eq!(
        ResolvedQuantitySuffix::resolve("MB")
            .unwrap()
            .decimal_exponent(),
        Some(6)
    );
    assert_eq!(
        ResolvedQuantitySuffix::resolve("MiB")
            .unwrap()
            .decimal_exponent(),
        None
    );
    assert_eq!(
        ResolvedQuantitySuffix::resolve("kg")
            .unwrap()
            .decimal_exponent(),
        Some(3)
    );
    assert_eq!(
        ResolvedQuantitySuffix::resolve("cm²")
            .unwrap()
            .decimal_exponent(),
        Some(-4)
    );
    assert_eq!(ResolvedQuantitySuffix::resolve("m°C").unwrap().base(), None);
}
