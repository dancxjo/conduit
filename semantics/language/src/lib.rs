#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    AnnotationBundleFour, LanguageAnalysisRevisionId, LanguageAnalysisTokenRef, LanguageCoverage,
    LanguageDependencyArc, LanguageDependencyHead, LanguageDependencyRelation,
    LanguageDependencySubtype, LanguageDiscourseRole, LanguageExternalIdentity,
    LanguageFallbackProsodyAccepted, LanguageFallbackProsodyRequest, LanguageId,
    LanguageLexicalCandidate, LanguageLexicalCompleteness, LanguageLexicalEntry,
    LanguageLexicalPos, LanguageLexicalProfile, LanguageLexicalTape, LanguageLexicalToken,
    LanguageMappingDeclaration, LanguageProsodyBoundary, LanguageProsodyChoice,
    LanguageProsodyPitch, LanguageProsodyProfile, LanguageProsodyProminence, LanguageRequest,
    LanguageRichProsodyAccepted, LanguageRichProsodyRequest, LanguageText, LanguageTextFinality,
    LanguageTextId, LanguageTextPriorRevision, LanguageTextRange, LanguageTextReferenceMatch,
    LanguageTextRevision, LanguageTextRevisionId, LanguageTextRevisionLineage,
    LanguageTextSegmentKind, LanguageTextSegmentRef, LanguageUniversalDependencyRelation,
    LanguageVariety, LanguageVarietyPolicy, LanguageVocativeDiscourseAdmission,
    LanguageVocativeDiscourseFact, LinguisticAnnotation, LinguisticDependencyEdge,
    LinguisticDependencyRelation, LinguisticDerivationProvenance, LinguisticEvidence,
    LinguisticOffsetBasis, LinguisticOptionalText, LinguisticSegment, LinguisticSegmentKind,
    LinguisticSyntacticLinkKind, LinguisticToken, LinguisticTokenCategory, LinguisticTokenFeature,
    LinguisticTokenFeatureSlot, LinguisticTokenIdentity, LinguisticTokensFour, TextSpan, VarietyId,
};

pub use generated::{
    LanguageParserAction, LanguageParserArcProposal, LanguageParserArcQuery,
    LanguageParserArcResult, LanguageParserBasis, LanguageParserBegin, LanguageParserContext,
    LanguageParserNumericState, LanguageParserRefusal, LanguageParserRelation,
    LanguageParserRequest, LanguageParserResult, LanguageParserState, LanguageParserSubtype,
};

pub use generated::{
    LanguageParserAdvanceRequest, LanguageParserAgreement, LanguageParserAgreementQuery,
    LanguageParserBeam, LanguageParserCommitRequest, LanguageParserEdgeVote,
    LanguageParserFeatures, LanguageParserFrontier, LanguageParserFrontierRefusal,
    LanguageParserFrontierResult, LanguageParserHypothesis, LanguageParserPruneRequest,
    LanguageParserRawFrontier, LanguageParserRawHypothesis, LanguageParserVoteCheck,
    LanguageParserVotes,
};

pub use generated::{LanguageParserAdmittedAgreementQuery, LanguageParserAdmittedBeam};

pub mod parser_session_runtime;

mod parser;
pub use parser::parser_types;

mod catalog;
mod info;
mod reference;

pub use catalog::*;
pub use info::*;
pub use reference::*;

mod dependency;
pub use dependency::*;

pub mod revision;

mod identity;
pub use identity::*;

mod mapping;
pub use mapping::*;

mod mapping_projection;
pub use mapping_projection::*;

mod coverage;
pub use coverage::*;

mod request_contract;
pub use request_contract::*;

mod source_material;
pub use source_material::*;

mod text_revision;
pub use text_revision::*;

mod revision_lineage;
pub use revision_lineage::*;

pub mod lexical;
mod lexical_canonical_sequence;
pub mod lexical_proposer_port;
pub mod lexical_proposer_resource;
mod lexical_scalar_scan;
/// Exact descriptor-backed representation construction for independent owners.
pub mod parser_canonical_composition;
/// Representation construction only; no admission or publication authority.
pub mod parser_canonical_schema;
pub use generated::{
    LanguageLexicalProposalOrigin, LanguageLexicalProposalQuery, LanguageLexicalProposedTape,
    LanguageLexicalProposerDefinition, LanguageLexicalReviewedOrigin, LanguageLexicalTokenProposal,
    LanguageLexicalUnknownCommitPolicy, LanguageLexicalUnknownOrigin, LanguageLexicalUnknownPolicy,
};

pub mod discourse;

pub mod prosody;

pub use generated::{
    LanguageParserBufferGrowth, LanguageParserLegalFacts, LanguageParserLegalMask,
    LanguageParserMaskQuery, LanguageParserModelFeatures, LanguageParserPosEvidence,
    LanguageParserScoreContext, LanguageParserScoredClass, LanguageParserScoredProposal,
    LanguageParserScorerContext, LanguageParserScorerQuery,
};

pub use generated::{
    LanguageParserJointBeam, LanguageParserJointChoiceQuery, LanguageParserJointHypothesis,
    LanguageParserJointLexical, LanguageParserJointPosContext,
};

pub use generated::{LanguageParserJointMerge, LanguageParserJointRawBeam};
pub mod prepared_stable_lexical_fact;
pub mod pronunciation_selection;
pub mod stable_lexical_selection;

