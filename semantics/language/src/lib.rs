#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    AnnotationBundleFour, LanguageAnalysisRevisionId, LanguageAnalysisTokenRef,
    LanguageDependencyArc, LanguageDependencyHead, LanguageDependencyRelation,
    LanguageDependencySubtype, LanguageUniversalDependencyRelation, LinguisticAnnotation,
    LinguisticDependencyEdge, LinguisticDependencyRelation, LinguisticDerivationProvenance,
    LinguisticEvidence, LinguisticOffsetBasis, LinguisticOptionalText, LinguisticSegment,
    LinguisticSegmentKind, LinguisticToken, LinguisticTokenCategory, LinguisticTokenFeature,
    LinguisticTokenFeatureSlot, LinguisticTokenIdentity, LinguisticTokensFour, TextSpan,
};

mod catalog;
mod info;
mod reference;

pub use catalog::*;
pub use info::*;
pub use reference::*;

mod dependency;
pub use dependency::*;
