//! Preserve the authored recording disposition Type and exact generated Form.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{fs, path::Path};

pub fn emit(out: &Path) {
    let recording = "recording.conduit";
    println!("cargo:rerun-if-changed={recording}");
    let syntax = parse_syntax_document(&fs::read_to_string(recording).unwrap());
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let bindings = conduit_plot::rust_binding::generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &Default::default(),
    )
    .unwrap();
    fs::write(out.join("recording-forms.rs"), bindings.source).unwrap();
}
