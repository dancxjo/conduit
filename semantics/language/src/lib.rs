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

/// Four-token prefix availability; this does not stabilize or commit parser facts.
pub use generated::{
    LanguageParserAvailability, LanguageParserAvailableLexical, LanguageParserRawAvailability,
};
mod parser_availability;
pub use parser_availability::parser_availability_types;

pub mod pronunciation_selection;
pub use generated::{
    LanguagePronunciationArcTarget, LanguagePronunciationCandidateQuery,
    LanguagePronunciationCandidateResult, LanguagePronunciationPosResult,
    LanguagePronunciationSelectionProfile, LanguagePronunciationSelectionRequest,
    LanguagePronunciationSelectionRule,
};

pub mod lexical_consumer;
pub use generated::{
    LanguageLexicalConsumerBasis, LanguageLexicalConsumerFeatureMatch,
    LanguageLexicalConsumerInventory, LanguageLexicalConsumerVarietyMatch,
};

mod parser_revision;
pub use generated::LanguageParserRevisionRebaseRequest;
pub use parser_revision::*;

pub use generated::{LanguageParserBeamCommitRequest, LanguageParserBeamCommitResult};
pub const PARSER_BEAM_COMMIT_SOURCE: &str = include_str!("../parser_beam_commit.conduit");

/// Optional exact model admission; checked Source still owns parser decisions.
#[cfg(all(feature = "parser-model", target_has_atomic = "ptr"))]
pub mod parser_model;

mod parser_scorer;
pub use parser_scorer::*;

pub mod parser_commitment;

pub mod dependency_commitment;
pub mod prosody_commitment;
pub mod prosody_revision;
pub use generated::{
    LanguageCommittedDependencyAdmission, LanguageCommittedDependencyQuery,
    LanguageCommittedProsodyAdmission, LanguageDependencyExecutionContext,
    LanguageDependencyExecutionObservation, LanguageDependencyExecutionResult,
    LanguageProsodyDurationIntent, LanguageProsodyFocusIntent, LanguageProsodyFrontierRequest,
    LanguageProsodyFrontierResult, LanguageProsodyRateIntent, LanguageProsodySymbolicIntent,
    LanguageProsodySymbolicProfile,
};

pub mod analysis_session;

/// Explicit unaccepted candidate, never the default accepted model selection.
#[cfg(all(feature = "parser-model", target_has_atomic = "ptr"))]
pub mod parser_punctuation_model;

/// Opaque replay of the existing single-survivor checked revision rebase.
pub mod parser_rebase_replay;

pub mod analysis_epoch_initialization;
pub mod analysis_epoch_owner;
