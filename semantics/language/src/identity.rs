//! Language-owned native Types shared with downstream semantic catalogs.
use crate::*;
use alloc::vec::Vec;
use conduit_core::StructuredInfoType;

pub fn identity_types() -> Vec<(&'static str, StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageMorphemeId",
            LanguageMorphemeId::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageMorphemeReference",
            LanguageMorphemeReference::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageExternalIdentity",
            LanguageExternalIdentity::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageId",
            LanguageId::semantic_type().expect("checked Language Type")
        ),
        (
            "VarietyId",
            VarietyId::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageVariety",
            LanguageVariety::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextId",
            LanguageTextId::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextRevisionId",
            LanguageTextRevisionId::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextRange",
            LanguageTextRange::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextSegmentKind",
            LanguageTextSegmentKind::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextSegmentRef",
            LanguageTextSegmentRef::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageText",
            LanguageText::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageTextReferenceMatch",
            LanguageTextReferenceMatch::semantic_type().expect("checked Language Type")
        ),
        (
            "LinguisticSyntacticLinkKind",
            LinguisticSyntacticLinkKind::semantic_type().expect("checked Language Type")
        ),
    ]
}
