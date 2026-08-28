#[test]
fn universal_value_fallback_names_only_language_primitives() {
    let source = include_str!("../src/value_type.rs");
    for forbidden in [
        "ScalarField2",
        "alife/",
        "Presentation",
        "Manifestation",
        "presentation/",
    ] {
        assert!(
            !source.contains(forbidden),
            "domain value spelling returned to universal Form ownership: {forbidden}"
        );
    }
}
