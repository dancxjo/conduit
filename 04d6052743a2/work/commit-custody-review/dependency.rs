//! Native UD relation vocabulary, independent of parser or commitment state.
use crate::{
    LanguageAnalysisRevisionId, LanguageAnalysisTokenRef, LanguageDependencyArc,
    LanguageDependencyHead, LanguageDependencyRelation, LanguageDependencySubtype,
    LanguageUniversalDependencyRelation,
};
use conduit_core::StructuredInfoType;
pub const UNIVERSAL_DEPENDENCY_RELATION_TYPE: &str = "LanguageUniversalDependencyRelation";
pub const LANGUAGE_DEPENDENCY_RELATION_TYPE: &str = "LanguageDependencyRelation";
pub const DEPENDENCY_SUBTYPE_TYPE: &str = "LanguageDependencySubtype";
pub fn universal_dependency_relation_type() -> StructuredInfoType {
    LanguageUniversalDependencyRelation::semantic_type().expect("checked native UD bases")
}
pub fn language_dependency_relation_type() -> StructuredInfoType {
    LanguageDependencyRelation::semantic_type().expect("checked native UD relation")
}
pub fn dependency_subtype_type() -> StructuredInfoType {
    LanguageDependencySubtype::semantic_type().expect("checked native subtype suffix")
}

pub const ANALYSIS_REVISION_TYPE: &str = "LanguageAnalysisRevisionId";
pub const ANALYSIS_TOKEN_REF_TYPE: &str = "LanguageAnalysisTokenRef";
pub const DEPENDENCY_HEAD_TYPE: &str = "LanguageDependencyHead";
pub const DEPENDENCY_ARC_TYPE: &str = "LanguageDependencyArc";
pub fn analysis_revision_type() -> StructuredInfoType {
    LanguageAnalysisRevisionId::semantic_type().expect("checked analysis revision")
}
pub fn analysis_token_ref_type() -> StructuredInfoType {
    LanguageAnalysisTokenRef::semantic_type().expect("checked analysis reference")
}
pub fn dependency_head_type() -> StructuredInfoType {
    LanguageDependencyHead::semantic_type().expect("checked native head")
}
pub fn dependency_arc_type() -> StructuredInfoType {
    LanguageDependencyArc::semantic_type().expect("checked native arc")
}