pub use generated::{
    LanguagePronunciationArcTarget, LanguagePronunciationCandidateQuery,
    LanguagePronunciationCandidateResult, LanguagePronunciationPosResult,
    LanguagePronunciationSelectionProfile, LanguagePronunciationSelectionRequest,
    LanguagePronunciationSelectionRule,
};

pub use generated::{
    LanguageParserJointBranchQuery, LanguageParserJointBranchResult, LanguageParserJointExpansion,
    LanguageParserJointRuntimeBeam, LanguageParserJointRuntimeHypothesis,
    LanguageParserJointRuntimeMerge, LanguageParserJointRuntimeRawBeam,
    LanguageParserJointRuntimeRawHypothesis, LanguageParserRawJointHypothesis,
};

pub use generated::{
    LanguageParserAvailability, LanguageParserAvailableGrowth, LanguageParserAvailableLexical,
    LanguageParserAvailableMask, LanguageParserAvailableState, LanguageParserWaitState,
};

pub use generated::{
    LanguageParserV2ChoiceQuery, LanguageParserV2FeaturesContext, LanguageParserV2ModelFeatures,
    LanguageParserV2PosContext,
};

pub use generated::{
    LanguageParserRevisionContext, LanguageParserRevisionRefusal, LanguageParserRevisionResult,
};

// Independent finite eight-token structural profile.
pub use generated::{
    LanguageParserWindow8Ancestry, LanguageParserWindow8Begin,
    LanguageParserWindow8CheckedHypothesis, LanguageParserWindow8FactQuery,
    LanguageParserWindow8RawState, LanguageParserWindow8RawWalk, LanguageParserWindow8RootCount,
    LanguageParserWindow8Snapshot, LanguageParserWindow8StableLexicalFact,
    LanguageParserWindow8StableLexicalFactProposal, LanguageParserWindow8StateProof,
    LanguageParserWindow8WalkQuery,
};
pub use generated::{
    LanguageParserWindow8Available, LanguageParserWindow8ChoiceQuery,
    LanguageParserWindow8ClassQuery, LanguageParserWindow8CodeQuery,
    LanguageParserWindow8Completion, LanguageParserWindow8Features, LanguageParserWindow8Lexical,
    LanguageParserWindow8Ordinal, LanguageParserWindow8RawAdvance, LanguageParserWindow8RawBeam,
    LanguageParserWindow8RawClass, LanguageParserWindow8RawClassContext,
    LanguageParserWindow8RawClassIndex, LanguageParserWindow8RawClassRelations,
    LanguageParserWindow8RawCodes, LanguageParserWindow8RawContext,
    LanguageParserWindow8RawFeatureContext, LanguageParserWindow8RawFeatureQuery,
    LanguageParserWindow8RawHypothesis, LanguageParserWindow8RawMerge,
    LanguageParserWindow8RawModelFeatures, LanguageParserWindow8RawProjection,
    LanguageParserWindow8RawRequest, LanguageParserWindow8RawResult, LanguageParserWindow8Selected,
};
// Distinct proposer-backed feature ABI; legacy Window8 features remain unchanged.
pub use generated::{
    LanguageParserProposalWindow8FeatureContext, LanguageParserProposalWindow8FeatureQuery,
    LanguageParserProposalWindow8Features, LanguageParserProposalWindow8OriginQuery,
    LanguageParserProposalWindow8Origins, LanguageParserProposalWindow8RawFeatures,
    LanguageParserProposalWindow8RawOrigins, LanguageParserProposalWindow8V2Features,
    LanguageParserProposalWindow8V2RawFeatures,
};
pub mod parser_window8;
pub mod parser_window8_program_bank;
pub use generated::{LanguageParserJointConsensusObservation, LanguageParserJointConsensusQuery};

#[cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
pub mod parser_model_selection;

pub use generated::{LanguageParserJointScoreBandProposal, LanguageParserJointScoreBandQuery};

pub use generated::{LanguageParserJointStableFact, LanguageParserJointStableFactProposal};

pub use generated::{LanguageParserJointCommitProposal, LanguageParserJointCommitQuery};

pub use generated::LanguageParserCommittedDependencyAdmission;
pub use generated::{
    LanguageParserJointProtectedBranchQuery, LanguageParserJointRebaseContext,
    LanguageParserJointRebaseProposal, LanguageParserProtectedEdgeProposal,
    LanguageParserProtectedProjectionContext,
};

/// Full Source-admitted correlation of an independent parser fact and portable arc.
/// This retains stable fact custody; it does not imply playback commitment.
pub use generated::LanguageParserStableDependencyAdmission;
pub use generated::{
    LanguageParserIndependentBranchContext, LanguageParserIndependentMaskContext,
    LanguageParserIndependentMaskQuery, LanguageParserProtectedInsertContext,
    LanguageParserProtectedInsertStage, LanguageParserProtectedMaskResolved,
    LanguageParserProtectedSetProposal,
};

pub use generated::{
    LanguageParserIndependentProtectedAdmission, LanguageParserProtectedHypothesisCompatibility,
    LanguageParserProtectedSetRebaseContext, LanguageParserProtectionForest,
    LanguageParserProtectionForestProposal, LanguageParserProtectionForestQuery,
};
