use super::*;
use crate::prelude::*;

fn expression(source: &str) -> crate::ExpressionSyntax {
    crate::pure_expression::parse(source, source, 0).unwrap()
}

fn named(source: &str) -> crate::SpannedText {
    match expression(source) {
        crate::ExpressionSyntax::Atomic(name) => name,
        _ => panic!("name"),
    }
}

fn temperature() -> (
    Vec<DimensionDeclarationSyntax>,
    Vec<(crate::SpannedText, QuantityDefinitionSyntax)>,
) {
    (
        alloc::vec![parse_dimension_declaration(
            "dimension temperature",
            "dimension temperature",
            0
        )
        .unwrap()],
        alloc::vec![
            (
                named("TemperatureDelta"),
                parse_quantity_definition(&expression("{ dimension: temperature }")).unwrap()
            ),
            (
                named("Temperature"),
                parse_quantity_definition(&expression("{ point: TemperatureDelta }")).unwrap()
            ),
        ],
    )
}

fn unit(source: &str) -> UnitDeclarationSyntax {
    parse_unit_declaration(source, source, 0).unwrap()
}

#[test]
fn physical_declaration_scalars_walk_the_ordinary_exact_expression_ast() {
    assert_eq!(
        check_exact_scalar(&expression("5/9")).unwrap(),
        ExactScalar {
            numerator: 5,
            denominator: 9,
            decimal_exponent: 0
        }
    );
    assert_eq!(
        check_exact_scalar(&expression("{ numerator: 5, denominator: 9 }")).unwrap(),
        check_exact_scalar(&expression("5/9")).unwrap()
    );
    assert_eq!(
        check_exact_scalar(&expression("-273.15")).unwrap(),
        ExactScalar {
            numerator: -27315,
            denominator: 1,
            decimal_exponent: -2
        }
    );
    assert!(check_exact_scalar(&expression("1/0")).is_err());
    assert!(check_exact_scalar(&expression("\"5/9\"")).is_err());
    assert!(check_exact_scalar(&expression("1kHz")).is_err());
}

#[test]
fn point_and_difference_declarations_admit_forward_references_without_prefix_defaults() {
    let (dimensions, quantities) = temperature();
    let units = alloc::vec![
        unit("unit °C : Temperature = { reference: K, scale: 1, offset: 273.15, delta: { quantity: TemperatureDelta, reference: K, scale: 1 } }"),
        unit("unit K : Temperature = { reference: origin, scale: 1, delta: { quantity: TemperatureDelta, reference: origin, scale: 1 }, prefixes: si }"),
    ];
    let order = admit_declaration_graph(&dimensions, &quantities, &units).unwrap();
    assert_eq!(order.units, alloc::vec![1, 0]);
    assert_eq!(
        order.families["TemperatureDelta"],
        Some("Temperature".into())
    );
    assert!(!units[0].prefixes.si);
    assert!(units[1].prefixes.si);
    assert!(units[0].transform.offset.is_some());
    assert!(units[0]
        .difference
        .as_ref()
        .unwrap()
        .transform
        .offset
        .is_none());
}

#[test]
fn unit_reference_cycles_multiple_roots_and_unanchored_families_are_refused() {
    let (dimensions, quantities) = temperature();
    let root = unit("unit K : Temperature = { reference: origin, scale: 1, delta: { quantity: TemperatureDelta, reference: origin, scale: 1 } }");
    let cycle_a = unit("unit A : Temperature = { reference: B, scale: 1, delta: { quantity: TemperatureDelta, reference: B, scale: 1 } }");
    let cycle_b = unit("unit B : Temperature = { reference: A, scale: 1, delta: { quantity: TemperatureDelta, reference: A, scale: 1 } }");
    assert!(admit_declaration_graph(
        &dimensions,
        &quantities,
        &[root.clone(), cycle_a.clone(), cycle_b.clone()]
    )
    .unwrap_err()
    .message
    .contains("cycle"));
    assert!(
        admit_declaration_graph(&dimensions, &quantities, &[cycle_a, cycle_b])
            .unwrap_err()
            .message
            .contains("no root")
    );
    let second = unit("unit Root : Temperature = { reference: origin, scale: 1, delta: { quantity: TemperatureDelta, reference: origin, scale: 1 } }");
    assert!(
        admit_declaration_graph(&dimensions, &quantities, &[root, second])
            .unwrap_err()
            .message
            .contains("multiple root")
    );
}

