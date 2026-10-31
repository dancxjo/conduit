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
    fs::write(
        out.join("tm1637-frame.type"),
        program.input_type.canonical_bytes().unwrap(),
    )
    .unwrap();
    let canonical = program.canonical_bytes().unwrap();
    let material = conduit_plot::MakeExpressionMaterial::export(&program).unwrap();
    fs::write(out.join("tm1637-expression.make"), material.bytes()).unwrap();
    fs::write(out.join("tm1637-expression-material.rs"),format!(
        "pub const PROGRAM_DIGEST:[u8;32]={:?};\npub const MATERIAL_DIGEST:[u8;32]={:?};\npub const BOUNDS:(u32,u32)=({},{});\n",
        conduit_core::assigned_plan_payload_digest(&canonical),conduit_core::assigned_plan_payload_digest(material.bytes()),program.maximum_prepared_input_bytes().unwrap(),program.maximum_prepared_output_bytes().unwrap())).unwrap();
    let conduit_core::StructuredInfoTypeShape::Variant { cases, .. } = program.output_type.shape()
    else {
        panic!("TM1637 packet variant")
    };
    let packets = cases
        .iter()
        .find(|c| c.tag() == "packets")
        .unwrap()
        .payload_type();
    fs::write(
        out.join("tm1637-packet-output.type"),
        packets.canonical_bytes().unwrap(),
    )
    .unwrap();
    crate::display_frame::emit(&program.input_type, out);
}
