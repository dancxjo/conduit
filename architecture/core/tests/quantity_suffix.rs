use conduit_core::*;
#[test]
fn suffix_evidence_uses_embedded_capsule_without_a_global_registry() {
    let dim = DimensionDefinition::new(&[DimensionTerm {
        anchor: DimensionDefinition::anchor("custom").unwrap(),
        power: 1,
    }])
    .unwrap();
    let family = QuantityFamilyDefinition::new("Custom", dim, QuantityRoles::Linear).unwrap();
    let def = UnitDefinition::new_exact(
        family,
        "span",
        DefinitionScalar::new(3, 2, 0).unwrap(),
        DefinitionScalar::new(0, 1, 0).unwrap(),
        PrefixPolicy::NONE,
    )
    .unwrap();
    let unit = Unit::from_definition(def);
    assert!(Unit::resolve("span").is_err());
    let suffix = ResolvedQuantitySuffix::from_unit("span", unit).unwrap();
    assert_eq!(suffix.source(), "span");
    assert_eq!(suffix.unit(), unit);
    assert!(ResolvedQuantitySuffix::from_unit("other", unit).is_err());
    let quantity = Quantity::from_decimal(2, 0, unit).unwrap();
    assert!(quantity.matches_literal_evidence("2span"));
    assert!(!quantity.matches_literal_evidence("2other"));
    assert!(QuantityConfigurationValue::new(quantity, "2span".into()).is_some());
    assert!(UnitConfigurationValue::new(unit, "span".into()).is_some());
}
#[test]
fn whole_suffix_resolution_preserves_case_and_refuses_stacking() {
    for spelling in [
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
        "C",
        "",
    ] {
        assert!(
            ResolvedQuantitySuffix::resolve(spelling).is_err(),
            "{spelling:?}"
        );
    }
    for (spelling, exp) in [("mW", -3), ("MW", 6), ("kg", 3), ("cm²", -4)] {
        assert_eq!(
            ResolvedQuantitySuffix::resolve(spelling)
                .unwrap()
                .unit()
                .decimal_exponent(),
            exp
        );
    }
    assert_eq!(
        ResolvedQuantitySuffix::resolve("MiB")
            .unwrap()
            .unit()
            .binary_exponent(),
        20
    );
}
