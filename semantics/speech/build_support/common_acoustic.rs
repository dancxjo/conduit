//! Integration helper for additive common acoustic Types and fixed Source laws.
//! The Speech build owner installs this explicitly; no whole-Speech generation.
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedNativeType, CheckedSyntaxDocument,
    StartupCatalog,
};
use std::{env, fs, path::PathBuf};
pub fn audio_types() -> Vec<CheckedNativeType> {
    for name in [
        "types.conduit",
        "trajectory_types.conduit",
        "rate_types.conduit",
        "decibel_types.conduit",
    ] {
        println!("cargo:rerun-if-changed=../audio/{name}");
    }
    let source = [
        include_str!("../../audio/types.conduit"),
        include_str!("../../audio/trajectory_types.conduit"),
        include_str!("../../audio/rate_types.conduit"),
        include_str!("../../audio/decibel_types.conduit"),
    ]
    .join("\n");
    check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new())
        .expect("exact Audio owners check")
        .native_types
}
pub fn is_rust_binding(name: &str) -> bool {
    matches!(
        name,
        "AudioSampleProjectionRequest"
            | "AudioResonator"
            | "AudioFrequencyHz"
            | "AudioTrajectoryAnchor"
            | "AudioTrajectoryProvenance"
            | "AudioTrajectoryQuantity"
            | "AudioTrajectoryInterpolation"
            | "AudioTrajectoryOutside"
            | "AudioTrajectoryEndpoints"
            | "AudioTrajectorySegment"
            | "AudioQuantityTrajectory"
            | "AudioTimeFraction"
            | "AudioDecibelConvention"
            | "AudioDecibelBasis"
            | "AudioDecibelFraction"
            | "AudioDecibelValue"
            | "AudioDecibelLevel"
    )
}
pub fn write_programs(semantic: &CheckedSyntaxDocument) {
    let mut programs = String::new();
    for (name, constant) in [
        ("speech/common-audio-target-domain", "AUDIO_DOMAIN"),
        ("speech/common-step-covers", "COVERS"),
        ("speech/common-step-profile", "STEP"),
        ("speech/common-rate-to-period", "RATE_PERIOD"),
        ("speech/common-anchor-equal", "ANCHOR"),
        ("speech/common-duration-to-audio", "DURATION"),
        ("speech/common-cycle-to-audio", "CYCLE"),
        ("speech/common-intensity-to-audio", "INTENSITY"),
        ("speech/common-time-before", "BEFORE"),
        ("speech/common-time-equal", "EQUAL"),
        ("speech/common-tilt-single-octave", "TILT"),
    ] {
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            semantic,
            name,
            &conduit_plot::ProfileCatalog::new(),
        )
        .unwrap_or_else(|e| panic!("expand {name}: {e:?}"));
        let conduit_core::ConfigurationValue::Text(hex) =
            &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!("fixed Source program")
        };
        programs += &format!("pub(crate) const {constant}:&str={hex:?};\n");
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("common_acoustic_programs.rs"),
        programs,
    )
    .expect("retain exact Source programs");
}
