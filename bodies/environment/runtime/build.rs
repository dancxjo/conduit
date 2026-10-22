//! Check the authored protocol Types and Forms once on the build host.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};
fn main() {
    let path = "../../../plots/device-protocols/dht11.conduit";
    println!("cargo:rerun-if-changed={path}");
    let pulse = "../../../plots/device-protocols/pulse-types.conduit";
    println!("cargo:rerun-if-changed={pulse}");
    let request = "../../../plots/device-protocols/dht11-capture.conduit";
    println!("cargo:rerun-if-changed={request}");
    let source = format!(
        "{}\n{}\n{}",
        fs::read_to_string(pulse).unwrap(),
        fs::read_to_string(path).unwrap(),
        fs::read_to_string(request).unwrap(),
    );
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty());
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let request = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "dht11-capture-request",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(request.gears.len(), 1);
    let conduit_core::ConfigurationValue::Text(encoded) = &request.gears[0].configuration[0].value
    else {
        panic!("checked capture request program");
    };
    fs::write(
        out.join("dht11-capture.program"),
        conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    )
    .unwrap();

    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "dht11-decode",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(expanded.gears.len(), 1);
    fs::write(out.join("dht11-source-identity.rs"), format!(
        "pub const SOURCE_DOCUMENT_ID: &str = {:?};\npub const CHECKED_PLOT_ID: &str = {:?};\npub const EXPANDED_PLOT_ID: &str = {:?};\n",
        expanded.source_document_id.as_str(), expanded.checked_plot_id.as_str(), expanded.expanded_plot_id.as_str(),
    )).unwrap();
    let program_configuration = &expanded.gears[0].configuration[0].value;
    let conduit_core::ConfigurationValue::Text(encoded) = program_configuration else {
        panic!("checked DHT11 expression program");
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    fs::write(
        out.join("dht11-decode.program"),
        program.canonical_bytes().unwrap(),
    )
    .unwrap();

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
    let display = "../../../plots/device-protocols/tm1637.conduit";
    println!("cargo:rerun-if-changed={display}");
    let syntax = parse_syntax_document(&fs::read_to_string(display).unwrap());
    assert!(syntax.diagnostics.is_empty());
    let checked_display = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let display = conduit_plot::expand_canonical_plot_for_authoring(
        &checked_display,
        "tm1637-packets",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(display.gears.len(), 1);
    let conduit_core::ConfigurationValue::Text(encoded) = &display.gears[0].configuration[0].value
    else {
        panic!("checked TM1637 expression program");
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    fs::write(
        out.join("tm1637-packets.program"),
        program.canonical_bytes().unwrap(),
    )
    .unwrap();
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
