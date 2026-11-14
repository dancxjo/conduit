//! Check schemas on the build host; MCU preparation only reads canonical Types.
use conduit_plot::rust_binding::semantic_core::StructuredInfoTypeShape;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};
fn main() {
    let path = "../../../plots/device-protocols/pulse-types.conduit";
    println!("cargo:rerun-if-changed={path}");
    let syntax = parse_syntax_document(&fs::read_to_string(path).unwrap());
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
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
    }
    let capture = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "PulseReadCapture")
        .unwrap()
        .value_type;
    let StructuredInfoTypeShape::Record { fields, .. } = capture.shape() else {
        panic!("pulse capture record");
    };
    for (name, file) in [("length", "length.type"), ("wire", "wire.type")] {
        let field = fields.iter().find(|field| field.name() == name).unwrap();
        fs::write(
            out.join(file),
            field.value_type().canonical_bytes().unwrap(),
        )
        .unwrap();
    }
    let result = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "PulseReadResult")
        .unwrap()
        .value_type;
    let StructuredInfoTypeShape::Variant { cases, .. } = result.shape() else {
        panic!("pulse result variant");
    };
    let empty = cases
        .iter()
        .find(|case| case.tag() == "provider-failed")
        .unwrap()
        .payload_type();
    fs::write(out.join("empty.type"), empty.canonical_bytes().unwrap()).unwrap();
}
