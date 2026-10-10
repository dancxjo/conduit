use conduit_core::*;

#[test]
fn physical_unit_catalogue_codec_is_bounded_and_distinct() {
    for base in PREFIXABLE_UNITS {
        for prefix in DECIMAL_PREFIXES {
            let spelling = format!("{}{}", prefix.symbol(), base.unit().plot_suffix());
            let unit = Unit::resolve(&spelling).unwrap();
            assert_eq!(unit.dimension(), base.dimension());
            assert_eq!(unit.decimal_exponent(), base.composed_exponent(prefix));
            assert_eq!(unit.catalogue_id(), QUANTITY_PREFIX_CATALOG_ID);
            assert_eq!(Unit::decode(&unit.encode()), Ok(unit));
            assert_eq!(
                validate_primitive_info(UNIT_INFO_ID, &unit.encode()),
                Ok(())
            );
            assert!(validate_primitive_info(QUANTITY_INFO_ID, &unit.encode()).is_err());
            assert!(validate_primitive_info(EMPTY_INFO_ID, &unit.encode()).is_err());
        }
    }
    assert_eq!(Unit::resolve("um"), Unit::resolve("µm"));
    assert_eq!(Unit::resolve("km2"), Unit::resolve("km²"));
    assert_ne!(Unit::resolve("Hz"), Unit::resolve("kHz"));
    assert!(Unit::resolve("C").is_err());
    assert!(Unit::resolve("KHZ").is_err());
    assert!(Unit::resolve("kkg").is_err());
    assert!(Unit::resolve("γm").is_err());
    assert!(Unit::decode(&[]).is_err());
    assert!(Unit::decode(&[2, Unit::Meter.encode()[0], 0]).is_err());
    assert!(Unit::decode(&[1, Unit::Celsius.encode()[0], 3]).is_err());
    assert!(Unit::decode(&[1, Unit::Meter.encode()[0], 4]).is_err());
    // A historical prefixed tag is not a second canonical encoding.
    assert!(Unit::decode(&[1, Unit::Kilohertz.encode()[0], 0]).is_err());
}

#[test]
fn owned_unit_retains_exact_prefix_and_affine_conversion_laws() {
    let source = Quantity::parse_plot_literal("1kHz").unwrap();
    let hz = Unit::resolve("Hz").unwrap();
    assert_eq!(source.convert_to_unit(hz), Ok((1, 3)));
    assert_eq!(
        source.convert_to_unit(Unit::resolve("QHz").unwrap()),
        Ok((1, -27))
    );
    assert_eq!(
        source.convert_to_unit(Unit::resolve("m").unwrap()),
        Err(QuantityConversionRefusal::IncompatibleDimensions)
    );
    assert_eq!(Unit::resolve("Qm³").unwrap().decimal_exponent(), 90);
    assert_eq!(Unit::resolve("QHz").unwrap().decimal_exponent(), 30);
    let celsius = Quantity::parse_plot_literal("21°C").unwrap();
    assert_eq!(
        celsius.convert_to_unit(Unit::resolve("°F").unwrap()),
        Ok((698, -1))
    );
    let difference = ExactTemperatureDifference::parse_plot_literal("21°C").unwrap();
    assert_eq!(
        difference.convert_to_unit(Unit::resolve("°F").unwrap()),
        Ok((378, -1))
    );
    let (_, offset, _, exponent) = Unit::resolve("°C").unwrap().reference_transform();
    assert_ne!(offset, 0);
    assert_eq!(exponent, 0);
}

