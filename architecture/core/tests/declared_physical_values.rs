use conduit_core::*;
use core::cmp::Ordering;
fn scalar(n: i128, d: i128, e: i16) -> DefinitionScalar {
    DefinitionScalar::new(n, d, e).unwrap()
}
fn custom_family(name: &str) -> QuantityFamilyDefinition {
    let dimension = DimensionDefinition::new(&[DimensionTerm {
        anchor: DimensionDefinition::anchor("custom-temperature").unwrap(),
        power: 1,
    }])
    .unwrap();
    QuantityFamilyDefinition::point_delta(name, &format!("{name}Delta"), dimension).unwrap()
}
#[test]
fn self_contained_custom_affine_and_delta_laws_survive_transport() {
    let family = custom_family("Position");
    let origin = UnitDefinition::new_exact_role(
        family,
        "root",
        QuantityRole::Point,
        scalar(1, 1, 0),
        scalar(0, 1, 0),
        PrefixPolicy::NONE,
    )
    .unwrap();
    let related = UnitDefinition::related_exact_role(
        origin,
        "mark",
        QuantityRole::Point,
        scalar(5, 9, 0),
        scalar(27315, 1, -2),
        PrefixPolicy::NONE,
    )
    .unwrap();
    let root = Unit::from_definition(origin);
    let unit = Unit::from_definition(related);
    let point = Quantity::from_decimal(9, 0, unit).unwrap();
    assert_eq!(point.convert_to_unit(root), Ok((27815, -2)));
    let delta = Quantity::from_decimal_role(9, 0, unit, QuantityRole::Delta).unwrap();
    assert_eq!(delta.convert_to_unit(root), Ok((5, 0)));
    assert_eq!(
        point.compare(delta),
        Err(QuantityConversionRefusal::IncompatibleQuantityRoles)
    );
    assert_eq!(Quantity::decode(&delta.encode()), Ok(delta));
    let authored = "PositionDelta(9, mark)";
    assert!(delta.matches_literal_evidence(authored));
    assert!(!delta.matches_literal_evidence("9mark"));
    let receipt =
        ExactQuantityConversionReceipt::from_checked(delta, root, authored, "root").unwrap();
    assert_eq!(receipt.result().unwrap().coefficient(), 5);
    let wrapper = QuantityConfigurationValue::new(delta, authored.into()).unwrap();
    assert_eq!(
        serde_json::from_str::<QuantityConfigurationValue>(
            &serde_json::to_string(&wrapper).unwrap()
        )
        .unwrap(),
        wrapper
    );
    let role_kind = quantity_role_info_id(family, QuantityRole::Delta).unwrap();
    let contract = CheckedValueContract::new(
        kind_id(&role_kind),
        QUANTITY_ENCODED_LEN as u32,
        vec![ValueConstraint::QuantityRange {
            minimum: Some(delta),
            maximum: None,
            minimum_endpoint: IntervalEndpoint::Inclusive,
            maximum_endpoint: IntervalEndpoint::Inclusive,
        }],
    )
    .unwrap();
    assert_eq!(contract.validate(&delta.encode()), Ok(()));
    assert!(contract.validate(&point.encode()).is_err());
    assert_eq!(validate_primitive_info(&role_kind, &delta.encode()), Ok(()));
    assert_eq!(
        validate_primitive_info(&role_kind, &point.encode()),
        Err(PrimitiveInfoRefusal::WrongQuantityRole)
    );
    let other = Unit::from_definition(
        UnitDefinition::new_exact(
            custom_family("Other"),
            "other",
            scalar(1, 1, 0),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap(),
    );
    assert_eq!(
        point.compare(Quantity::new(9, other)),
        Err(QuantityConversionRefusal::IncompatibleQuantityFamilies)
    );
    assert_eq!(
        validate_primitive_info(&role_kind, &Quantity::new(9, other).encode()),
        Err(PrimitiveInfoRefusal::WrongQuantityFamily)
    );
}
#[test]
fn independent_origins_refuse_and_prefixes_never_scale_the_affine_offset() {
    let family = custom_family("CustomTemperature");
    let root = UnitDefinition::new_exact(
        family,
        "base",
        scalar(1, 1, 0),
        scalar(0, 1, 0),
        PrefixPolicy::NONE,
    )
    .unwrap();
    let policy = PrefixPolicy::new(&[-3, 3], &[10], 2).unwrap();
    let affine =
        UnitDefinition::related_exact(root, "affine", scalar(1, 1, 0), scalar(2, 1, 3), policy)
            .unwrap();
    let milli = Unit::from_definition(affine.with_decimal_prefix("maffine", -3).unwrap());
    assert_eq!(
        Quantity::new(1, milli).convert_to_unit(Unit::from_definition(root)),
        Ok((2000000001, -6))
    );
    let binary = Unit::from_definition(affine.with_binary_prefix("Kiaffine", 10).unwrap());
    assert_eq!(
        Quantity::new(1, binary).convert_to_unit(Unit::from_definition(root)),
        Ok((1050576, 0))
    );
    let independent = Unit::from_definition(root.with_named_origin("independent").unwrap());
    assert_eq!(
        Quantity::new(1, independent).compare(Quantity::new(1, Unit::from_definition(root))),
        Err(QuantityConversionRefusal::Inexact)
    );
    let same = Quantity::from_decimal(1, 128, Unit::from_definition(root)).unwrap();
    assert_eq!(same.compare(same), Ok(Ordering::Equal));
}
#[test]
fn rich_codec_is_the_only_codec_and_every_role_leaf_checks_the_full_capsule() {
    assert_eq!(UNIT_ENCODED_LEN, 768);
    assert_eq!(QUANTITY_ENCODED_LEN, 788);
    for &(symbol, definition) in BUILTIN_UNIT_DEFINITIONS {
        let unit = Unit::from_definition(definition);
        assert_eq!(unit.symbol(), symbol);
        let quantity = Quantity::new(1, unit);
        let kind = quantity_role_info_id(unit.family(), quantity.role()).unwrap();
        assert_eq!(validate_primitive_info(&kind, &quantity.encode()), Ok(()));
        let ty = StructuredInfoType::leaf(kind_id(&kind)).unwrap();
        assert!(StructuredInfoValue::leaf(ty, quantity.encode().to_vec()).is_ok());
    }
    assert_eq!(primitive_info_kind("value/frequency"), None);
    assert!(Unit::decode(&[1, 1, 0]).is_err());
    assert!(Quantity::decode(&[0; 9]).is_err());
}

#[test]
fn maximal_names_have_bounded_constructor_evidence_without_a_builtin_lookup() {
    let point_name = "P".repeat(DEFINITION_NAME_BYTES);
    let delta_name = "D".repeat(64);
    let symbol = "u".repeat(UNIT_MAX_SOURCE_BYTES);
    let family = QuantityFamilyDefinition::point_delta(
        &point_name,
        &delta_name,
        DimensionDefinition::DIMENSIONLESS,
    )
    .unwrap();
    let unit = Unit::from_definition(
        UnitDefinition::new_exact(
            family,
            &symbol,
            scalar(1, 1, 0),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap(),
    );
    let quantity = Quantity::new(1, unit);
    let number = format!("1.{}", "0".repeat(90));
    let source = format!("{point_name}({number}, {symbol})");
    assert!(source.len() > 128 && source.len() <= QUANTITY_MAX_LITERAL_BYTES);
    assert!(quantity.matches_literal_evidence(&source));
    let checked = QuantityConfigurationValue::new(quantity, source).unwrap();
    assert_eq!(checked.value(), quantity);
    assert!(!quantity.matches_literal_evidence(&" ".repeat(QUANTITY_MAX_LITERAL_BYTES + 1)));
}

#[test]
fn exact_relations_cancel_binary_factors_before_bounded_admission() {
    let dimension = DimensionDefinition::DIMENSIONLESS;
    let family =
        QuantityFamilyDefinition::new("BinaryCoordinate", dimension, QuantityRoles::Linear)
            .unwrap();
    let policy = PrefixPolicy::new(&[], &[80], 2).unwrap();
    let base = UnitDefinition::new_exact(
        family,
        "base",
        scalar(1, 1_i128 << 40, 0),
        scalar(0, 1, 0),
        policy,
    )
    .unwrap();
    let reference = base.with_binary_prefix("Yibase", 80).unwrap();
    for unit in [
        UnitDefinition::related_exact(
            reference,
            "derived",
            scalar(1, 1, 0),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap(),
        UnitDefinition::related_exact_role(
            reference,
            "role-derived",
            QuantityRole::Linear,
            scalar(1, 1, 0),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap(),
    ] {
        assert!(unit.exact_scale().equivalent(scalar(1_i128 << 120, 1, 0)));
        assert_eq!(UnitDefinition::decode(&unit.encode()), Ok(unit));
    }
    let unscaled =
        UnitDefinition::new_exact(family, "unscaled", scalar(1, 1, 0), scalar(0, 1, 0), policy)
            .unwrap()
            .with_binary_prefix("Yiunscaled", 80)
            .unwrap();
    let cancelled = UnitDefinition::related_exact(
        unscaled,
        "cancelled",
        scalar(1, 1_i128 << 40, 0),
        scalar(0, 1, 0),
        PrefixPolicy::NONE,
    )
    .unwrap();
    assert!(cancelled
        .exact_scale()
        .equivalent(scalar(1_i128 << 120, 1, 0)));
    assert!(scalar(1, 1, 0)
        .multiply_binary(160)
        .unwrap()
        .equivalent_binary(0, scalar(1, 1, 0), 160));
}

#[test]
fn exact_relation_exponent_spill_uses_available_coefficient_capacity() {
    assert_eq!(
        scalar(1, 1, 128).multiply(scalar(1, 1, 1)),
        Ok(scalar(10, 1, 128))
    );
    assert_eq!(
        scalar(1, 1, -128).multiply(scalar(1, 1, -1)),
        Ok(scalar(1, 10, -128))
    );
    assert_eq!(
        scalar(0, 1, 0).multiply(scalar(1, 1, 128)),
        Ok(scalar(0, 1, 0))
    );
    let family = QuantityFamilyDefinition::new(
        "LargeCoordinate",
        DimensionDefinition::DIMENSIONLESS,
        QuantityRoles::Linear,
    )
    .unwrap();
    for (exponent, relation, expected) in [
        (128, 1, scalar(10, 1, 128)),
        (-128, -1, scalar(1, 10, -128)),
    ] {
        let reference = UnitDefinition::new_exact(
            family,
            "base",
            scalar(1, 1, exponent),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap();
        let related = UnitDefinition::related_exact(
            reference,
            "related",
            scalar(1, 1, relation),
            scalar(0, 1, 0),
            PrefixPolicy::NONE,
        )
        .unwrap();
        assert!(related.exact_scale().equivalent(expected));
        assert_eq!(UnitDefinition::decode(&related.encode()), Ok(related));
    }
}

#[test]
fn differently_written_exact_offset_cancellation_never_expands_a_large_power() {
    let positive = scalar(1_i128 << 120, 1, 0);
    let negative = scalar(-(1_i128 << 80), 5_i128.pow(40), 40);
    assert_eq!(positive.add(negative), Ok(scalar(0, 1, 0)));
    assert_eq!(negative.add(positive), Ok(scalar(0, 1, 0)));
}

#[test]
fn source_default_angle_is_turn_and_quarter_turn_is_exact() {
    let default = builtin_angle_unit();
    assert_eq!(default.symbol(), "turn");
    assert_eq!(
        Unit::default_for_builtin_family(Unit::Degree.family()),
        Some(default)
    );
    assert!(default.exact_scale().equivalent(scalar(1, 1, 0)));
    let authored = Quantity::parse_plot_literal("90°").unwrap();
    let normalized = authored.convert(default).unwrap();
    assert_eq!(
        normalized,
        Quantity::parse_plot_literal("0.25turn").unwrap()
    );
    assert_eq!(authored.unit().symbol(), "°");
    assert_eq!(authored.compare(normalized), Ok(Ordering::Equal));
    assert_eq!(
        Quantity::new(1, Unit::Turn).convert(Unit::Degree),
        Ok(Quantity::new(360, Unit::Degree))
    );
    assert_eq!(
        Quantity::new(1, Unit::Radian).convert(default),
        Err(QuantityConversionRefusal::Inexact)
    );
}

#[test]
fn root_declaration_identity_keeps_distinct_snapshots_separate() {
    let family = custom_family("SnapshotPosition");
    let root = |symbol, scale, offset, role| {
        UnitDefinition::new_exact_role(family, symbol, role, scale, offset, PrefixPolicy::NONE)
            .unwrap()
    };
    let first = root(
        "origin",
        scalar(1, 2, 0),
        scalar(3, 1, 0),
        QuantityRole::Point,
    );
    let equivalent = root(
        "origin",
        scalar(5, 1, -1),
        scalar(3, 1, 0),
        QuantityRole::Point,
    );
    assert_eq!(first.reference_anchor(), equivalent.reference_anchor());
    for changed in [
        root(
            "other",
            scalar(1, 2, 0),
            scalar(3, 1, 0),
            QuantityRole::Point,
        ),
        root(
            "origin",
            scalar(1, 1, 0),
            scalar(3, 1, 0),
            QuantityRole::Point,
        ),
        root(
            "origin",
            scalar(1, 2, 0),
            scalar(4, 1, 0),
            QuantityRole::Point,
        ),
    ] {
        assert_ne!(first.reference_anchor(), changed.reference_anchor());
        let left = Unit::from_definition(first);
        let right = Unit::from_definition(changed);
        assert!(!left.same_physical_definition(right));
        assert_eq!(
            Quantity::new(1, left).convert_to_unit(right),
            Err(QuantityConversionRefusal::Inexact)
        );
        assert_eq!(
            Quantity::new(1, left).compare(Quantity::new(1, right)),
            Err(QuantityConversionRefusal::Inexact)
        );
        assert_ne!(
            first
                .with_named_origin("shared")
                .unwrap()
                .reference_anchor(),
            changed
                .with_named_origin("shared")
                .unwrap()
                .reference_anchor()
        );
    }
    let derivative = root(
        "origin",
        scalar(1, 2, 0),
        scalar(0, 1, 0),
        QuantityRole::Delta,
    )
    .with_reference_origin(first)
    .unwrap();
    assert_eq!(derivative.reference_anchor(), first.reference_anchor());
    assert_eq!(
        derivative.exact_offset(QuantityRole::Delta).unwrap(),
        scalar(0, 1, 0)
    );
}
