//! Compact turn spellings preserve exact checked Angle meaning.
use conduit_core::{Quantity, Unit};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, StartupCatalog,
};

#[test]
fn compact_turn_aliases_are_units_and_exact_angle_quantities() {
    let source = "plot angles {\n a = 1tr\n b = 1rev\n c = 1rot\n d = 1pla\n reference = 1turn\n degrees = 360°\n gradians = 400gon\n quarter = 0.25tr\n ninety = 90°\n twice = 2rev\n unit_value = tr\n}\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let values = &checked.plots[0].local_values;
    let get = |name: &str| &values.iter().find(|(n, _)| n == name).unwrap().1;
    let turn = conduit_core::builtin_angle_unit();
    for name in ["tr", "rev", "rot", "pla"] {
        let unit = Unit::resolve(name).unwrap();
        assert!(unit.same_physical_definition(turn));
        assert_eq!(Unit::decode(&unit.encode()).unwrap(), unit);
    }
    for name in ["a", "b", "c", "d", "reference", "degrees", "gradians"] {
        let CanonicalStartupValue::Quantity(value) = get(name) else {
            panic!("{name} must be a Quantity")
        };
        assert_eq!(value.value().convert(turn).unwrap(), Quantity::new(1, turn));
    }
    for name in ["quarter", "ninety"] {
        let CanonicalStartupValue::Quantity(value) = get(name) else {
            panic!("{name} must be a Quantity")
        };
        assert_eq!(
            value.value().convert(turn).unwrap(),
            Quantity::from_decimal(25, -2, turn).unwrap()
        );
    }
    let CanonicalStartupValue::Quantity(twice) = get("twice") else {
        panic!("Angle Quantity")
    };
    assert_eq!(twice.value().convert(turn).unwrap(), Quantity::new(2, turn));
    let CanonicalStartupValue::Unit(unit) = get("unit_value") else {
        panic!("bare tr must be a Unit")
    };
    assert_eq!(
        Quantity::new(1, unit.value()).convert(turn).unwrap(),
        Quantity::new(1, turn)
    );
}

#[test]
fn unknown_and_colliding_turn_aliases_are_refused() {
    for (source, code, message) in [
        (
            "plot invalid (\n angle: Angle = 1trr\n) {\n}\n",
            "CND-FRM-055",
            "undeclared Unit suffix",
        ),
        (
            "unit tr : Angle = { reference: turn, scale: 2 }\nplot invalid {\n}\n",
            "CND-FRM-058",
            "unit symbol conflicts",
        ),
    ] {
        let error = check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new())
            .unwrap_err();
        assert_eq!(error.code, code);
        assert!(error.message.contains(message), "{}", error.message);
    }
}
