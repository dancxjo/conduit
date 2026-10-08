//! Fixed Source programs for the bounded authored gesture profile.
use conduit_plot::CheckedSyntaxDocument;
use std::{env, fs, path::PathBuf};
pub fn write_programs(source: &CheckedSyntaxDocument) {
    let mut programs = String::new();
    for (name, constant) in [
        ("speech/gesture-symbol-profile", "SYMBOL"),
        ("speech/gesture-fifths", "FIFTHS"),
        ("speech/gesture-role-active", "ROLE"),
        ("speech/gesture-formant-profile", "FORMANTS"),
        ("speech/gesture-choice-supported", "CHOICE"),
        ("speech/gesture-role-window", "WINDOW"),
        ("speech/gesture-conflict", "CONFLICT"),
    ] {
        let e = conduit_plot::expand_canonical_plot_for_authoring(
            source,
            name,
            &conduit_plot::ProfileCatalog::new(),
        )
        .unwrap();
        let conduit_core::ConfigurationValue::Text(hex) =
            &e.expanded.gears[0].configuration[0].value
        else {
            panic!("fixed gesture program")
        };
        programs += &format!("pub(crate) const {constant}:&str={hex:?};\n");
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("gesture_programs.rs"),
        programs,
    )
    .unwrap();
}
