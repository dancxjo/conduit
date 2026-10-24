//! Compile the reviewed packet expression once; no Source parser runs in Play.
use std::{fs, path::Path};
pub fn emit(out: &Path) {
    let path = "../../../plots/device-protocols/tm1637.conduit";
    println!("cargo:rerun-if-changed={path}");
    let syntax = conduit_plot::parse_syntax_document(&fs::read_to_string(path).unwrap());
    assert!(syntax.diagnostics.is_empty());
    let checked =
        conduit_plot::check_syntax_document(&syntax, &conduit_plot::StartupCatalog::new()).unwrap();
    let expanded = conduit_plot::expand_canonical_plot_for_authoring(
        &checked,
        "tm1637-packets",
        &conduit_plot::ProfileCatalog::new(),
    )
    .unwrap()
    .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let conduit_core::ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value
    else {
        panic!("checked TM1637 expression program");
    };
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    fs::write(
        out.join("tm1637-packets.program"),
        program.canonical_bytes().unwrap(),
    )
    .unwrap();
}
