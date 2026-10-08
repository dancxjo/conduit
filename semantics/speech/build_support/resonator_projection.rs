//! Fixed Source programs for the bounded Q20/Q14 resonator profile.
use conduit_plot::CheckedSyntaxDocument;
use std::{env, fs, path::PathBuf};
pub fn write_programs(source: &CheckedSyntaxDocument) {
    let mut programs = String::new();
    for (name, constant) in [
        ("speech/resonator-angles-q20", "ANGLES"),
        ("speech/gesture-frame-gates", "FRAME_GATES"),
        ("speech/gesture-dsp-profile", "DSP_PROFILE"),
        ("speech/resonator-polynomial-seed", "SEED"),
        ("speech/resonator-polynomial-step", "STEP"),
        ("speech/resonator-combine-q20", "COMBINE"),
        ("speech/resonator-round-q14", "ROUND"),
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
            panic!("fixed resonator program")
        };
        programs += &format!("pub(crate) const {constant}:&str={hex:?};\n");
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("resonator_programs.rs"),
        programs,
    )
    .unwrap();
}
