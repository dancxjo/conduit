//! Check the authored protocol Types and Forms once on the build host.
mod build_observation_source;
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};
mod build_display;
#[path = "build/display_frame.rs"]
mod display_frame;
#[path = "build/observation_template.rs"]
mod observation_template;
#[path = "build/sampling_extents.rs"]
mod sampling_extents;
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
    build_observation_source::emit(&out);
    sampling_extents::emit(&checked, &out);
    observation_template::emit(&checked, &out);
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
    fs::write(
        out.join("empty.type"),
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::EMPTY_INFO_ID))
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();
    let observation = &checked
        .native_types
        .iter()
        .find(|ty| ty.name == "Dht11Observation")
        .unwrap()
        .value_type;
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = observation.shape() else {
        panic!("checked observation record");
    };
    for field in fields {
        fs::write(
            out.join(format!("observation-{}.type", field.name())),
            field.value_type().canonical_bytes().unwrap(),
        )
        .unwrap();
    }
    let forms = conduit_plot::rust_binding::generate_rust_bindings_with_forms(
        &[],
        &checked.type_forms,
        &Default::default(),
    )
    .unwrap();
    fs::write(out.join("observation-forms.rs"), forms.source).unwrap();
    build_display::emit(&out);
    let recording = "../profiles/recording.conduit";
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
