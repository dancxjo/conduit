use conduit_form::rust_binding::{generate_rust_bindings, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("speech semantic Types must check");
    let generated = generate_rust_bindings(
        &checked.native_types,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_exclusions: [
                "RecognitionTextRefusal".into(),
                "SpeechCommitReason".into(),
                "SpeechCommitRefusal".into(),
                "SpeechRecognitionAttempt".into(),
                "SpeechRecognitionDisposition".into(),
                "SpeechRecognitionRefusal".into(),
                "SpeechRecognitionResult".into(),
                "SpeechRecognitionValueError".into(),
                "StreamingRecognitionRefusal".into(),
            ]
            .into(),
            serde_record_types: ["CommittedUserMessage".into(), "SpeakableSegment".into()].into(),
            copy_record_types: ["LiveConversationSpeechRequirements".into()].into(),
            copy_record_value_getters: ["LiveConversationSpeechRequirements".into()].into(),
            copy_nominal_types: ["SpeechOutputCondition".into()].into(),
            serde_nominal_types: ["SpeechDigest".into(), "SpeechSignReason".into()].into(),
            public_record_fields: [
                "CommittedUserMessage".into(),
                "HouseGenerationRequest".into(),
                "SpeakableSegment".into(),
            ]
            .into(),
            record_constructor_orders: BTreeMap::from([
                (
                    "CommittedUserMessage".into(),
                    vec![
                        "turn_identity".into(),
                        "role".into(),
                        "source".into(),
                        "text".into(),
                    ],
                ),
                (
                    "SpeakableSegment".into(),
                    vec![
                        "stream_identity".into(),
                        "sequence".into(),
                        "text".into(),
                        "reason".into(),
                    ],
                ),
            ]),
            ..RustBindingOptions::default()
        },
    )
    .expect("speech semantic Types must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated speech bindings");
}
