use conduit_form::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("time semantic Types must check");
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            derive_serde_for_unit_variants: true,
            ..RustBindingOptions::default()
        },
    )
    .expect("time semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated time bindings");
}
