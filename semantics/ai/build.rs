use conduit_form::rust_binding::{
    generate_rust_bindings_with_codes_and_external_bindings, ExternalNativeRustBinding,
    RustBindingOptions,
};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let tensor_value =
        conduit_data::TensorValue::semantic_type().expect("TensorValue semantic Type checks");
    let sampled_signal =
        conduit_data::SampledSignal::semantic_type().expect("SampledSignal semantic Type checks");
    let tensor_element =
        conduit_data::TensorElement::semantic_type().expect("TensorElement semantic Type checks");
    let tensor_axis_role =
        conduit_data::TensorAxisRole::semantic_type().expect("TensorAxisRole semantic Type checks");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_form::rust_binding::semantic_core::kind_id(
                conduit_form::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    catalog
        .insert_structured_type("TensorValue", tensor_value.clone())
        .expect("TensorValue installs once");
    catalog
        .insert_structured_type("SampledSignal", sampled_signal.clone())
        .expect("SampledSignal installs once");
    catalog
        .insert_structured_type("TensorElement", tensor_element.clone())
        .expect("TensorElement installs once");
    catalog
        .insert_structured_type("TensorAxisRole", tensor_axis_role.clone())
        .expect("TensorAxisRole installs once");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("AI semantic Types must check");
    let external_types = [
        tensor_value,
        sampled_signal,
        tensor_element,
        tensor_axis_role,
    ];
    let external_identities =
        external_types
            .each_ref()
            .map(|value_type| match value_type.shape() {
                conduit_form::rust_binding::semantic_core::StructuredInfoTypeShape::Nominal {
                    schema,
                    ..
                }
                | conduit_form::rust_binding::semantic_core::StructuredInfoTypeShape::Record {
                    schema,
                    ..
                }
                | conduit_form::rust_binding::semantic_core::StructuredInfoTypeShape::Variant {
                    schema,
                    ..
                } => schema.as_str().to_owned(),
                _ => panic!("external native Type has a named identity"),
            });
    let generated = generate_rust_bindings_with_codes_and_external_bindings(
        &checked.native_types,
        &checked.codes,
        &external_types,
        &[
            ExternalNativeRustBinding {
                semantic_identity: &external_identities[0],
                rust_type_path: "conduit_data::TensorValue",
            },
            ExternalNativeRustBinding {
                semantic_identity: &external_identities[1],
                rust_type_path: "conduit_data::SampledSignal",
            },
            ExternalNativeRustBinding {
                semantic_identity: &external_identities[2],
                rust_type_path: "conduit_data::TensorElement",
            },
            ExternalNativeRustBinding {
                semantic_identity: &external_identities[3],
                rust_type_path: "conduit_data::TensorAxisRole",
            },
        ],
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
                "EmbeddingProfileIdentity".into(),
                "VectorMetadataEntries".into(),
                "MetadataFilters".into(),
                "FiniteF32".into(),
                "NonnegativeFiniteF32".into(),
                "TemporalEvidenceCandidates".into(),
                "TemporalEvidenceIdentities".into(),
                "TemporalEvidenceIdentity".into(),
            ]
            .into(),
            serde_variant_exclusions: [
                "ContextSelectionOutcome".into(),
                "ContextSelectionDisposition".into(),
                "GroundingInputAssessment".into(),
                "GroundedClaimSupport".into(),
                "MissingModalityPolicy".into(),
                "ModelValueConstraint".into(),
                "RelationValue".into(),
                "RetrievalMode".into(),
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
                "TemporalProvenance".into(),
                "TemporalEvidenceBatch".into(),
                "TemporalEvidenceCandidate".into(),
                "TemporalRetrievalWindow".into(),
                "EmbeddingProfile".into(),
                "VectorMetadata".into(),
                "SimilarityQuery".into(),
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
                "Citation".into(),
                "ContextOmission".into(),
                "ExtractedField".into(),
                "ExtractionLineage".into(),
                "FiniteClassification".into(),
                "IntegrationAccuracy".into(),
                "ModelAxisConstraint".into(),
                "ModelRelationSignature".into(),
                "ModelPortConstraint".into(),
                "RelationQuery".into(),
                "ModelSignature".into(),
                "ModelTensorConstraint".into(),
                "RelationEvidence".into(),
                "RelationVariable".into(),
                "ModelWorkAccounting".into(),
                "SelectedContextCost".into(),
                "SourceRef".into(),
                "TemporalReference".into(),
                "TemporalProvenance".into(),
                "TemporalEvidenceBatch".into(),
                "TemporalEvidenceCandidate".into(),
                "ValidatedExtraction".into(),
                "Embedding".into(),
                "EmbeddingProfile".into(),
                "VectorMetadata".into(),
                "SimilarityQuery".into(),
            ]
            .into(),
            record_constructor_orders: [
                ("AnswerSpan".into(), vec!["start".into(), "end".into()]),
                (
                    "MeanCovariance".into(),
                    vec![
                        "mean".into(),
                        "covariance".into(),
                        "provenance".into(),
                        "disposition".into(),
                    ],
                ),
                (
                    "MeanVariance".into(),
                    vec![
                        "mean".into(),
                        "variance".into(),
                        "provenance".into(),
                        "disposition".into(),
                    ],
                ),
                (
                    "ProbabilitySample".into(),
                    vec!["value".into(), "provenance".into(), "disposition".into()],
                ),
                (
                    "ProbabilitySampleSet".into(),
                    vec![
                        "alternatives".into(),
                        "provenance".into(),
                        "disposition".into(),
                    ],
                ),
                (
                    "TrajectoryAlternatives".into(),
                    vec![
                        "observation_identity".into(),
                        "plausible_alternatives".into(),
                        "provenance".into(),
                        "disposition".into(),
                    ],
                ),
                (
                    "WeightedSamples".into(),
                    vec![
                        "alternatives".into(),
                        "weights".into(),
                        "provenance".into(),
                        "disposition".into(),
                    ],
                ),
                (
                    "AnswerClaimSupport".into(),
                    vec!["answer_span".into(), "support".into()],
                ),
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
                    "GroundedClaim".into(),
                    vec!["answer_span".into(), "citation_indices".into()],
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
                    "RetrievalIntent".into(),
                    vec![
                        "identity".into(),
                        "modes".into(),
                        "maximum_candidates".into(),
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
                    "TemporalRetrievalWindow".into(),
                    vec!["start".into(), "end".into()],
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
