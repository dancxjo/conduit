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
