use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=identity.conduit");
    println!("cargo:rerun-if-changed=syntax.conduit");
    let source = format!(
        "{}\n{}\n{}",
        include_str!("types.conduit"),
        include_str!("identity.conduit"),
        include_str!("syntax.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .expect("language semantic Types must check");
    let generated = generate_rust_bindings(&checked.native_types, &RustBindingOptions::default())
        .expect("language semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated language bindings");
}
