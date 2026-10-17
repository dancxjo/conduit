//! Checked finite scorer schemas; these retain grammar and search policy in Source.
pub use crate::generated::{
    LanguageParserScoreContext, LanguageParserScoredClass, LanguageParserScoredProposal,
    LanguageParserScorerBeam, LanguageParserScorerBranch, LanguageParserScorerBranchResult,
    LanguageParserScorerChoiceQuery, LanguageParserScorerExpansion,
    LanguageParserScorerExpansionContext, LanguageParserScorerFeatures,
    LanguageParserScorerFeaturesContext, LanguageParserScorerHypothesis,
    LanguageParserScorerLegality, LanguageParserScorerMerge, LanguageParserScorerPosContext,
    LanguageParserScorerRanking, LanguageParserScorerRawHypothesis, LanguageParserScorerSelected,
    LanguageParserScorerTournament1, LanguageParserScorerTournament10,
    LanguageParserScorerTournament19, LanguageParserScorerTournament2,
    LanguageParserScorerTournament3, LanguageParserScorerTournament38,
    LanguageParserScorerTournament5, LanguageParserScorerTournamentRow,
};
pub fn parser_scorer_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageParserScorerTournamentRow",
            LanguageParserScorerTournamentRow::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerTournament38",
            LanguageParserScorerTournament38::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerTournament19",
            LanguageParserScorerTournament19::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerTournament10",
            LanguageParserScorerTournament10::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerTournament5",
            LanguageParserScorerTournament5::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerTournament3",
            LanguageParserScorerTournament3::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerTournament2",
            LanguageParserScorerTournament2::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerTournament1",
            LanguageParserScorerTournament1::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerSelected",
            LanguageParserScorerSelected::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerLegality",
            LanguageParserScorerLegality::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerRanking",
            LanguageParserScorerRanking::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerChoiceQuery",
            LanguageParserScorerChoiceQuery::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerPosContext",
            LanguageParserScorerPosContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerFeaturesContext",
            LanguageParserScorerFeaturesContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerFeatures",
            LanguageParserScorerFeatures::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScoredClass",
            LanguageParserScoredClass::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScoredProposal",
            LanguageParserScoredProposal::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScoreContext",
            LanguageParserScoreContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerHypothesis",
            LanguageParserScorerHypothesis::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerRawHypothesis",
            LanguageParserScorerRawHypothesis::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerBranch",
            LanguageParserScorerBranch::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerBranchResult",
            LanguageParserScorerBranchResult::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerExpansion",
            LanguageParserScorerExpansion::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerExpansionContext",
            LanguageParserScorerExpansionContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerBeam",
            LanguageParserScorerBeam::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerMerge",
            LanguageParserScorerMerge::semantic_type().expect("checked Language Type")
        ),
    ]
}

/// Install the existing typed finite Zip (Selected, Result) before checking.
const PARSER_SCORER_TRANSITION_SOURCE: &str = include_str!("../parser_scorer_transition.conduit");

/// Frame the authored transition with the existing generic typed-pair encoder.
/// This computes transport bytes only; all linguistic policy stays in Source.
pub fn parser_scorer_transition_source() -> alloc::string::String {
    let pair = conduit_core::PreparedTypedTuplePairEncoder::new(
        LanguageParserScorerSelected::semantic_type().expect("checked Selected Type"),
        16384,
        crate::LanguageParserResult::semantic_type().expect("checked Result Type"),
        16384,
    )
    .expect("finite checked parser pair");
    assert_eq!(
        pair.maximum_bytes(),
        53414,
        "exact checked parser pair envelope"
    );
    alloc::string::String::from(PARSER_SCORER_TRANSITION_SOURCE)
}
