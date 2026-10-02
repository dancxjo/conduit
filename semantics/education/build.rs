use conduit_audio::TimingFeedback;
use conduit_plot::rust_binding::{
    generate_rust_bindings_with_external_bindings, ExternalNativeRustBinding, RustBindingOptions,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let timing_type = TimingFeedback::semantic_type().expect("audio timing feedback Type checks");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_structured_type("TimingFeedback", timing_type.clone())
        .expect("audio timing feedback Type installs once");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("education semantic Types must check");
    let conduit_plot::rust_binding::semantic_core::StructuredInfoTypeShape::Record {
        schema, ..
    } = timing_type.shape()
    else {
        panic!("audio timing feedback Type is a record")
    };
    let timing_identity = schema.as_str().to_owned();
    let generated = generate_rust_bindings_with_external_bindings(
        &checked.native_types,
        &[timing_type],
        &[ExternalNativeRustBinding {
            semantic_identity: &timing_identity,
            rust_type_path: "conduit_audio::TimingFeedback",
        }],
        &RustBindingOptions::default(),
    )
    .expect("education semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated education bindings");
}
