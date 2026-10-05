//! #4952 report validation for an exact declared language identity mapping.
use crate::{LanguageExternalIdentity, LanguageMapping, LanguageMappingRefusal, LanguageSelection};
use conduit_core::projection::*;

pub struct LanguageMappingProjection<'a> {
    pub mapping: &'a LanguageMapping<'a>,
    pub law: ProjectionText<'a>,
}
impl ProjectionDomain for LanguageMappingProjection<'_> {
    type Source = LanguageExternalIdentity;
    type Target = LanguageSelection;
    type Detail = LanguageMappingRefusal;
    fn source_contract(&self) -> ProjectionText<'_> {
        ProjectionText::new("language/external-identity@1").expect("constant contract")
    }
    fn target_contract(&self) -> ProjectionText<'_> {
        ProjectionText::new("language/selection@1").expect("constant contract")
    }
    fn validate(
        &self,
        source: &Self::Source,
        target: Option<&Self::Target>,
        facts: &[ProjectionFact<'_, Self>],
        native: &[ProjectionNativeFact<'_>],
        scores: &[ProjectionScore<'_>],
        mechanism: ProjectionMechanism,
    ) -> bool {
        if mechanism != ProjectionMechanism::Completed
            || !scores.is_empty()
            || facts.len() != 1
            || native.len() != 1
        {
            return false;
        }
        let n = native[0];
        if n.identity.as_str() != "external-language"
            || n.contract.as_str() != source.contract()
            || n.provider.as_str() != source.contract()
            || n.encoding.as_str() != "utf8@1"
            || n.bytes != source.name().as_bytes()
            || facts[0].obligation().as_str() != "language-identity"
        {
            return false;
        }
        match (self.mapping.resolve(source), target, &facts[0]) {
            (Ok(expected), Some(actual), ProjectionFact::Transformed { law, .. }) => {
                expected == actual && law == &self.law
            }
            (
                Err(LanguageMappingRefusal::Undeclared),
                None,
                ProjectionFact::Lost {
                    class: ProjectionLoss::Unrecognized,
                    detail: LanguageMappingRefusal::Undeclared,
                    native_fact: Some(id),
                    ..
                },
            ) => id.as_str() == "external-language",
            _ => false,
        }
    }
}

pub struct ExactLanguageMappingPolicy;
impl ProjectionPolicy<LanguageMappingProjection<'_>> for ExactLanguageMappingPolicy {
    fn identity(&self) -> ProjectionText<'_> {
        ProjectionText::new("language/exact-mapping@1").expect("constant policy")
    }
    fn permits(
        &self,
        _: &LanguageExternalIdentity,
        _: &LanguageSelection,
        _: &[ProjectionFact<'_, LanguageMappingProjection<'_>>],
    ) -> bool {
        false
    }
}
