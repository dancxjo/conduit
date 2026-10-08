//! Fixed Source programs for the separate authored greeting profile.
use conduit_plot::CheckedSyntaxDocument;
use std::{env, fs, path::PathBuf};
pub fn write_programs(source: &CheckedSyntaxDocument) {
    let mut programs = String::new();
    for (name, constant) in [
        ("speech/greeting-symbol-v2", "SYMBOL"),
        ("speech/gesture-pitch-grid-v1", "PITCH_GRID"),
        ("speech/word-pitch-point-input-v1", "WORD_GRID"),
        ("speech/word-pitch-partition-end-v1", "WORD_END"),
        ("speech/pitch-cycle-fraction", "PITCH_FRACTION"),
        ("speech/gesture-pitch-cycle-q8-v1", "PITCH_Q8"),
        ("speech/greeting-reviewed-features-v1", "FEATURES"),
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
        if ["PITCH_GRID", "PITCH_FRACTION", "PITCH_Q8"].contains(&constant) {
            let bytes = conduit_plot::PortableExpressionProgram::from_canonical_hex(hex)
                .unwrap()
                .canonical_bytes()
                .unwrap();
            programs += &format!("pub(crate) const {constant}_BYTES:&[u8]=&{bytes:?};\n");
        }
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("greeting_programs.rs"),
        programs,
    )
    .unwrap();
}
