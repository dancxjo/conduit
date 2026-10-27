//! Check schemas on the build host; MCU preparation only reads canonical Types.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};
fn main() {
    let path = "../../../plots/device-protocols/pulse-types.conduit";
    println!("cargo:rerun-if-changed={path}");
    let syntax = parse_syntax_document(&fs::read_to_string(path).unwrap());
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let mut bounds = String::new();
    for (name, file) in [
        ("PulseReadRequest", "request.type"),
        ("PulseReadCapture", "capture.type"),
        ("PulseReadResult", "result.type"),
    ] {
        let ty = checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap();
        assert!(
            ty.invariants.is_empty() && ty.value_contracts.is_empty(),
            "pulse ABI acquired additional laws"
        );
        fs::write(out.join(file), ty.value_type.canonical_bytes().unwrap()).unwrap();
        if name != "PulseReadCapture" {
            let maximum =
                conduit_plot::maximum_prepared_canonical_value_bytes(&ty.value_type).unwrap();
            assert!(
                maximum <= 1024,
                "pulse canonical Type exceeds public Host Call ceiling"
            );
            let suffix = if name == "PulseReadRequest" {
                "REQUEST"
            } else {
                "RESULT"
            };
            bounds.push_str(&format!(
                "pub const PULSE_READ_CANONICAL_{suffix}_BYTES: usize = {maximum};\n"
            ));
        }
    }
    fs::write(out.join("canonical_bounds.rs"), bounds).unwrap();
}
