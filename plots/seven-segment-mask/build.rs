use conduit_plot::rust_binding::{generate_rust_bindings_with_forms, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let syntax = parse_syntax_document(include_str!("types.conduit"));
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    // The native const constructors below may bypass only structural checking.
    for name in ["SegmentPattern", "SevenSegmentShow", "SevenSegmentLook"] {
        let ty = checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap();
        assert!(
            ty.invariants.is_empty() && ty.value_contracts.is_empty(),
            "{name} acquired laws requiring checked constructors"
        );
    }
    let options = RustBindingOptions {
        copy_record_types: ["SegmentPattern".into(), "SevenSegmentShow".into()].into(),
        ..Default::default()
    };
    let module =
        generate_rust_bindings_with_forms(&checked.native_types, &checked.type_forms, &options)
            .unwrap();
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("types.rs"),
        module.source,
    )
    .unwrap();
}
