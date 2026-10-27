//! Check schemas on the build host; MCU preparation only reads canonical Types.
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};
fn emit_bound(bounds: &mut String, suffix: &str, ty: &StructuredInfoType, public: bool) {
    let maximum = conduit_plot::maximum_prepared_canonical_value_bytes(ty).unwrap();
    assert!(
        maximum <= 1024,
        "pulse canonical Type exceeds public Host Call ceiling"
    );
    let visibility = if public { "pub" } else { "pub(crate)" };
    bounds.push_str(&format!(
        "{visibility} const PULSE_READ_CANONICAL_{suffix}_BYTES: usize = {maximum};\n"
    ));
}
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
        match name {
            "PulseReadRequest" => emit_bound(&mut bounds, "REQUEST", &ty.value_type, true),
            "PulseReadCapture" => {
                emit_bound(&mut bounds, "CAPTURE", &ty.value_type, false);
                let StructuredInfoTypeShape::Record { fields, .. } = ty.value_type.shape() else {
                    panic!("capture record")
                };
                let field = |name: &str| {
                    fields
                        .iter()
                        .find(|f| f.name() == name)
                        .unwrap()
                        .value_type()
                };
                emit_bound(&mut bounds, "LENGTH", field("length"), false);
                let wire = field("wire");
                emit_bound(&mut bounds, "WIRE", wire, false);
                let StructuredInfoTypeShape::Collection { element, length: 8 } = wire.shape()
                else {
                    panic!("capture wire")
                };
                emit_bound(&mut bounds, "OCTET", element, false);
            }
            "PulseReadResult" => {
                emit_bound(&mut bounds, "RESULT", &ty.value_type, true);
                let StructuredInfoTypeShape::Variant { cases, .. } = ty.value_type.shape() else {
                    panic!("result variant")
                };
                let empty = cases
                    .iter()
                    .find(|c| c.tag() == "provider-failed")
                    .unwrap()
                    .payload_type();
                emit_bound(&mut bounds, "EMPTY", empty, false);
            }
            _ => unreachable!(),
        }
    }
    fs::write(out.join("canonical_bounds.rs"), bounds).unwrap();
}
