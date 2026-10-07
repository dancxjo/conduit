#![no_std]

extern crate alloc;
#[cfg(feature = "hosted-catalog-cache")]
extern crate std;
#[cfg(all(feature = "hosted-catalog-cache", not(target_has_atomic = "ptr")))]
compile_error!("hosted checked-catalog cache requires pointer atomics and a std target");

// Native bounded-sequence payloads intentionally retain their admitted inline
// capacity rather than hiding a play-time allocation behind enum indirection.
#[allow(dead_code, clippy::large_enum_variant)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}
pub use generated::{
    AnswerClaimSupport, AnswerSpan, BaseProofClass, BatchOrder, CandidateLifecycle,
    CheckpointPolicy, ChunkIdentity, Citation, CitationIndices, ClassificationLabel,
    ClassificationLabels, ClockBasis, ClockBasisMonotonicMilliseconds, ClockIdentity,
    CompatibleMetrics, ConfidencePermille, ContextBudgetCost, ContextOmission,
    ContextOmissionReason, ContextOmissions, ContextOrderingPolicy, ContextRedundancyPolicy,
    ContextSelectionDisposition, ContextSelectionDispositionOmitted, ContextSelectionOutcome,
    ContextSelectionOutcomeTruncated, ContextSelectionPolicy, ContextSelectionRationale,
    ContextSelectionRefusal, ContextTruncationReason, DataHandling, DrawRelationship,
    DrawRelationshipCorrelated, DynamicsRefusal, Embedding, EmbeddingNormalization,
    EmbeddingProfile, EmbeddingProfileIdentity, EmbodimentStage, EntityBoundary,
    EvaluationDisposition, EvaluationPolicy, ExactVectorSearchRefusal, ExtractedField,
    ExtractionFields, ExtractionKey, ExtractionLineage, ExtractionValue, FiniteClassification,
    FiniteEmbedding, FiniteEmbeddingValuePage, FiniteEmbeddingValuePages, FiniteF32,
    FusionStrategy, FusionStrategyReciprocalRank, GeneratedTextChunk, GeneratedTextFlowEvidence,
    GeneratedTextFlowRefusal, GeneratedTextFlowTerminal, GroundedAnswerBytePage,
    GroundedAnswerBytePages, GroundedAnswerDisposition, GroundedAnswerPolicy,
    GroundedAnswerRefusal, GroundedClaim, GroundedClaimSupport, GroundedClaimSupportSupported,
    GroundedClaimSupportUnsupported, GroundingDisposition, GroundingInputAssessment,
    GroundingInputAssessmentConflictingEvidence, GroundingInputAssessmentInsufficientEvidence,
    GroundingLimitation, HouseContextProvenanceClass, HouseContextRefusal, HouseModelRequest,
    HumanAssessmentDisposition, HybridFusionPolicy, HybridRequiredMechanisms,
    HybridRetrievalOfferInvalidity, IntegrationAccuracy, IntegrationResourceEnvelope,
    IntegrationTerminal, InterpretationDisposition, InterpretationInvalidity,
    InterpretationProvenance, LearnedLifecycleRefusal, LlmDeterminismProfile,
    LlmImplementationControl, LlmInterruptionReason, LlmPlanningRefusal, LlmTerminalOutcome,
    LlmWorkBounds, LocalModelCachePolicy, LocalModelFailure, LocalModelKindProfile,
    LocalModelLifecycleState, LocalModelOfferInvalidity, LocalModelRefusal, LocalModelTerminal,
    LogProbability, LogScoreKind, MeanCovariance, MeanVariance, MechanismScore, MetadataFilter,
    MetadataFilters, Metering, MissingModality, MissingModalityPolicy,
    MissingModalityPolicyPermitDeclared, ModelAxisConstraint, ModelCachePolicy,
    ModelCachePolicyBounded, ModelCompatibilityRefusal, ModelComputeLifecycle,
    ModelComputeOperation, ModelComputeRefusal, ModelDimensionConstraint,
    ModelDimensionConstraintBounded, ModelDimensionConstraintFixed, ModelEvidenceRefusal,
    ModelFailure, ModelInvocationTerminal, ModelOperation, ModelOperationForm, ModelOperations,
    ModelPortConstraint, ModelPortIdentity, ModelPortPresence, ModelPortPresenceForm, ModelPorts,
    ModelRefusal, ModelRelationSignature, ModelResultDisposition, ModelResultInvalidity,
    ModelResultPayload, ModelResultProvenance, ModelSemanticKind, ModelSignature,
    ModelSignatureRefusal, ModelTensorAxes, ModelTensorConstraint, ModelTensorElements,
    ModelTextRefusal, ModelValueConstraint, ModelValueConstraintProbabilisticSignal,
    ModelValueConstraintProbabilisticTensor, ModelValueConstraintSampledSignal,
    ModelValueConstraintTensor, ModelWorkAccounting, NonnegativeFiniteF32, ObjectiveParticipation,
    PortableComputeClass, PortableGroundedAnswer, PortableModelDerivedResult,
    PortableProposedClaimSupport, PortableProposedGroundedClaim, ProbabilisticDisposition,
    ProbabilisticDispositionApproximate, ProbabilisticDispositionTruncated,
    ProbabilityClaimProfile, ProbabilityDigest, ProbabilityRefusal, ProbabilitySample,
    ProbabilitySampleSet, ProbabilitySummary, ProfileReportedConfidence, PromotionDecision,
    PromotionTerminal, R3OfferInvalidity, RagAnswerOfferInvalidity, RagIdentity, RandomnessProfile,
    RandomnessProfileExplicitSeed, RandomnessProfileProviderChosen, RelationCandidateOutput,
    RelationDigest, RelationEvidence, RelationEvidenceValues, RelationIdentity, RelationQuery,
    RelationQueryIdentity, RelationQueryMode, RelationRefusal, RelationResultProfile,
    RelationResultProfileProbabilistic, RelationSemanticRole, RelationTerminal, RelationValue,
    RelationVariable, RelationVariableIdentities, RelationVariableIdentity, RelationVariables,
    RerankObservation, RerankScore, RerankingPolicy, RerankingProofClass, RerankingRefusal,
    RerankingStrategy, RerankingStrategyObservedScores, RetrievalContribution, RetrievalIntent,
    RetrievalIntentIdentity, RetrievalMechanism, RetrievalMechanismForm, RetrievalMode,
    RetrievalModeBoundary, RetrievalModeTemporal, RetrievalModes, RetrievalScore,
    RetrieverIdentity, RollbackTerminal, SelectedContextCost, SelectedContextRationale,
    ShadowResourceEnvelope, ShadowTerminal, SimilarityMetric, SimilarityQuery, SimilarityScore,
    SimilarityThreshold, SourceExtractionLimits, SourceExtractionOfferInvalidity,
    SourceExtractionProfile, SourceExtractionProfileResourceMetadata,
    SourceExtractionProfileStructuredItems, SourceExtractionProfileTextUtf8, SourceRef, SourceSpan,
    SourceSpanUnit, StochasticProvenance, StructuredResultInvalidity, SupportedRelationQueries,
    SupportedRelationQuery, TemporalContextRefusal, TemporalEvidenceBatch,
    TemporalEvidenceCandidate, TemporalEvidenceCandidates, TemporalEvidenceIdentities,
    TemporalEvidenceIdentity, TemporalEvidenceSelection, TemporalEvidenceSelectionRefusal,
    TemporalEvidenceSelectionSelected, TemporalInterpretationRefusal, TemporalProvenance,
    TemporalReference, TemporalRetrievalIntent, TemporalRetrievalIntentDurationSince,
    TemporalRetrievalIntentEvidenceWithin, TemporalRetrievalIntentStateValidAt,
    TemporalRetrievalIntentTransition, TemporalRetrievalWindow, TemporalSource, TemporalValidity,
    TemporalWindowRelation, TrainStepFailure, TrainStepRequest, TrainingBatch,
    TrainingExampleIdentityPage, TrainingExampleIdentityPages, TrainingLifecyclePhase,
    TrainingLifecyclePhaseActiveStep, TrainingMetric, TrainingModalities, TrainingModality,
    TrainingObjective, TrainingObjectiveIdentity, TrainingObjectives, TrainingRefusal,
    TrainingResourceEnvelope, TrainingSession, TrajectoryAlternatives, TransformProfiles,
    TransitionDirection, ValidatedExtraction, VectorIndexHealth, VectorIndexMaintenanceKind,
    VectorIndexResourceRefusal, VectorMetadata, VectorMetadataEntries, VectorRefusal,
    VectorSearchExecutionProofClass, VectorSearchOfferInvalidity, VectorSearchProofClass,
    WeightedSamples, WiredHouseContextItem,
};