#[test]
fn authored_role_and_exact_rational_boundaries_are_checked() {
    let (dimensions, quantities) = temperature();
    let missing_delta = unit("unit K : Temperature = { reference: origin, scale: 1 }");
    assert!(admit_declaration_graph(&dimensions, &quantities, &[missing_delta]).is_err());
    let wrong_role = unit("unit K : Temperature = { reference: origin, scale: 1, delta: { quantity: Temperature, reference: origin, scale: 1 } }");
    assert!(admit_declaration_graph(&dimensions, &quantities, &[wrong_role]).is_err());
    assert!(parse_unit_declaration(
        "unit X : Temperature = { reference: origin, scale: 1, scale: 2 }",
        "unit X : Temperature = { reference: origin, scale: 1, scale: 2 }",
        0
    )
    .is_err());
}

#[test]
fn physical_source_spans_retain_the_declared_exact_scalar() {
    let source = "unit °C : Temperature = { reference: K, scale: 1, offset: 273.15, delta: { quantity: TemperatureDelta, reference: K, scale: 1 } }";
    let value = unit(source);
    let at = value.transform.offset.unwrap().span();
    assert_eq!(&source[at.start..at.end], "273.15");
    assert_eq!(
        &source[value.symbol.span.start..value.symbol.span.end],
        "°C"
    );
}

#[test]
fn prefix_entries_are_source_authored_and_aliases_deduplicate_exact_policy_exponents() {
    let prefix = |text: &str| parse_prefix_declaration(text, text, 0).unwrap();
    let prefixes = [
        prefix("prefix si u = { exponent: -6, alias: µ }"),
        prefix("prefix si µ = { exponent: -6 }"),
        prefix("prefix binary Ki = { exponent: 10 }"),
    ];
    let checked = admit_prefix_declarations(&prefixes).unwrap();
    assert_eq!(checked.decimal_exponents, alloc::vec![-6]);
    assert_eq!(checked.binary_exponents, alloc::vec![10]);
    assert_eq!(checked.canonical_prefixes, alloc::vec![1, 1, 2]);
    let conflicting = [
        prefix("prefix si u = { exponent: -6, alias: µ }"),
        prefix("prefix si µ = { exponent: -3 }"),
    ];
    assert!(admit_prefix_declarations(&conflicting)
        .unwrap_err()
        .message
        .contains("different exponent"));
}

#[test]
fn independent_exact_origins_are_explicit_and_duplicate_named_origins_refuse() {
    let dimensions =
        [parse_dimension_declaration("dimension angle", "dimension angle", 0).unwrap()];
    let quantities = [(
        named("Angle"),
        parse_quantity_definition(&expression("{ dimension: angle }")).unwrap(),
    )];
    let degrees = unit("unit ° : Angle = { reference: origin, scale: 1 }");
    let radians = unit("unit rad : Angle = { reference: origin(radian), scale: 1 }");
    assert!(admit_declaration_graph(
        &dimensions,
        &quantities,
        &[degrees.clone(), radians.clone()]
    )
    .is_ok());
    let duplicate = unit("unit alternate : Angle = { reference: origin(radian), scale: 1 }");
    assert!(
        admit_declaration_graph(&dimensions, &quantities, &[degrees, radians, duplicate])
            .unwrap_err()
            .message
            .contains("multiple root")
    );
}

#[test]
fn checked_builtin_language_is_the_authority_for_every_generated_capsule() {
    let document = crate::parse_syntax_document(conduit_core::BUILTIN_PHYSICAL_SOURCE);
    assert!(
        document.diagnostics.is_empty(),
        "{:?}",
        document.diagnostics
    );
    let quantities = document
        .types
        .iter()
        .map(|declaration| {
            let crate::TypeDefinitionSyntax::Quantity(definition) = &declaration.definition else {
                panic!("physical bootstrap contains a nonphysical Type");
            };
            (declaration.name.clone(), definition.clone())
        })
        .collect::<Vec<_>>();
    let checked = check_physical_declarations(
        &document.dimensions,
        &document.prefixes,
        &quantities,
        &document.units,
    )
    .unwrap();
    for (symbol, generated) in conduit_core::BUILTIN_UNIT_DEFINITIONS {
        let authored = checked.units.get(*symbol).unwrap_or_else(|| {
            panic!("generated unit '{symbol}' has no checked source definition")
        });
        assert_eq!(
            authored.encode(),
            generated.encode(),
            "generated unit '{symbol}' differs from its authoritative checked source"
        );
        assert_eq!(authored.identity(), generated.identity());
    }
    for (family_name, generated) in conduit_core::BUILTIN_DEFAULT_UNIT_DEFINITIONS {
        let authored = checked
            .units
            .get(generated.symbol())
            .expect("default Unit is authored");
        assert_eq!(
            authored.encode(),
            generated.encode(),
            "default for {family_name}"
        );
        assert_eq!(authored.family(), checked.quantities[*family_name].family);
    }
    let turn = conduit_core::Unit::from_definition(checked.units["turn"]);
    let quarter_turn =
        conduit_core::Quantity::new(90, conduit_core::Unit::from_definition(checked.units["°"]))
            .convert(turn)
            .unwrap();
    assert_eq!(
        quarter_turn,
        conduit_core::Quantity::from_decimal(25, -2, turn).unwrap()
    );
    assert_eq!(conduit_core::builtin_angle_unit(), turn);
    for (name, generated) in conduit_core::BUILTIN_QUANTITY_ROLE_INFO_IDS {
        assert_eq!(
            checked.quantities[*name].leaf_kind.as_str(),
            *generated,
            "generated quantity role '{name}' differs from checked source"
        );
    }
    for (group, symbol, exponent, alias) in conduit_core::BUILTIN_PREFIX_DEFINITIONS {
        let authored = document
            .prefixes
            .iter()
            .find(|prefix| prefix.group.text == *group && prefix.symbol.text == *symbol)
            .unwrap_or_else(|| {
                panic!("generated prefix '{symbol}' has no checked source declaration")
            });
        assert_eq!(
            check_exact_scalar(&authored.exponent).unwrap().integer(),
            Some(i128::from(*exponent))
        );
        assert_eq!(
            authored.alias.as_ref().map(|name| name.text.as_str()),
            *alias
        );
    }
}

