//! Finite conditional queries over one learned relational artifact.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_data::{SampledSignal, TensorValue};

use crate::{ModelValueConstraint, ProbabilisticDisposition, RandomnessProfile};

#[path = "relation_digest.rs"]
mod digest;
#[path = "relation_validation.rs"]
mod validation;

pub const MAXIMUM_RELATION_VARIABLES: usize = 32;
pub const MAXIMUM_RELATION_PATTERNS: usize = 64;
pub const MAXIMUM_RELATION_VALUES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationVariable {
    pub identity: String,
    pub semantic_role: String,
    pub value: ModelValueConstraint,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RelationQueryMode {
    InferPosterior,
    SampleConditional,
    Reconstruct,
    EncodeLatent,
    DecodeGenerate,
    LogProbability,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RelationResultProfile {
    Deterministic,
    Probabilistic { maximum_samples: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportedRelationQuery {
    pub evidence_variables: Vec<String>,
    pub target_variables: Vec<String>,
    pub mode: RelationQueryMode,
    pub result_profile: RelationResultProfile,
    pub maximum_work_units: u64,
    pub maximum_output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRelationSignature {
    pub identity: String,
    pub compatibility_version: u32,
    pub callable_signature_identity: [u8; 32],
    pub variables: Vec<RelationVariable>,
    pub supported_queries: Vec<SupportedRelationQuery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationValue {
    Tensor(TensorValue),
    SampledSignal(SampledSignal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationEvidence {
    pub variable: String,
    pub value: RelationValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationQuery {
    pub identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub relation_signature_identity: [u8; 32],
    pub evidence: Vec<RelationEvidence>,
    pub targets: Vec<String>,
    pub mode: RelationQueryMode,
    pub requested_result: RelationResultProfile,
    pub randomness: RandomnessProfile,
    pub admitted_work_units: u64,
    pub maximum_output_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationRealization {
    pub implementation_identity: String,
    pub runtime_name: String,
    pub runtime_version: String,
    pub runtime_build_identity: String,
    pub device_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationCandidate {
    pub outputs: Vec<RelationCandidateOutput>,
    pub consumed_work_units: u64,
    pub encoded_output_bytes: u64,
    pub realization: RelationRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationCandidateOutput {
    pub target_variable: String,
    pub value_identity: [u8; 32],
    pub disposition: ProbabilisticDisposition,
    pub sample_count: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RelationTerminal {
    Cancelled,
    ResourceExhausted,
    ProviderLost,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostRelationTerminal {
    Candidate(Box<RelationCandidate>),
    NoResult(RelationTerminal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationReceipt {
    pub query_identity: [u8; 32],
    pub query_descriptor_identity: [u8; 32],
    pub artifact_identity: [u8; 32],
    pub checkpoint_identity: Option<[u8; 32]>,
    pub relation_signature_identity: [u8; 32],
    pub evidence_identities: Vec<(String, [u8; 32])>,
    pub targets: Vec<String>,
    pub mode: RelationQueryMode,
    pub requested_result: RelationResultProfile,
    pub randomness: RandomnessProfile,
    pub admitted_work_units: u64,
    pub consumed_work_units: u64,
    pub output_identities: Vec<(String, [u8; 32])>,
    pub realization: RelationRealization,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationQueryOutcome {
    Completed(Box<RelationReceipt>),
    NoResult(RelationTerminal),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RelationRefusal {
    MissingIdentity,
    InvalidSignature,
    TooManyVariables,
    DuplicateVariable,
    InvalidPattern,
    TooManyPatterns,
    DuplicatePattern,
    ArtifactMismatch,
    UnknownVariable,
    DuplicateEvidence,
    DuplicateTarget,
    UnsupportedQuery,
    InvalidValue,
    ShapeMismatch,
    DeterminismMismatch,
    InvalidRandomness,
    WorkBoundExceeded,
    OutputBoundExceeded,
    InvalidResult,
    InvalidRealization,
}
