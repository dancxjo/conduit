use conduit_plot::rust_binding::{generate_rust_bindings_with_forms, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("audio semantic Types must check");
    let generated = generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            copy_record_types: [
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
