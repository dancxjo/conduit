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
                "ChunkIdentity".into(),
                "FiniteF32".into(),
                "NonnegativeFiniteF32".into(),
            ]
            .into(),
            hash_nominal_types: [
                "ChunkIdentity".into(),
                "FiniteF32".into(),
                "NonnegativeFiniteF32".into(),
            ]
            .into(),
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
            serde_variant_exclusions: [
                "ContextSelectionOutcome".into(),
                "ContextSelectionDisposition".into(),
                "MissingModalityPolicy".into(),
            ]
            .into(),
            serde_record_types: [
                "CompatibleMetrics".into(),
                "ExtractedField".into(),
                "FiniteClassification".into(),
                "GeneratedTextChunk".into(),
                "GeneratedTextFlowEvidence".into(),
                "HouseModelRequest".into(),
                "LlmWorkBounds".into(),
                "WiredHouseContextItem".into(),
                "ValidatedExtraction".into(),
                "ModelWorkAccounting".into(),
                "ProfileReportedConfidence".into(),
                "SourceExtractionLimits".into(),
                "TemporalReference".into(),
            ]
            .into(),
            copy_record_types: [
                "AnswerSpan".into(),
                "CompatibleMetrics".into(),
                "ContextOmission".into(),
                "ContextBudgetCost".into(),
                "IntegrationAccuracy".into(),
                "IntegrationResourceEnvelope".into(),
                "RerankObservation".into(),
                "RetrievalScore".into(),
                "SelectedContextCost".into(),
                "SourceSpan".into(),
                "GeneratedTextFlowEvidence".into(),
                "LlmWorkBounds".into(),
                "ModelWorkAccounting".into(),
                "ProfileReportedConfidence".into(),
                "SourceExtractionLimits".into(),
                "ShadowResourceEnvelope".into(),
                "TrainingResourceEnvelope".into(),
            ]
            .into(),
            copy_record_value_getters: [
                "AnswerSpan".into(),
                "ContextOmission".into(),
                "ContextBudgetCost".into(),
                "IntegrationAccuracy".into(),
                "IntegrationResourceEnvelope".into(),
                "RerankObservation".into(),
                "RetrievalScore".into(),
                "SelectedContextCost".into(),
                "SourceSpan".into(),
                "LlmWorkBounds".into(),
                "ShadowResourceEnvelope".into(),
                "SourceExtractionLimits".into(),
                "TrainingResourceEnvelope".into(),
            ]
            .into(),
            direct_checked_record_constructors: [
                "LlmWorkBounds".into(),
                "SourceExtractionLimits".into(),
            ]
            .into(),
            public_record_fields: [
                "ContextOmission".into(),
                "ExtractedField".into(),
                "FiniteClassification".into(),
                "IntegrationAccuracy".into(),
                "ModelWorkAccounting".into(),
                "SelectedContextCost".into(),
                "TemporalReference".into(),
                "ValidatedExtraction".into(),
            ]
            .into(),
            record_constructor_orders: [
                ("AnswerSpan".into(), vec!["start".into(), "end".into()]),
                (
                    "ContextBudgetCost".into(),
                    vec!["bytes".into(), "tokens".into()],
                ),
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
                    "ContextOmission".into(),
                    vec!["chunk_identity".into(), "reason".into()],
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
                ("RetrievalScore".into(), vec!["value_micros".into()]),
                (
                    "SourceSpan".into(),
                    vec!["unit".into(), "start".into(), "end".into()],
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
                    "IntegrationResourceEnvelope".into(),
                    vec![
                        "maximum_state_bytes".into(),
                        "maximum_context_bytes".into(),
                        "maximum_output_samples".into(),
                        "maximum_output_bytes".into(),
                        "maximum_internal_steps".into(),
                        "maximum_function_evaluations".into(),
                        "maximum_work_units".into(),
                        "memory_ceiling_bytes".into(),
                    ],
                ),
                (
                    "HouseModelRequest".into(),
                    vec![
                        "request_identity".into(),
                        "addressed_utterance".into(),
                        "context".into(),
                        "maximum_output_bytes".into(),
                    ],
                ),
                (
                    "LlmWorkBounds".into(),
                    vec![
                        "maximum_input_bytes".into(),
                        "maximum_context_items".into(),
                        "maximum_output_bytes".into(),
                        "maximum_work_units".into(),
                        "maximum_history_items".into(),
                    ],
                ),
                (
                    "RerankObservation".into(),
                    vec![
                        "chunk_identity".into(),
                        "score_micros".into(),
                        "work_units".into(),
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
                    "LogProbability".into(),
                    vec![
                        "natural_log_millionths".into(),
                        "score_kind".into(),
                        "support_identity".into(),
                        "provenance".into(),
                        "disposition".into(),
                    ],
                ),
                (
                    "ProbabilitySummary".into(),
                    vec![
                        "claim_profile".into(),
                        "result_count".into(),
                        "model_artifact_identity".into(),
                        "query_identity".into(),
                        "randomness".into(),
                        "disposition".into(),
                    ],
                ),
                (
                    "RelationCandidateOutput".into(),
                    vec![
                        "target_variable".into(),
                        "value_identity".into(),
                        "disposition".into(),
                        "sample_count".into(),
                    ],
                ),
                (
                    "RetrieverIdentity".into(),
                    vec!["identity".into(), "mechanism".into()],
                ),
                (
                    "SourceExtractionLimits".into(),
                    vec![
                        "maximum_source_bytes".into(),
                        "maximum_source_items".into(),
                        "maximum_chunk_bytes".into(),
                        "maximum_chunks".into(),
                        "maximum_output_bytes".into(),
                        "maximum_work_units".into(),
                    ],
                ),
                (
                    "SelectedContextCost".into(),
                    vec!["bytes".into(), "tokens".into(), "work_units".into()],
                ),
                (
                    "ShadowResourceEnvelope".into(),
                    vec![
                        "maximum_runs".into(),
                        "maximum_input_bytes".into(),
                        "maximum_output_bytes".into(),
                        "maximum_work_units".into(),
                    ],
                ),
                (
                    "TrainingResourceEnvelope".into(),
                    vec![
                        "model_bytes".into(),
                        "working_memory_bytes".into(),
                        "compute_lanes".into(),
                        "maximum_batch_items".into(),
                        "maximum_batch_bytes".into(),
                        "maximum_steps".into(),
                        "maximum_work_units".into(),
                        "maximum_checkpoint_bytes".into(),
                        "maximum_in_flight_steps".into(),
                    ],
                ),
                (
                    "TrainingObjective".into(),
                    vec![
                        "role".into(),
                        "weight_millionths".into(),
                        "configuration_identity".into(),
                        "output_identity".into(),
                        "participation".into(),
                    ],
                ),
                (
                    "TrainingMetric".into(),
                    vec!["output_identity".into(), "value_millionths".into()],
                ),
                (
                    "StochasticProvenance".into(),
                    vec![
                        "model_artifact_identity".into(),
                        "checkpoint_identity".into(),
                        "query_identity".into(),
                        "randomness".into(),
                        "draws".into(),
                    ],
                ),
                (
                    "SupportedRelationQuery".into(),
                    vec![
                        "evidence_variables".into(),
                        "target_variables".into(),
                        "mode".into(),
                        "result_profile".into(),
                        "maximum_work_units".into(),
                        "maximum_output_bytes".into(),
                    ],
                ),
                (
                    "ValidatedExtraction".into(),
                    vec!["schema_identity".into(), "fields".into()],
                ),
                (
                    "WiredHouseContextItem".into(),
                    vec![
                        "item_identity".into(),
                        "value_kind".into(),
                        "canonical_value".into(),
                        "provenance".into(),
                        "source_identity".into(),
                    ],
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
