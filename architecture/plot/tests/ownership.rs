#[test]
fn universal_value_fallback_names_only_language_primitives() {
    let source = include_str!("../src/value_type.rs");
    for forbidden in ["ScalarField2", "alife/", "Face", "Manifestation", "face/"] {
        assert!(
            !source.contains(forbidden),
            "domain value spelling returned to universal Plot ownership: {forbidden}"
        );
    }
}
