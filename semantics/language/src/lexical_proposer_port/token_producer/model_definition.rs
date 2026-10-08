//! Distinct declaration for resource-backed proposals. This is metadata custody,
//! not evidence that a model executed or that its selected analysis is correct.
use super::revision::PreparedRevisionProducer;
use crate::parser_model_selection::ParserSourceModelContract;
use alloc::{boxed::Box, sync::Arc};
use conduit_ai::{
    integer_categorical_step::PreparedCategoricalStep, ModelArtifact, ModelSignature,
};
use conduit_core::semantic_digest;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalModelRefusal {
    Declaration,
    Proposer,
    Artifact,
    Signature,
    Dimensions,
    Source,
    Score,
    Capacity,
}
/// Source-defined scalar segmentation, independent of token finality/stability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProposalSegmentationAbi {
    UnicodeScalarAlphanumericApostropheV1,
}
/// Every field is immutable. The complete proposal policy and training manifest
/// are retained; an identity digest never substitutes for their material.
pub struct ProposalModelDefinition {
    identity: Box<str>,
    proposer: Box<[u8]>,
    dictionary: [u8; 32],
    proposal_source: [u8; 32],
    segmentation: ProposalSegmentationAbi,
    feature_abi: Box<str>,
    source: ParserSourceModelContract,
    artifact: ModelArtifact,
    signature: ModelSignature,
    dimensions: (usize, usize, usize),
    maximum_score_magnitude: u64,
    training_manifest: Box<[u8]>,
    training_identity: [u8; 32],
}
impl ProposalModelDefinition {
    /// The artifact/signature are already-owned model metadata. Their preparation
    /// and retained storage are the model owner's responsibility, separately from
    /// the lexical revision receipt. This method does not relabel a legacy profile.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        owner: &PreparedRevisionProducer,
        identity: Box<str>,
        feature_abi: Box<str>,
        source: ParserSourceModelContract,
        artifact: ModelArtifact,
        signature: ModelSignature,
        dimensions: (usize, usize, usize),
        maximum_score_magnitude: u64,
        training_manifest: Box<[u8]>,
        maximum_copied_proposer_bytes: usize,
    ) -> Result<Self, ProposalModelRefusal> {
        use ProposalModelRefusal as R;
        if identity.is_empty()
            || identity.len() > 256
            || feature_abi.is_empty()
            || feature_abi.len() > 256
            || training_manifest.is_empty()
            || training_manifest.len() > 1_048_576
        {
            return Err(R::Declaration);
        }
        if dimensions.0 == 0
            || dimensions.0 > 65536
            || dimensions.1 != 76
            || dimensions.2 == 0
            || dimensions.2 > 128
        {
            return Err(R::Dimensions);
        }
        if maximum_score_magnitude > i64::MAX as u64 {
            return Err(R::Score);
        }
        signature.validate().map_err(|_| R::Signature)?;
        artifact.validate(&signature).map_err(|_| R::Artifact)?;
        let proposer = &owner.producer.port.definition;
        if proposer.len() > maximum_copied_proposer_bytes {
            return Err(R::Capacity);
        }
        let training_identity = semantic_digest(
            "language/lexical-proposal-training-manifest@1",
            &training_manifest,
        );
        Ok(Self {
            identity,
            proposer: proposer.as_ref().into(),
            dictionary: owner.producer.port.dictionary.identity(),
            proposal_source: owner.producer.port.source_identity,
            segmentation: ProposalSegmentationAbi::UnicodeScalarAlphanumericApostropheV1,
            feature_abi,
            source,
            artifact,
            signature,
            dimensions,
            maximum_score_magnitude,
            training_manifest,
            training_identity,
        })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn canonical_proposer_definition(&self) -> &[u8] {
        &self.proposer
    }
    pub const fn dictionary_identity(&self) -> [u8; 32] {
        self.dictionary
    }
    pub const fn proposal_source_identity(&self) -> [u8; 32] {
        self.proposal_source
    }
    /// Complete reviewed lexical proposal Source material, separate from its
    /// prepared expression identity. Final execution remains the Source owner's duty.
    pub fn proposal_source_material(&self) -> &'static [u8] {
        include_bytes!("../../../lexical_proposer.conduit")
    }
    pub const fn segmentation_abi(&self) -> ProposalSegmentationAbi {
        self.segmentation
    }
    pub fn feature_abi(&self) -> &str {
        &self.feature_abi
    }
    pub fn source_contract(&self) -> &ParserSourceModelContract {
        &self.source
    }
    pub fn artifact(&self) -> &ModelArtifact {
        &self.artifact
    }
    pub fn signature(&self) -> &ModelSignature {
        &self.signature
    }
    pub const fn dimensions(&self) -> (usize, usize, usize) {
        self.dimensions
    }
    pub const fn maximum_score_magnitude(&self) -> u64 {
        self.maximum_score_magnitude
    }
    pub fn training_manifest(&self) -> &[u8] {
        &self.training_manifest
    }
    pub const fn training_identity(&self) -> [u8; 32] {
        self.training_identity
    }
}
/// Exact model selection for the distinct proposal declaration. The caller must
/// retain and execute the checked Source identified by actual_source. This
/// selection itself grants no lexical fact, commitment or Session publication.
pub struct PreparedProposalModelSelection {
    categorical: Arc<PreparedCategoricalStep>,
    declaration: Arc<ProposalModelDefinition>,
}
impl PreparedProposalModelSelection {
    pub fn prepare(
        owner: &PreparedRevisionProducer,
        categorical: Arc<PreparedCategoricalStep>,
        declaration: Arc<ProposalModelDefinition>,
        actual_source: &ParserSourceModelContract,
    ) -> Result<Self, ProposalModelRefusal> {
        use ProposalModelRefusal as R;
        if declaration.proposal_source != owner.producer.port.source_identity
            || declaration.dictionary != owner.producer.port.dictionary.identity()
            || declaration.proposer.as_ref() != owner.producer.port.definition.as_ref()
        {
            return Err(R::Proposer);
        }
        if &declaration.source != actual_source {
            return Err(R::Source);
        }
        if categorical.resource().artifact() != &declaration.artifact {
            return Err(R::Artifact);
        }
        if categorical.resource().signature() != &declaration.signature {
            return Err(R::Signature);
        }
        if categorical.dimensions() != declaration.dimensions {
            return Err(R::Dimensions);
        }
        if categorical
            .indices_type()
            .semantic_digest()
            .map_err(|_| R::Source)?
            != actual_source.numeric_indices_contract
            || categorical
                .scores_type()
                .semantic_digest()
                .map_err(|_| R::Source)?
                != actual_source.numeric_scores_contract
        {
            return Err(R::Source);
        }
        if categorical.maximum_score_magnitude() > declaration.maximum_score_magnitude {
            return Err(R::Score);
        }
        Ok(Self {
            categorical,
            declaration,
        })
    }
    pub fn declaration(&self) -> &ProposalModelDefinition {
        &self.declaration
    }
    pub fn categorical(&self) -> &PreparedCategoricalStep {
        &self.categorical
    }
}
