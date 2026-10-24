//! Compile ordinary checked capture and decoding Source on the build host.
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{fs, path::Path};
pub fn emit(out: &Path) {
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
}
