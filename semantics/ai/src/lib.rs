#![no_std]

extern crate alloc;

// Native bounded-sequence payloads intentionally retain their admitted inline
// capacity rather than hiding a play-time allocation behind enum indirection.
#[allow(dead_code, clippy::large_enum_variant)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}
pub use generated::{
    BaseProofClass, BatchOrder, CandidateLifecycle, CheckpointPolicy, ClassificationLabel,
    ClassificationLabels, CompatibleMetrics, ContextOmissionReason, ContextOrderingPolicy,
    ContextRedundancyPolicy, ContextSelectionPolicy, ContextSelectionRationale, ContextSelectionRefusal,
    ConfidencePermille, ContextTruncationReason, DataHandling, DrawRelationship, DrawRelationshipCorrelated,
    DynamicsRefusal, EmbeddingNormalization, EmbodimentStage, EntityBoundary,
    EvaluationDisposition, EvaluationPolicy, ExactVectorSearchRefusal, ExtractedField,
    ExtractionFields, ExtractionKey, ExtractionValue, FiniteClassification, FiniteF32,
    FusionStrategy, FusionStrategyReciprocalRank, GeneratedTextFlowRefusal,
    GeneratedTextFlowTerminal, GroundedAnswerDisposition, GroundedAnswerRefusal,
    GroundingDisposition, HouseContextProvenanceClass, HouseContextRefusal,
    HumanAssessmentDisposition, HybridRetrievalOfferInvalidity, IntegrationAccuracy,
    IntegrationTerminal, InterpretationDisposition, InterpretationInvalidity,
    InterpretationProvenance, LearnedLifecycleRefusal, LlmDeterminismProfile,
    LlmImplementationControl, LlmInterruptionReason, LlmPlanningRefusal, LlmTerminalOutcome,
    LocalModelCachePolicy, LocalModelFailure, LocalModelKindProfile, LocalModelLifecycleState,
    LocalModelOfferInvalidity, LocalModelRefusal, LocalModelTerminal, LogScoreKind, MechanismScore,
    Metering, MissingModality, MissingModalityPolicy, MissingModalityPolicyPermitDeclared,
    ModelCachePolicy, ModelCachePolicyBounded, ModelCompatibilityRefusal, ModelComputeLifecycle,
    ModelComputeOperation, ModelComputeRefusal, ModelDimensionConstraint,
    ModelDimensionConstraintBounded, ModelDimensionConstraintFixed, ModelEvidenceRefusal,
    ModelFailure, ModelInvocationTerminal, ModelOperation, ModelOperationCode, ModelPortPresence,
    ModelPortPresenceCode, ModelRefusal, ModelResultDisposition, ModelResultInvalidity,
    ModelResultProvenance, ModelSignatureRefusal, ModelTextRefusal, ModelWorkAccounting,
    NonnegativeFiniteF32,
    ObjectiveParticipation, PortableComputeClass, ProbabilisticDisposition,
    ProbabilisticDispositionApproximate, ProbabilisticDispositionTruncated, ProbabilityRefusal,
    PromotionDecision, PromotionTerminal, R3OfferInvalidity, RagAnswerOfferInvalidity,
    RandomnessProfile, RandomnessProfileExplicitSeed, RandomnessProfileProviderChosen,
    RelationQueryMode, RelationRefusal, RelationResultProfile, RelationResultProfileProbabilistic,
    RelationTerminal, RerankScore, RerankingPolicy, RerankingProofClass, RerankingRefusal, RerankingStrategy,
    RerankingStrategyObservedScores, RetrievalMechanism, RetrievalMechanismCode, RollbackTerminal,
    SelectedContextRationale, ShadowTerminal, SimilarityMetric, SimilarityScore,
    SimilarityThreshold, SourceExtractionOfferInvalidity, SourceExtractionProfile,
    SourceExtractionProfileResourceMetadata, SourceExtractionProfileStructuredItems,
    SourceExtractionProfileTextUtf8, SourceSpanUnit, StructuredResultInvalidity,
    TemporalContextRefusal, TemporalEvidenceSelectionRefusal, TemporalInterpretationRefusal,
    TemporalSource, TemporalValidity, TemporalWindowRelation, TrainStepFailure,
    TrainingLifecyclePhase, TrainingLifecyclePhaseActiveStep, TrainingRefusal, TransitionDirection,
    ValidatedExtraction, VectorIndexHealth, VectorIndexMaintenanceKind, VectorIndexResourceRefusal,
    VectorRefusal, VectorSearchExecutionProofClass, VectorSearchOfferInvalidity,
    VectorSearchProofClass,
};

mod bases;
pub use bases::*;
mod context_selection;
pub use context_selection::*;
mod context_selection_contract;
pub use context_selection_contract::*;
#[cfg(feature = "form-catalog")]
mod form_composition;
#[cfg(feature = "form-catalog")]
pub use form_composition::*;
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
#[cfg(feature = "form-catalog")]
mod provider;
#[cfg(feature = "form-catalog")]
pub use provider::*;

pub const TEXT_VALUE_KIND: &str = "value/text";
