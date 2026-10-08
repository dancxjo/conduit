use conduit_plot::rust_binding::{generate_rust_bindings_with_forms, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    println!("cargo:rerun-if-changed=trajectory_types.conduit");
    println!("cargo:rerun-if-changed=rate_types.conduit");
    println!("cargo:rerun-if-changed=rate_projection.conduit");
    println!("cargo:rerun-if-changed=trajectory.conduit");
    let type_source = format!(
        "{}\n{}\n{}",
        include_str!("types.conduit"),
        include_str!("trajectory_types.conduit"),
        include_str!("rate_types.conduit")
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&type_source), &StartupCatalog::new())
            .expect("audio semantic Types must check");
    println!("cargo:rerun-if-changed=acoustic_quantities.conduit");
    let acoustic_source = format!(
        "{}\n{}\n{}\n{}",
        type_source,
        include_str!("acoustic_quantities.conduit"),
        include_str!("trajectory.conduit"),
        include_str!("rate_projection.conduit")
    );
    let acoustic = check_syntax_document(
        &parse_syntax_document(&acoustic_source),
        &StartupCatalog::new(),
    )
    .expect("acoustic Source checks");
    let mut programs = String::new();
    for (name, constant) in [
        ("audio/frequency-to-cycle", "FREQUENCY_TO_CYCLE"),
        ("audio/cycle-to-frequency", "CYCLE_TO_FREQUENCY"),
        ("audio/amplitude-to-power", "AMPLITUDE_TO_POWER"),
        ("audio/trajectory-ratio", "TRAJECTORY_RATIO"),
        ("audio/trajectory-anchor-equal", "TRAJECTORY_ANCHOR"),
        ("audio/trajectory-segment-valid", "TRAJECTORY_VALID"),
        ("audio/trajectory-order-valid", "TRAJECTORY_ORDER"),
        ("audio/trajectory-domain-equal", "TRAJECTORY_DOMAIN"),
        ("audio/trajectory-covers", "TRAJECTORY_COVERS"),
        ("audio/trajectory-weight", "TRAJECTORY_WEIGHT"),
        ("audio/trajectory-blend", "TRAJECTORY_BLEND"),
        ("audio/sample-fraction", "SAMPLE_FRACTION"),
        ("audio/sample-at-rate", "SAMPLE_AT_RATE"),
        ("audio/frame-grid-fidelity", "FRAME_GRID_FIDELITY"),
        ("audio/cumulative-basis-equal", "CUMULATIVE_BASIS"),
        ("audio/cumulative-append", "CUMULATIVE_APPEND"),
        ("audio/cumulative-origin", "CUMULATIVE_ORIGIN"),
    ] {
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            &acoustic,
            name,
            &conduit_plot::ProfileCatalog::new(),
        )
        .expect("expand acoustic Source");
        let conduit_core::ConfigurationValue::Text(encoded) =
            &expanded.expanded.gears[0].configuration[0].value
        else {
            panic!("acoustic program")
        };
        programs.push_str(&format!(
            "pub(crate) const {constant}: &str = {encoded:?};\n"
        ));
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("acoustic_programs.rs"),
        programs,
    )
    .expect("write acoustic programs");
    let generated = generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            copy_record_types: [
                "AudioRelativeAmplitude".into(),
                "AudioPowerRatio".into(),
                "AudioAmplitudePowerSquared".into(),
                "AudioAmplitudePowerRequest".into(),
                "AudioAmplitudePowerEligible".into(),
                "AudioFrequencyHz".into(),
                "AudioCycleDuration".into(),
                "AudioResonator".into(),
                "AudioRenderDemand".into(),
                "BeatReference".into(),
                "MusicalPitch".into(),
                "MusicalNoteEvent".into(),
                "PcmClipProfile".into(),
                "PcmCompatibilityProfile".into(),
                "PcmFrameHeader".into(),
                "ToneIntent".into(),
                "TimingFeedback".into(),
            ]
            .into(),
            copy_record_value_getters: [
                "AudioRenderDemand".into(),
                "BeatReference".into(),
                "MusicalNoteEvent".into(),
                "MusicalPitch".into(),
                "PcmFrameHeader".into(),
                "ToneIntent".into(),
                "TimingFeedback".into(),
            ]
            .into(),
            public_record_fields: [
                "AudioRenderDemand".into(),
                "PcmCompatibilityProfile".into(),
                "PcmFrameHeader".into(),
            ]
            .into(),
            serde_record_types: ["PcmCompatibilityProfile".into()].into(),
            direct_checked_record_constructors: [
                "AudioRelativeAmplitude".into(),
                "AudioPowerRatio".into(),
                "AudioAmplitudePowerSquared".into(),
                "AudioAmplitudePowerRequest".into(),
                "AudioFrequencyHz".into(),
                "AudioCycleDuration".into(),
                "AudioResonator".into(),
                "MusicalControlEvent".into(),
                "MusicalPitch".into(),
                "MusicalNoteEvent".into(),
                "ToneIntent".into(),
                "BeatReference".into(),
                "TimingFeedback".into(),
            ]
            .into(),
            record_constructor_orders: [
                (
                    "BeatReference".into(),
                    vec!["beat".into(), "expected_time_micros".into()],
                ),
                (
                    "AudioRenderDemand".into(),
                    vec![
                        "clock-id".into(),
                        "start-frame".into(),
                        "frame-count".into(),
                        "sequence".into(),
                    ],
                ),
                (
                    "TimingFeedback".into(),
                    vec![
                        "beat".into(),
                        "classification".into(),
                        "delta_micros".into(),
                        "expected_time_micros".into(),
                        "observed".into(),
                        "observed_time_micros".into(),
                        "recovery_state".into(),
                    ],
                ),
                (
                    "MusicalNoteEvent".into(),
                    vec![
                        "occurrence".into(),
                        "pitch".into(),
                        "gate".into(),
                        "velocity".into(),
                        "event-time-micros".into(),
                        "order".into(),
                    ],
                ),
                (
                    "MusicalPitch".into(),
                    vec![
                        "frequency-millihertz".into(),
                        "a4-reference-millihertz".into(),
                        "detune-microcents".into(),
                    ],
                ),
                (
                    "PcmFrameHeader".into(),
                    vec![
                        "representation".into(),
                        "sample-rate-hz".into(),
                        "layout".into(),
                        "frame-count".into(),
                        "clock-id".into(),
                        "start-frame".into(),
                        "discontinuity".into(),
                        "payload-bytes".into(),
                    ],
                ),
                (
                    "ToneIntent".into(),
                    vec![
                        "correlation".into(),
                        "pitch".into(),
                        "gate".into(),
                        "event-time-micros".into(),
                        "order".into(),
                    ],
                ),
            ]
            .into(),
            record_constructor_names: [
                ("AudioRenderDemand".into(), "new_native".into()),
                ("PcmFrameHeader".into(), "new_native".into()),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("audio semantic Types and Forms must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated audio bindings");
}
