use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_plot::rust_binding::semantic_core::kind_id(
                conduit_plot::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("process semantic Types must check");
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            ..RustBindingOptions::default()
        },
    )
    .expect("process semantic Types must generate exact Rust bindings");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
            .join("semantic_types.rs"),
        generated.source,
    )
    .expect("write generated process bindings");
}
