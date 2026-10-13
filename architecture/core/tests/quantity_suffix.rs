use conduit_core::*;
#[test]
fn suffix_evidence_uses_embedded_capsule_without_a_global_registry() {
    let dim = DimensionDefinition::new(&[DimensionTerm {
        anchor: DimensionDefinition::anchor("custom").unwrap(),
        power: 1,
    }])
    .unwrap();
    let family = QuantityFamilyDefinition::new("Custom", dim, QuantityRoles::Linear).unwrap();
    let def = UnitDefinition::new_math_role(
        family,
        "span",
        MathValue::parse_decimal("1.5").unwrap(),
        MathValue::integer(0),
        PrefixPolicy::NONE,
        QuantityRole::Linear,
    )
    .unwrap();
    let unit = Unit::from_definition(def);
    assert!(Unit::resolve("span").is_err());
    let suffix = ResolvedQuantitySuffix::from_unit("span", unit.clone()).unwrap();
    assert_eq!(suffix.source(), "span");
    assert_eq!(suffix.unit(), unit);
    assert!(ResolvedQuantitySuffix::from_unit("other", unit.clone()).is_err());
    let quantity = Quantity::from_decimal(2, 0, unit.clone()).unwrap();
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
                .exact_scale()
                .to_owned()
                .unwrap(),
            MathValue::from_decimal_parts(1, exp).unwrap()
        );
    }
    assert_eq!(
        ResolvedQuantitySuffix::resolve("MiB")
            .unwrap()
            .unit()
            .exact_scale()
            .to_owned()
            .unwrap(),
        MathValue::integer(1_048_576)
    );
}
