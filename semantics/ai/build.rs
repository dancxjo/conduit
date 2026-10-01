use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("AI semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_exclusions: ["MissingModalityPolicy".into()].into(),
            serde_record_types: ["CompatibleMetrics".into()].into(),
            copy_record_types: ["CompatibleMetrics".into(), "IntegrationAccuracy".into()].into(),
            copy_record_value_getters: ["IntegrationAccuracy".into()].into(),
            public_record_fields: ["IntegrationAccuracy".into()].into(),
            record_constructor_orders: [(
                "IntegrationAccuracy".into(),
                vec![
                    "absolute-tolerance-millionths".into(),
                    "relative-tolerance-millionths".into(),
                    "maximum-estimated-error-millionths".into(),
                ],
            )]
            .into(),
            serde_variant_orders: [(
                "SourceExtractionProfile".into(),
                ["text_utf8", "structured_items", "resource_metadata"]
                    .map(String::from)
                    .into(),
            )]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("AI semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated AI bindings");
}
