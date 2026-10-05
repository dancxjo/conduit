#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    AnnotationBundleFour, LanguageAnalysisRevisionId, LanguageAnalysisTokenRef, LanguageCoverage,
    LanguageDependencyArc, LanguageDependencyHead, LanguageDependencyRelation,
    LanguageDependencySubtype, LanguageExternalIdentity, LanguageId, LanguageMappingDeclaration,
    LanguageRequest, LanguageText, LanguageTextId, LanguageTextRange, LanguageTextReferenceMatch,
    LanguageTextRevisionId, LanguageTextSegmentKind, LanguageTextSegmentRef,
    LanguageUniversalDependencyRelation, LanguageVariety, LanguageVarietyPolicy,
    LinguisticAnnotation, LinguisticDependencyEdge, LinguisticDependencyRelation,
    LinguisticDerivationProvenance, LinguisticEvidence, LinguisticOffsetBasis,
    LinguisticOptionalText, LinguisticSegment, LinguisticSegmentKind, LinguisticSyntacticLinkKind,
    LinguisticToken, LinguisticTokenCategory, LinguisticTokenFeature, LinguisticTokenFeatureSlot,
    LinguisticTokenIdentity, LinguisticTokensFour, TextSpan, VarietyId,
};

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
