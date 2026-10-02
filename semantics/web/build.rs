use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .expect("resource reference alias is unique");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("web semantic Types must check");
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            public_record_fields: ["HttpRequest".into(), "HttpResponse".into()]
                .into_iter()
                .collect(),
            ..RustBindingOptions::default()
        },
    )
    .expect("web semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated web bindings");
}
