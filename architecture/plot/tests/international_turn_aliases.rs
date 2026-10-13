use conduit_core::{Quantity, Unit};
use conduit_plot::{
    CanonicalStartupValue, StartupCatalog, check_syntax_document,
    checked_physical_catalog_for_document, parse_syntax_document,
};

const SOURCE: &str = include_str!("../../../plots/units/international-turns.conduit");
const NAMES: &[&str] = &[
    "tour",
    "toer",
    "revolución",
    "rivoluzione",
    "revolução",
    "Umdrehung",
    "varv",
    "obrót",
];

#[test]
fn checked_import_preserves_unicode_spelling_and_exact_turn_reference() {
    let document = parse_syntax_document(SOURCE);
    assert_eq!(document.round_trip().as_bytes(), SOURCE.as_bytes());
    let base = StartupCatalog::new();
    let imported = checked_physical_catalog_for_document(&document, &base).unwrap();
    let turn = conduit_core::builtin_angle_unit();
    for name in NAMES {
        assert!(
            Unit::resolve(name).is_err(),
            "optional vocabulary must not become global"
        );
        let source = format!(
            "plot sample {{\n unit_value = {name}\n amount = 1{name}\n quarter = 0.25{name}\n}}\n"
        );
        let checked = check_syntax_document(&parse_syntax_document(&source), &imported).unwrap();
        let values = &checked.plots[0].local_values;
        let get = |key: &str| &values.iter().find(|(n, _)| n == key).unwrap().1;
        let CanonicalStartupValue::Unit(unit) = get("unit_value") else {
            panic!("Unit")
        };
        assert_eq!(unit.source().as_bytes(), name.as_bytes());
        assert_eq!(unit.value().reference_anchor(), turn.reference_anchor());
        assert!(unit.value().same_physical_definition(turn));
        for (key, expected) in [
            ("amount", Quantity::new(1, turn)),
            ("quarter", Quantity::from_decimal(25, -2, turn).unwrap()),
        ] {
            let CanonicalStartupValue::Quantity(q) = get(key) else {
                panic!("Angle")
            };
            assert_eq!(q.value().convert(turn).unwrap(), expected);
        }
    }
}

#[test]
fn missing_import_and_unsupported_spelling_refuse_without_normalization() {
    let imported = checked_physical_catalog_for_document(
        &parse_syntax_document(SOURCE),
        &StartupCatalog::new(),
    )
    .unwrap();
    for (suffix, catalog) in [
        ("tour", StartupCatalog::new()),
        ("revolucio\u{301}n", imported.clone()),
        ("revolucion", imported),
    ] {
        let source = format!("plot sample (\n amount: Angle = 1{suffix}\n) {{\n}}\n");
        let error = check_syntax_document(&parse_syntax_document(&source), &catalog).unwrap_err();
        assert_eq!(error.code, "CND-FRM-055", "{suffix}");
        assert!(
            error.message.contains("undeclared Unit suffix"),
            "{}",
            error.message
        );
    }
}

#[test]
fn imported_vocabulary_cannot_rebind_existing_units() {
    let base = StartupCatalog::new();
    let imported =
        checked_physical_catalog_for_document(&parse_syntax_document(SOURCE), &base).unwrap();
    for (name, catalog) in [("turn", base), ("tour", imported)] {
        let source = format!("unit {name} : Angle = {{ reference: turn, scale: 2 }}\n");
        let error =
            checked_physical_catalog_for_document(&parse_syntax_document(&source), &catalog)
                .unwrap_err();
        assert_eq!(error.code, "CND-FRM-058");
        assert!(
            error.message.contains("unit symbol conflicts"),
            "{}",
            error.message
        );
    }
}
