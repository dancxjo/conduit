//! Fixed Source programs for the separate authored greeting profile.
use conduit_plot::CheckedSyntaxDocument;
use std::{env, fs, path::PathBuf};
pub fn write_programs(source: &CheckedSyntaxDocument) {
    let mut programs = String::new();
    for (name, constant) in [
        ("speech/greeting-symbol-v2", "SYMBOL"),
        ("speech/greeting-legacy-effect-v2", "LEGACY_EFFECT"),
        ("speech/greeting-effect-v2", "EFFECT"),
        ("speech/greeting-role-v2", "ROLE"),
        ("speech/greeting-role-split-v2", "SPLIT"),
        ("speech/greeting-window-v2", "WINDOW"),
        ("speech/greeting-formants-v2", "FORMANTS"),
        ("speech/greeting-target-second-v2", "SECOND"),
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
            panic!("fixed greeting program")
        };
        programs += &format!("pub(crate) const {constant}:&str={hex:?};\n");
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("greeting_programs.rs"),
        programs,
    )
    .unwrap();
}
