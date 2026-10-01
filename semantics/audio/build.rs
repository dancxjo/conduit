use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("audio semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            copy_record_types: [
                "AudioRenderDemand".into(),
                "MusicalPitch".into(),
                "MusicalNoteEvent".into(),
                "PcmClipProfile".into(),
                "PcmFrameHeader".into(),
                "ToneIntent".into(),
            ]
            .into(),
            copy_record_value_getters: [
                "AudioRenderDemand".into(),
                "MusicalNoteEvent".into(),
                "MusicalPitch".into(),
                "PcmFrameHeader".into(),
                "ToneIntent".into(),
            ]
            .into(),
            public_record_fields: ["AudioRenderDemand".into(), "PcmFrameHeader".into()].into(),
            direct_checked_record_constructors: [
                "MusicalControlEvent".into(),
                "MusicalPitch".into(),
                "MusicalNoteEvent".into(),
                "ToneIntent".into(),
            ]
            .into(),
            record_constructor_orders: [
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
    .expect("audio semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated audio bindings");
}
