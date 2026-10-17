//! Separate native consumers for the exact authored Parsec transition specimen.
use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{fs, path::Path};

pub fn generate(output: &Path) {
    println!("cargo:rerun-if-changed=components/parsec-state-step.conduit");
    println!("cargo:rerun-if-changed=components/parsec-map-transition.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(concat!(
            include_str!("components/parsec-state-step.conduit"),
            "\n",
            include_str!("components/parsec-map-transition.conduit"),
        )),
        &StartupCatalog::new(),
    )
    .expect("Parsec source types check");
    let state = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "ParsecMapState")
        .expect("Parsec map state Type exists");
    assert!(
        state.invariants.is_empty(),
        "input policy checked import must retain newly authored map state where laws"
    );
    let bindings = generate_rust_bindings(&checked.native_types, &RustBindingOptions::default())
        .expect("Parsec source types generate exact native consumers");
    fs::write(output.join("parsec_map_types.rs"), bindings.source)
        .expect("write separate Parsec native consumers");
}
