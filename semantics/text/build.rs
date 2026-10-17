use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=pattern-types.conduit");
    write_pattern_bindings();
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("text semantic Types must check");
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            record_constructor_names: [("AddressSet".into(), "new_native".into())].into(),
            public_record_fields: ["MorsePattern".into()].into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("text semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated text bindings");
}

fn write_pattern_bindings() {
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("pattern-types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("portable pattern specification Type must check");
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            record_constructor_names: [(
                "PortablePatternSpecification".into(),
                "new_native".into(),
            )]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("portable pattern specification binding must generate");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("pattern_types.rs");
    fs::write(output, generated.source).expect("write generated pattern binding");
}
