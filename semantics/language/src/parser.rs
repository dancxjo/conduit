//! Schema registration for the bounded source-owned symbolic parser profile.
//! Raw numeric states and arc proposals require native admission before use.
pub fn parser_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageParserSubtype",
            crate::LanguageParserSubtype::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserRelation",
            crate::LanguageParserRelation::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserBasis",
            crate::LanguageParserBasis::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserBegin",
            crate::LanguageParserBegin::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserState",
            crate::LanguageParserState::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserNumericState",
            crate::LanguageParserNumericState::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserAction",
            crate::LanguageParserAction::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserRequest",
            crate::LanguageParserRequest::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserContext",
            crate::LanguageParserContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserRefusal",
            crate::LanguageParserRefusal::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserResult",
            crate::LanguageParserResult::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserArcQuery",
            crate::LanguageParserArcQuery::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserArcProposal",
            crate::LanguageParserArcProposal::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserArcResult",
            crate::LanguageParserArcResult::semantic_type().expect("checked Language Type")
        ),
    ]
}
