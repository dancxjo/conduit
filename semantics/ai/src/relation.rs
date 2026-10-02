//! Finite conditional queries over one learned relational artifact.

use crate::{
    ModelRelationSignature, ModelValueConstraint, RandomnessProfile, RelationCandidateOutput,
    RelationDigest, RelationEvidence, RelationEvidenceValues, RelationIdentity, RelationQuery,
    RelationQueryIdentity, RelationQueryMode, RelationRefusal, RelationResultProfile,
    RelationSemanticRole, RelationTerminal, RelationValue, RelationVariable,
    RelationVariableIdentities, RelationVariableIdentity, RelationVariables,
    SupportedRelationQueries, SupportedRelationQuery,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal};

#[path = "relation_digest.rs"]
mod digest;
#[path = "relation_validation.rs"]
mod validation;

pub const MAXIMUM_RELATION_VARIABLES: usize = 32;
pub const MAXIMUM_RELATION_PATTERNS: usize = 64;
pub const MAXIMUM_RELATION_VALUES: usize = 32;

impl RelationVariable {
    pub fn from_parts(
        identity: String,
        semantic_role: String,
        value: ModelValueConstraint,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::new(
            RelationIdentity::new(identity)?,
            RelationSemanticRole::new(semantic_role)?,
            value,
        )
    }
}

impl ModelRelationSignature {
    pub fn from_parts(
        identity: String,
        compatibility_version: u32,
        callable_signature_identity: [u8; 32],
        variables: Vec<RelationVariable>,
        supported_queries: Vec<SupportedRelationQuery>,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::new(
            RelationDigest::new(callable_signature_identity)?,
            compatibility_version,
            identity,
            SupportedRelationQueries::new(
                BoundedSequence::try_from_iter(supported_queries)
                    .map_err(|_| invalid_relation_value())?,
            )?,
            RelationVariables::new(
                BoundedSequence::try_from_iter(variables).map_err(|_| invalid_relation_value())?,
            )?,
        )
    }
}

impl RelationEvidence {
    pub fn from_parts(
        variable: String,
        value: RelationValue,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::new(value, RelationVariableIdentity::new(variable)?)
    }
}

impl SupportedRelationQuery {
    pub fn from_parts(
        evidence_variables: Vec<String>,
        target_variables: Vec<String>,
        mode: RelationQueryMode,
        result_profile: RelationResultProfile,
        maximum_work_units: u64,
        maximum_output_bytes: u64,
    ) -> Result<Self, NativeBindingRefusal> {
        let identities = |values: Vec<String>| {
            let values = values
                .into_iter()
                .map(RelationVariableIdentity::new)
                .collect::<Result<Vec<_>, _>>()?;
            RelationVariableIdentities::new(
                BoundedSequence::try_from_iter(values).map_err(|_| invalid_relation_value())?,
            )
        };
        Self::new(
            identities(evidence_variables)?,
            identities(target_variables)?,
            mode,
            result_profile,
            maximum_work_units,
            maximum_output_bytes,
        )
    }
}

impl RelationQuery {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        identity: [u8; 32],
        artifact_identity: [u8; 32],
        checkpoint_identity: Option<[u8; 32]>,
        relation_signature_identity: [u8; 32],
        evidence: Vec<RelationEvidence>,
        targets: Vec<String>,
        mode: RelationQueryMode,
        requested_result: RelationResultProfile,
        randomness: RandomnessProfile,
        admitted_work_units: u64,
        maximum_output_bytes: u64,
    ) -> Result<Self, NativeBindingRefusal> {
        let targets = targets
            .into_iter()
            .map(RelationVariableIdentity::new)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(
            admitted_work_units,
            RelationDigest::new(artifact_identity)?,
            checkpoint_identity.map(RelationDigest::new).transpose()?,
            RelationEvidenceValues::new(
                BoundedSequence::try_from_iter(evidence).map_err(|_| invalid_relation_value())?,
            )?,
            RelationQueryIdentity::new(identity)?,
            maximum_output_bytes,
            mode,
            randomness,
            RelationDigest::new(relation_signature_identity)?,
            requested_result,
            RelationVariableIdentities::new(
                BoundedSequence::try_from_iter(targets).map_err(|_| invalid_relation_value())?,
            )?,
        )
    }
}

fn invalid_relation_value() -> NativeBindingRefusal {
    NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongCollectionLength)
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
