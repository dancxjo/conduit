//! Native UD relation vocabulary, independent of parser or commitment state.
use crate::{
    LanguageDependencyRelation, LanguageDependencySubtype, LanguageUniversalDependencyRelation,
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
