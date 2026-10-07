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
    LanguageTextRevision, LanguageTextRevisionId, LanguageTextSegmentKind, LanguageTextSegmentRef,
    LanguageUniversalDependencyRelation, LanguageVariety, LanguageVarietyPolicy,
    LanguageVocativeDiscourseAdmission, LanguageVocativeDiscourseFact, LinguisticAnnotation,
    LinguisticDependencyEdge, LinguisticDependencyRelation, LinguisticDerivationProvenance,
    LinguisticEvidence, LinguisticOffsetBasis, LinguisticOptionalText, LinguisticSegment,
    LinguisticSegmentKind, LinguisticSyntacticLinkKind, LinguisticToken, LinguisticTokenCategory,
    LinguisticTokenFeature, LinguisticTokenFeatureSlot, LinguisticTokenIdentity,
    LinguisticTokensFour, TextSpan, VarietyId,
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

pub mod lexical;

pub mod discourse;

pub mod prosody;

pub use generated::{
    LanguageParserBufferGrowth, LanguageParserModelFeatures, LanguageParserPosEvidence,
    LanguageParserScoreContext, LanguageParserScoredClass, LanguageParserScoredProposal,
    LanguageParserScorerContext, LanguageParserScorerQuery,
};

pub use generated::{
    LanguageParserJointBeam, LanguageParserJointChoiceQuery, LanguageParserJointHypothesis,
    LanguageParserJointLexical, LanguageParserJointPosContext, LanguageParserLegalFacts,
    LanguageParserLegalMask, LanguageParserMaskQuery,
};

pub use generated::{LanguageParserJointMerge, LanguageParserJointRawBeam};

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

mod revision_lineage;
pub use generated::LanguageTextRevisionLineage;
pub use revision_lineage::*;

pub use generated::{
    LanguageParserRevisionContext, LanguageParserRevisionRefusal, LanguageParserRevisionResult,
};

pub use generated::{LanguageParserJointConsensusObservation, LanguageParserJointConsensusQuery};

#[cfg(all(feature = "parser-model-selection", target_has_atomic = "ptr"))]
pub mod parser_model_selection;

pub use generated::{LanguageParserJointScoreBandProposal, LanguageParserJointScoreBandQuery};

pub use generated::{LanguageParserJointStableFact, LanguageParserJointStableFactProposal};

pub use generated::{LanguageParserJointCommitProposal, LanguageParserJointCommitQuery};

pub use generated::{
    LanguageParserJointProtectedBranchQuery, LanguageParserJointRebaseContext,
    LanguageParserJointRebaseProposal, LanguageParserProtectedEdgeProposal,
    LanguageParserProtectedProjectionContext,
};

pub use generated::{
    LanguageParserIndependentBranchContext, LanguageParserIndependentMaskContext,
    LanguageParserIndependentMaskQuery, LanguageParserProtectedInsertContext,
    LanguageParserProtectedInsertStage, LanguageParserProtectedMaskResolved,
    LanguageParserProtectedSetProposal,
};
/// Full Source-admitted correlation of an independent parser fact and portable arc.
/// This retains stable fact custody; it does not imply playback commitment.
pub use generated::LanguageParserStableDependencyAdmission;

pub use generated::{
    LanguageParserIndependentProtectedAdmission, LanguageParserProtectedHypothesisCompatibility,
    LanguageParserProtectedSetRebaseContext, LanguageParserProtectionForest,
    LanguageParserProtectionForestProposal, LanguageParserProtectionForestQuery,
};