mod bases;
pub use bases::*;
mod context_selection;
pub use context_selection::*;
mod context_selection_contract;
pub use context_selection_contract::*;
#[cfg(feature = "plot-catalog")]
mod plot_composition;
#[cfg(feature = "plot-catalog")]
pub use plot_composition::*;
mod effect_proposal;
pub use effect_proposal::*;
mod embodiment;
pub use embodiment::*;
mod effect_runtime;
mod grounded_answer;
pub use grounded_answer::*;
mod house_context;
pub use house_context::*;
mod grounded_answer_contract;
pub use grounded_answer_contract::*;
mod llm_contract;
pub use llm_contract::*;
mod streaming_generation;
pub use streaming_generation::*;
mod local_model;
pub use local_model::*;
mod cross_host_lifecycle;
pub use cross_host_lifecycle::*;
mod interpretation;
pub use interpretation::*;
mod hybrid_retrieval;
pub use hybrid_retrieval::*;
mod hybrid_retrieval_codec;
pub use hybrid_retrieval_codec::*;
mod hybrid_retrieval_contract;
pub use hybrid_retrieval_contract::*;
mod model_result;
pub use model_result::*;
mod model_text;
pub use model_text::*;
mod model_signature;
pub use model_signature::*;
mod model_artifact;
pub use model_artifact::*;
#[cfg(target_has_atomic = "ptr")]
mod model_resource;
#[cfg(target_has_atomic = "ptr")]
pub use model_resource::*;
mod learned_lifecycle;
pub use learned_lifecycle::*;
mod model_compute;
pub use model_compute::*;
mod probability;
pub use probability::*;
mod probability_digest;
mod relation;
pub use relation::*;
mod rag_semantics;
pub use rag_semantics::*;
mod reranking;
pub use reranking::*;
mod source_extraction_contract;
pub use source_extraction_contract::*;
mod source_extraction;
pub use source_extraction::*;
mod source_extraction_codec;
pub use source_extraction_codec::*;
mod structured_result;
pub use structured_result::*;
mod temporal_context;
pub use temporal_context::*;
mod temporal_follow_up;
pub use temporal_follow_up::*;
mod temporal_interpretation;
pub use temporal_interpretation::*;
mod temporal_evidence_selection;
pub use temporal_evidence_selection::*;
mod dynamics;
pub use dynamics::*;
mod dynamics_digest;
mod training;
pub use training::*;
mod training_digest;
mod vector_index_lifecycle;
pub use vector_index_lifecycle::*;
mod vector_index_resource;
pub use vector_index_resource::*;
mod vector_exact_search;
pub use vector_exact_search::*;
mod vector_retrieval;
pub use vector_retrieval::*;
mod vector_search_contract;
pub use vector_search_contract::*;
#[cfg(feature = "plot-catalog")]
mod provider;
#[cfg(feature = "plot-catalog")]
pub use provider::*;

