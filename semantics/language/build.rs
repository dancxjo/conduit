use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=identity.conduit");
    println!("cargo:rerun-if-changed=coverage.conduit");
    println!("cargo:rerun-if-changed=syntax.conduit");
    let source = format!(
        "{}\n{}\n{}\n{}",
        include_str!("types.conduit"),
        include_str!("identity.conduit"),
        include_str!("coverage.conduit"),
        include_str!("syntax.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .expect("language semantic Types must check");
    let generated = generate_rust_bindings(&checked.native_types, &RustBindingOptions::default())
        .expect("language semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated language bindings");
    println!("cargo:rerun-if-changed=parser.conduit");
    println!("cargo:rerun-if-changed=parser_beam.conduit");
    println!("cargo:rerun-if-changed=parser_scorer.conduit");
    println!("cargo:rerun-if-changed=parser_mask.conduit");
    let parser_source = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        include_str!("identity.conduit"),
        include_str!("types.conduit"),
        include_str!("parser.conduit"),
        include_str!("parser_beam.conduit"),
        include_str!("parser_scorer.conduit"),
        include_str!("parser_mask.conduit")
    );
    let parser_checked = check_syntax_document(
        &parse_syntax_document(&parser_source),
        &StartupCatalog::new(),
    )
    .expect("parser admission Types check");
    let parser_generated =
        generate_rust_bindings(&parser_checked.native_types, &RustBindingOptions::default())
            .expect("parser admission bindings");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("parser_admission_types.rs"),
        parser_generated.source,
    )
    .expect("write parser admission proof bindings");
}
