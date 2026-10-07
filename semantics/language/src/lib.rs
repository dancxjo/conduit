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
pub mod pronunciation_selection;

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
    LanguageParserWindow8Ancestry, LanguageParserWindow8Begin, LanguageParserWindow8RawState,
    LanguageParserWindow8RawWalk, LanguageParserWindow8RootCount, LanguageParserWindow8StateProof,
    LanguageParserWindow8WalkQuery,
};
pub mod parser_window8;
