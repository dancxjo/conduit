//! Validate authored finite layouts and retain their exact values for all Hosts.
use conduit_plot::rust_binding::semantic_core::{
    StructuredInfoValue, StructuredInfoValueShape as Shape,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalStartupValue, StartupCatalog,
};
use std::{env, fmt::Write, fs, path::PathBuf};

pub fn generate() {
    println!("cargo:rerun-if-changed=keymap-layouts.conduit");
    let source = include_str!("keymap-layouts.conduit");
    let syntax = parse_syntax_document(source);
    assert!(
        syntax.diagnostics.is_empty(),
        "keymap Source syntax: {:?}",
        syntax.diagnostics
    );
    let checked = check_syntax_document(&syntax, &StartupCatalog::new())
        .expect("authored keymap layouts must check as finite typed values");
    let mut generated =
        String::from("// Generated from checked keymap-layouts.conduit; do not edit.\n");
    let descriptors = checked
        .plots
        .iter()
        .find(|plot| plot.name == "input/keymap-presets")
        .expect("authored preset registry");
    let Some(CanonicalStartupValue::Structured(descriptors)) =
        &descriptors.startup_parameters[0].default
    else {
        panic!("typed preset descriptors")
    };
    let descriptors = descriptors
        .try_concrete()
        .expect("concrete descriptor registry");
    let mut choices = Vec::new();
    let mut prepared_layouts = Vec::new();
    for descriptor in collection(&descriptors) {
        let plot_name = text(field(descriptor, "definition"));
        let prefix = plot_name
            .strip_prefix("input/keymap-")
            .expect("source layout namespace")
            .to_ascii_uppercase()
            .replace('-', "_");
        let plot = checked
            .plots
            .iter()
            .find(|plot| plot.name == plot_name)
            .expect("layout plot");
        let Some(CanonicalStartupValue::Structured(value)) = &plot.startup_parameters[0].default
        else {
            panic!("{plot_name} must declare a structured layout default")
        };
        let concrete = value
            .try_concrete()
            .expect("layout defaults have no unresolved parameters");
        emit_static_default(&mut generated, &concrete, &prefix);
        prepared_layouts.push(format!(
            "({prefix}_KEYMAP_BYTES, {prefix}_KEYMAP_BINDINGS, {prefix}_KEYMAP_COMPOSITIONS)"
        ));
        let bytes = concrete
            .canonical_bytes()
            .expect("layout fits finite canonical encoding");
        let authored = syntax
            .plots
            .iter()
            .find(|plot| plot.name.text == plot_name)
            .expect("authored layout plot")
            .front
            .startup_parameters[0]
            .default
            .as_ref()
            .expect("authored default")
            .text
            .as_str();
        writeln!(
            generated,
            "pub const {prefix}_KEYMAP_BYTES: &[u8] = &{bytes:?};"
        )
        .unwrap();
        writeln!(
            generated,
            "pub const {prefix}_KEYMAP_LITERAL: &str = {authored:?};"
        )
        .unwrap();
        choices.push(format!(
            "{{identity:{:?},label:{:?},layout:{authored}}}",
            text(field(descriptor, "identity")),
            text(field(descriptor, "label"))
        ));
    }
    writeln!(
        generated,
        "const KEYMAP_PREPARED_LAYOUTS: &[(&[u8], &[Binding], &[Composition])] = &[{}];",
        prepared_layouts.join(",")
    )
    .unwrap();
    let choices_literal = format!("[{}]", choices.join(","));
    let choices_source = format!("{source}\nplot input/keymap-choices (\n choices: KeymapChoices = {choices_literal}\n) {{\n}}\n");
    let choices_checked = check_syntax_document(
        &parse_syntax_document(&choices_source),
        &StartupCatalog::new(),
    )
    .expect("source-linked typed layout registry");
    let choice_plot = choices_checked
        .plots
        .iter()
        .find(|plot| plot.name == "input/keymap-choices")
        .unwrap();
    let Some(CanonicalStartupValue::Structured(value)) = &choice_plot.startup_parameters[0].default
    else {
        panic!("typed layout choices")
    };
    let bytes = value
        .try_concrete()
        .expect("concrete choices")
        .canonical_bytes()
        .expect("bounded choices");
    writeln!(
        generated,
        "pub const DEFAULT_KEYMAP_CHOICES_LITERAL: &str = {choices_literal:?};"
    )
    .unwrap();
    writeln!(
        generated,
        "pub const DEFAULT_KEYMAP_CHOICES_BYTES: &[u8] = &{bytes:?};"
    )
    .unwrap();
    writeln!(
        generated,
        "pub const KEYMAP_CHOICES_SOURCE: &str = {source:?};"
    )
    .unwrap();
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    fs::write(output.join("keymap_layouts.rs"), generated).expect("write checked keymap layouts");
}

fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let Shape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .expect("field")
        .value()
}
fn collection(value: &StructuredInfoValue) -> &[StructuredInfoValue] {
    let Shape::Collection(values) = value.shape() else {
        panic!("sequence")
    };
    values
}
fn leaf(value: &StructuredInfoValue) -> &[u8] {
    let Shape::Leaf(bytes) = value.shape() else {
        panic!("leaf")
    };
    bytes
}
fn integer(value: &StructuredInfoValue) -> u32 {
    let bytes = leaf(value);
    let mut word = [0; 4];
    word[..bytes.len()].copy_from_slice(bytes);
    u32::from_le_bytes(word)
}
fn character(value: &StructuredInfoValue) -> char {
    char::from_u32(integer(value)).expect("Unicode scalar")
}
fn text(value: &StructuredInfoValue) -> &str {
    std::str::from_utf8(leaf(value)).expect("authored UTF-8")
}
fn text_literal(value: &StructuredInfoValue) -> String {
    let bytes = leaf(value);
    std::str::from_utf8(bytes).expect("UTF-8 text");
    let mut buffer = [0u8; 8];
    buffer[..bytes.len()].copy_from_slice(bytes);
    format!("Text {{ bytes: {buffer:?}, len: {} }}", bytes.len())
}
fn emit_static_default(output: &mut String, value: &StructuredInfoValue, prefix: &str) {
    writeln!(output, "const {prefix}_KEYMAP_BINDINGS: &[Binding] = &[").unwrap();
    for layer in collection(field(value, "layers")) {
        for binding in collection(layer) {
            let usage = integer(field(binding, "usage"));
            let shift = leaf(field(binding, "shift")) == [1];
            let altgr = leaf(field(binding, "altgr")) == [1];
            let Shape::Variant { tag, payload } = field(binding, "action").shape() else {
                panic!("action")
            };
            let action = match tag {
                "none" => "Action::None".to_owned(),
                "text" => format!("Action::Text({})", text_literal(payload)),
                "caps_lock" => "Action::CapsLock".to_owned(),
                "num_lock" => "Action::NumLock".to_owned(),
                "keypad" => format!("Action::Keypad({})", text_literal(payload)),
                "caps_text" => format!(
                    "Action::CapsText {{ ordinary: {}, locked: {} }}",
                    text_literal(field(payload, "ordinary")),
                    text_literal(field(payload, "locked"))
                ),
                "dead" => format!(
                    "Action::Dead {{ spacing: {}, combining: {:?} }}",
                    text_literal(field(payload, "spacing")),
                    character(field(payload, "combining"))
                ),
                _ => panic!("unknown action"),
            };
            writeln!(
                output,
                "Binding {{ usage: {usage}, shift: {shift}, altgr: {altgr}, action: {action} }},"
            )
            .unwrap();
        }
    }
    writeln!(
        output,
        "];\nconst {prefix}_KEYMAP_COMPOSITIONS: &[Composition] = &["
    )
    .unwrap();
    for composition in collection(field(value, "compositions")) {
        writeln!(
            output,
            "Composition {{ combining: {:?}, base: {:?}, result: {:?} }},",
            character(field(composition, "combining")),
            character(field(composition, "base")),
            character(field(composition, "result"))
        )
        .unwrap();
    }
    output.push_str("];\n");
}
