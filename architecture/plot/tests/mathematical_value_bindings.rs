use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
#[path = "common/generated_rust.rs"]
mod generated_rust;
#[test]
fn generated_native_carrier_preserves_exact_symbolic_meaning() {
    let syntax =
        conduit_plot::parse_syntax_document("type Sample = {\n exact: MathematicalValue\n}\n");
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();
    generated_rust::exercise(&generated.source, EXERCISE, 1, "mathematical-value");
}
const EXERCISE: &str = r#"
#[cfg(test)]
mod acceptance {
    use super::*;
    #[test]
    fn roundtrip() {
        let pi = conduit_core::MathematicalValue::parse_literal("π", &mut conduit_core::ExactNumericWork::new(
            conduit_core::MATHEMATICAL_VALUE_VALIDATION_WORK)).unwrap();
        let sample = Sample::new(pi.clone()).unwrap();
        let value = sample.into_structured().unwrap();
        let decoded = Sample::from_structured(value).unwrap();
        assert_eq!(decoded.exact,pi);
    }
}
"#;
