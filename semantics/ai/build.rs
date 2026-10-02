use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &StartupCatalog::new(),
    )
    .expect("AI semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            boxed_variant_payloads: ["TrainingLifecyclePhase.active_step".into()].into(),
            derive_serde_for_variants: true,
            copy_nominal_types: [
                "ConfidencePermille".into(),
                "FiniteF32".into(),
                "NonnegativeFiniteF32".into(),
            ]
            .into(),
            hash_nominal_types: ["FiniteF32".into(), "NonnegativeFiniteF32".into()].into(),
            serde_nominal_types: [
                "ClassificationLabel".into(),
                "ClassificationLabels".into(),
                "ClockIdentity".into(),
                "ConfidencePermille".into(),
                "ExtractionFields".into(),
                "ExtractionKey".into(),
                "ExtractionValue".into(),
                "FiniteF32".into(),
                "NonnegativeFiniteF32".into(),
            ]
            .into(),
            serde_variant_exclusions: ["MissingModalityPolicy".into()].into(),
            serde_record_types: [
                "CompatibleMetrics".into(),
                "ExtractedField".into(),
                "FiniteClassification".into(),
                "GeneratedTextChunk".into(),
                "GeneratedTextFlowEvidence".into(),
                "ValidatedExtraction".into(),
                "ModelWorkAccounting".into(),
                "ProfileReportedConfidence".into(),
                "TemporalReference".into(),
            ]
            .into(),
            copy_record_types: [
                "CompatibleMetrics".into(),
                "IntegrationAccuracy".into(),
                "GeneratedTextFlowEvidence".into(),
                "ModelWorkAccounting".into(),
                "ProfileReportedConfidence".into(),
            ]
            .into(),
            copy_record_value_getters: ["IntegrationAccuracy".into()].into(),
            public_record_fields: [
                "ExtractedField".into(),
                "FiniteClassification".into(),
                "IntegrationAccuracy".into(),
                "ModelWorkAccounting".into(),
                "TemporalReference".into(),
                "ValidatedExtraction".into(),
            ]
            .into(),
            record_constructor_orders: [
                (
                    "ContextSelectionPolicy".into(),
                    vec![
                        "identity".into(),
                        "token_accounting_profile".into(),
                        "redundancy".into(),
                        "ordering".into(),
                        "maximum_items".into(),
                        "maximum_bytes".into(),
                        "maximum_tokens".into(),
                        "maximum_work_units".into(),
                    ],
                ),
                (
                    "FiniteClassification".into(),
                    vec!["label".into(), "allowed_labels".into()],
                ),
                (
                    "GroundedAnswerPolicy".into(),
                    vec![
                        "identity".into(),
                        "answer_kind".into(),
                        "maximum_output_bytes".into(),
                        "maximum_claims".into(),
                        "maximum_citations".into(),
                        "maximum_work_units".into(),
                    ],
                ),
                (
                    "GeneratedTextChunk".into(),
                    vec!["sequence".into(), "text".into()],
                ),
                (
                    "GeneratedTextFlowEvidence".into(),
                    vec![
                        "chunks".into(),
                        "generated_bytes".into(),
                        "terminal".into(),
                        "retained_private_text".into(),
                    ],
                ),
                (
                    "IntegrationAccuracy".into(),
                    vec![
                        "absolute-tolerance-millionths".into(),
                        "relative-tolerance-millionths".into(),
                        "maximum-estimated-error-millionths".into(),
                    ],
                ),
                (
                    "RerankingPolicy".into(),
                    vec![
                        "identity".into(),
                        "strategy".into(),
                        "maximum_candidates".into(),
                        "maximum_work_units".into(),
                    ],
                ),
                (
                    "RetrieverIdentity".into(),
                    vec!["identity".into(), "mechanism".into()],
                ),
                (
                    "ValidatedExtraction".into(),
                    vec!["schema_identity".into(), "fields".into()],
                ),
            ]
            .into(),
            serde_variant_orders: [
                (
                    "ClockBasis".into(),
                    ["unix_epoch_milliseconds", "monotonic_milliseconds"]
                        .map(String::from)
                        .into(),
                ),
                (
                    "SourceExtractionProfile".into(),
                    ["text_utf8", "structured_items", "resource_metadata"]
                        .map(String::from)
                        .into(),
                ),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("AI semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated AI bindings");
}