pub const TEXT_VALUE_KIND: &str = "value/text";

/// Generic fixed-shape reference numeric helpers; graph admission is separate.
pub mod fixed_neural;

/// Reference computation over exact fixed resource-backed Data tensors.
pub mod fixed_tensor;

#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_back;
pub mod fixed_numeric_binding;
pub mod fixed_numeric_catalog;
pub mod fixed_numeric_codec;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_pair_back;
pub mod fixed_numeric_pair_catalog;
pub mod fixed_numeric_preparation;
mod fixed_numeric_signal_catalog;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_window_back;

/// Explicit signed Q7 and packed integer resource linear helpers.
pub mod fixed_compact;
#[cfg(feature = "kernel-step")]
mod fixed_numeric_finite_envelope;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_index_back;
pub mod fixed_numeric_index_codec;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_linear_back;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_operations_back;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_scan_back;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_signal_back;
pub mod fixed_tensor_linear;
pub mod fixed_tensor_resource;

#[cfg(all(feature = "kernel-step", target_has_atomic = "ptr"))]
pub mod fixed_numeric_flow;

#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_pair_flow;

pub mod fixed_numeric_dsp;

#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_dsp_back;
pub mod fixed_numeric_dsp_catalog;

pub mod fixed_numeric_i16_codec;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_integer_conversion;

#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_temporal;

#[cfg(all(feature = "kernel-step", target_has_atomic = "ptr"))]
pub mod fixed_numeric_embedding_flow;
#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_integer_narrowing;

#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_u16_profile;

#[cfg(feature = "kernel-step")]
pub mod fixed_numeric_float_integer;
#[cfg(all(feature = "kernel-step", target_has_atomic = "ptr"))]
pub mod fixed_numeric_linear_flow;

#[cfg(all(feature = "kernel-step", target_has_atomic = "ptr"))]
pub mod fixed_numeric_compact_back;
pub mod fixed_numeric_compact_catalog;