#[test]
fn quantity_constructor_accepts_forward_ordinary_unit_values_without_reinterpreting_points() {
    let document=crate::parse_syntax_document("plot sample {\n change = TemperatureDelta(amount, target)\n target = °F\n amount = 9\n point = 9°F\n}\n");
    let checked = crate::check_syntax_document(&document, &crate::StartupCatalog::new()).unwrap();
    let values = &checked.plots[0].local_values;
    let delta = values.iter().find(|(name, _)| name == "change").unwrap();
    let crate::CanonicalStartupValue::Quantity(delta) = &delta.1 else {
        panic!("Delta quantity")
    };
    assert_eq!(delta.value().role(), conduit_core::QuantityRole::Delta);
    assert_eq!(delta.source(), "TemperatureDelta(9, °F)");
    let point = values.iter().find(|(name, _)| name == "point").unwrap();
    let crate::CanonicalStartupValue::Quantity(point) = &point.1 else {
        panic!("Point quantity")
    };
    assert_eq!(point.value().role(), conduit_core::QuantityRole::Point);
    assert!(document
        .round_trip()
        .contains("TemperatureDelta(amount, target)"));
}

#[test]
fn admitted_physical_context_reuse_preserves_snapshot_and_checks_new_sources() {
    let empty = crate::parse_syntax_document("");
    let base = super::context::install(&empty, &crate::StartupCatalog::new()).unwrap();
    let extension =
        crate::parse_syntax_document("unit smoot : Distance = { reference: m, scale: 1.7018 }\n");
    let snapshot = super::context::install(&extension, &base).unwrap();
    assert_eq!(
        super::context::install(&empty, &snapshot).unwrap(),
        snapshot
    );
    assert_eq!(
        super::context::install(&extension, &snapshot).unwrap(),
        snapshot
    );
    let borrowed = super::context::install_borrowed(&empty, &snapshot).unwrap();
    assert!(
        matches!(borrowed, alloc::borrow::Cow::Borrowed(value) if core::ptr::eq(value, &snapshot))
    );
    let borrowed = super::context::install_borrowed(&extension, &snapshot).unwrap();
    assert!(
        matches!(borrowed, alloc::borrow::Cow::Borrowed(value) if core::ptr::eq(value, &snapshot))
    );
    assert!(!base.physical.units.contains_key("smoot"));
    assert!(snapshot.physical.units.contains_key("smoot"));
    let changed =
        crate::parse_syntax_document("unit smoot : Distance = { reference: m, scale: 0.8509 }\n");
    assert!(super::context::install(&changed, &snapshot).is_err());
    let replacement = super::context::install(&changed, &base).unwrap();
    assert_ne!(
        snapshot.physical.units["smoot"],
        replacement.physical.units["smoot"]
    );
    let unrelated =
        crate::parse_syntax_document("unit stride : Distance = { reference: m, scale: 2 }\n");
    let extended = super::context::install(&unrelated, &snapshot).unwrap();
    assert_eq!(
        extended.physical.units["smoot"],
        snapshot.physical.units["smoot"]
    );
    assert!(extended.physical.units.contains_key("stride"));
    assert!(!snapshot.physical.units.contains_key("stride"));
}
