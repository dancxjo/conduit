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
    let mut schemas: Vec<(String, Vec<u8>)> = Vec::new();
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
        let bytes = ty.value_type.canonical_bytes().unwrap();
        schemas.push((
            name.strip_prefix("PulseRead").unwrap().to_ascii_uppercase(),
            bytes.clone(),
        ));
        fs::write(out.join(file), bytes).unwrap();
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
                schemas.push(("LENGTH".into(), field("length").canonical_bytes().unwrap()));
                let wire = field("wire");
                emit_bound(&mut bounds, "WIRE", wire, false);
                schemas.push(("WIRE".into(), wire.canonical_bytes().unwrap()));
                let StructuredInfoTypeShape::Collection { element, length: 8 } = wire.shape()
                else {
                    panic!("capture wire")
                };
                emit_bound(&mut bounds, "OCTET", element, false);
                schemas.push(("OCTET".into(), element.canonical_bytes().unwrap()));
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
                schemas.push(("EMPTY".into(), empty.canonical_bytes().unwrap()));
            }
            _ => unreachable!(),
        }
    }
    fs::write(out.join("canonical_bounds.rs"), bounds).unwrap();
    // Longest first lets nested/equal canonical schemas share a single ROM
    // allocation. Bounds and every byte still derive from checked Source.
    schemas.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    let mut blob = Vec::new();
    let mut generated = String::from("static BLOB: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/pulse_schemas.bin\"));\nconst fn span(at: usize, len: usize) -> &'static [u8] { BLOB.split_at(at).1.split_at(len).0 }\n");
    for (name, bytes) in schemas {
        let at = blob
            .windows(bytes.len())
            .position(|candidate| candidate == bytes)
            .unwrap_or_else(|| {
                let at = blob.len();
                blob.extend_from_slice(&bytes);
                at
            });
        generated.push_str(&format!(
            "pub(super) const {name}: &[u8] = span({at}, {});\n",
            bytes.len()
        ));
    }
    fs::write(out.join("pulse_schemas.bin"), blob).unwrap();
    fs::write(out.join("pulse_schemas.rs"), generated).unwrap();
}
