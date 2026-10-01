use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("artificial-life semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            copy_record_types: ["ReactionDiffusionRegion".into()].into(),
            copy_record_value_getters: ["ReactionDiffusionRegion".into()].into(),
            record_constructor_orders: [(
                "ReactionDiffusionRegion".into(),
                vec![
                    "region-id".into(),
                    "origin-x".into(),
                    "origin-y".into(),
                    "width".into(),
                    "height".into(),
                ],
            )]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("artificial-life semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated artificial-life bindings");
}
