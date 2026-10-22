//! Check the authored protocol Types and Forms once on the build host.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};
fn main() {
    let path = "../../../plots/device-protocols/dht11.conduit";
    println!("cargo:rerun-if-changed={path}");
    let pulse = "../../../plots/device-protocols/pulse-types.conduit";
    println!("cargo:rerun-if-changed={pulse}");
    let source = format!(
        "{}\n{}",
        fs::read_to_string(pulse).unwrap(),
        fs::read_to_string(path).unwrap()
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (name, file) in [
        ("PulseReadCapture", "pulse-capture.type"),
        ("Dht11DecodedResult", "decoded.type"),
        ("Dht11Observation", "observation.type"),
        ("Dht11Result", "observation-result.type"),
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|t| t.name == name)
            .unwrap();
        assert!(ty.invariants.is_empty() && ty.value_contracts.is_empty());
        fs::write(out.join(file), ty.value_type.canonical_bytes().unwrap()).unwrap();
    }
    let forms = conduit_plot::rust_binding::generate_rust_bindings_with_forms(
        &[],
        &checked.type_forms,
        &Default::default(),
    )
    .unwrap();
    fs::write(out.join("observation-forms.rs"), forms.source).unwrap();
}
