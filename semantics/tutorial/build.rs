use conduit_plot::rust_binding::{RustBindingOptions, generate_rust_bindings};
use conduit_plot::{StartupCatalog, check_syntax_document, parse_syntax_document};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("Tutorial semantic Types must check");
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            ..RustBindingOptions::default()
        },
    )
    .expect("Tutorial semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated Tutorial bindings");
}
