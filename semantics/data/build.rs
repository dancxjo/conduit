use conduit_form::rust_binding::{generate_rust_bindings_with_representations, RustBindingOptions};
use conduit_form::{
    check_syntax_document, generate_ecmascript_representations, parse_syntax_document,
    StartupCatalog,
};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("data semantic Types must check");
    let generated = generate_rust_bindings_with_representations(
        &checked.native_types,
        &checked.representations,
        &RustBindingOptions::default(),
    )
    .expect("data semantic Types and representations must generate exact Rust bindings");
    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let output = output_directory.join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated data bindings");
    let ecmascript = output_directory.join("representations.mjs");
    fs::write(
        &ecmascript,
        generate_ecmascript_representations(&checked.representations),
    )
    .expect("write generated ECMAScript representation bindings");
    println!(
        "cargo:rustc-env=CONDUIT_DATA_REPRESENTATIONS_MJS={}",
        ecmascript.display()
    );
}