#[test]
fn typed_configuration_separates_spelling_from_checked_semantics() {
    let source = QuantityConfigurationValue::parse("1kHz").unwrap();
    let equivalent = QuantityConfigurationValue::parse("1000Hz").unwrap();
    assert_ne!(source.source(), equivalent.source());
    assert_ne!(source.canonical_value(), equivalent.canonical_value());
    assert_eq!(
        source.value().compare(equivalent.value()),
        Ok(core::cmp::Ordering::Equal)
    );
    assert_eq!(
        ConfigurationValue::Quantity(source.clone()).semantic_kind(),
        kind_id(QUANTITY_INFO_ID)
    );
    assert_eq!(
        ConfigurationValue::Unit(UnitConfigurationValue::parse("Hz").unwrap()).semantic_kind(),
        kind_id(UNIT_INFO_ID)
    );
    assert!(QuantityConfigurationValue::new(source.value(), "2kHz".into()).is_none());
    let unit = UnitConfigurationValue::parse("um").unwrap();
    assert_eq!(unit.source(), "um");
    assert!(UnitConfigurationValue::new(unit.value(), "nm".into()).is_none());
    assert!(ExactTemperatureDifferenceConfigurationValue::parse("21m").is_err());
    let roundtrip: QuantityConfigurationValue =
        serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
    assert_eq!(roundtrip, source);
    let mut encoded = serde_json::to_value(&source).unwrap();
    encoded["source"] = serde_json::json!("2kHz");
    assert!(serde_json::from_value::<QuantityConfigurationValue>(encoded).is_err());
    let mut encoded = serde_json::to_value(&unit).unwrap();
    encoded["canonical_value"] = serde_json::json!([1, Unit::Celsius.encode()[0], 3]);
    assert!(serde_json::from_value::<UnitConfigurationValue>(encoded).is_err());
}

#[test]
fn typed_receipt_construction_checks_evidence_without_using_it_as_conversion_input() {
    let source = Quantity::parse_plot_literal("1kHz").unwrap();
    let target = Unit::resolve("Hz").unwrap();
    let receipt =
        ExactQuantityConversionReceipt::from_checked(source, target, "1kHz", "Hz").unwrap();
    assert_eq!(receipt.source(), source);
    assert_eq!(receipt.result().unwrap().coefficient(), 1);
    assert_eq!(receipt.result().unwrap().exponent(), 3);
    assert_eq!(
        ExactQuantityConversionReceipt::from_checked(source, target, "2kHz", "Hz"),
        Err(ExactQuantityConversionRequestRefusal::SourceEvidenceMismatch)
    );
    assert_eq!(
        ExactQuantityConversionReceipt::from_checked(source, target, "1kHz", "kHz"),
        Err(ExactQuantityConversionRequestRefusal::TargetEvidenceMismatch)
    );
    let difference = ExactTemperatureDifference::parse_plot_literal("21°C").unwrap();
    let receipt = ExactTemperatureDifferenceConversionReceipt::from_checked(
        difference,
        Unit::resolve("°F").unwrap(),
        "21°C",
        "°F",
    )
    .unwrap();
    assert_eq!(receipt.result().unwrap().coefficient(), 378);
    assert_eq!(receipt.result().unwrap().exponent(), -1);
}

#[test]
fn temperature_difference_configuration_owns_its_structured_profile_identity() {
    let difference = ExactTemperatureDifferenceConfigurationValue::parse("21°C").unwrap();
    let value = ConfigurationValue::TemperatureDifference(difference.clone());
    let expected = exact_temperature_difference_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    assert_eq!(value.semantic_kind(), expected);
    assert_ne!(
        value.semantic_kind(),
        kind_id(EXACT_TEMPERATURE_DIFFERENCE_INFO_ID)
    );
    assert_ne!(value.semantic_kind(), kind_id(QUANTITY_INFO_ID));
    let roundtrip: ConfigurationValue =
        serde_json::from_str(&serde_json::to_string(&value).unwrap()).unwrap();
    assert_eq!(roundtrip.semantic_kind(), expected);
    assert_eq!(roundtrip, value);
    let mut encoded = serde_json::to_value(&difference).unwrap();
    encoded["source"] = serde_json::json!("22°C");
    assert!(
        serde_json::from_value::<ExactTemperatureDifferenceConfigurationValue>(encoded).is_err()
    );
}
