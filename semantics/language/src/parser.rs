//! Schema registration for the bounded source-owned symbolic parser profile.
//! Raw numeric states and arc proposals require native admission before use.
pub fn parser_types() -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageParserJointRuntimeHypothesis",
            crate::LanguageParserJointRuntimeHypothesis::semantic_type()
                .expect("checked Language Type")
        ),
        (
            "LanguageParserJointRuntimeRawHypothesis",
            crate::LanguageParserJointRuntimeRawHypothesis::semantic_type()
                .expect("checked Language Type")
        ),
        (
            "LanguageParserRawJointHypothesis",
            crate::LanguageParserRawJointHypothesis::semantic_type()
                .expect("checked Language Type")
        ),
        (
            "LanguageParserJointBranchQuery",
            crate::LanguageParserJointBranchQuery::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointBranchResult",
            crate::LanguageParserJointBranchResult::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointExpansion",
            crate::LanguageParserJointExpansion::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointRuntimeRawBeam",
            crate::LanguageParserJointRuntimeRawBeam::semantic_type()
                .expect("checked Language Type")
        ),
        (
            "LanguageParserJointRuntimeMerge",
            crate::LanguageParserJointRuntimeMerge::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointRuntimeBeam",
            crate::LanguageParserJointRuntimeBeam::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointRawBeam",
            crate::LanguageParserJointRawBeam::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointMerge",
            crate::LanguageParserJointMerge::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserLegalFacts",
            crate::LanguageParserLegalFacts::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserMaskQuery",
            crate::LanguageParserMaskQuery::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserLegalMask",
            crate::LanguageParserLegalMask::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointLexical",
            crate::LanguageParserJointLexical::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointHypothesis",
            crate::LanguageParserJointHypothesis::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointBeam",
            crate::LanguageParserJointBeam::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointChoiceQuery",
            crate::LanguageParserJointChoiceQuery::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserJointPosContext",
            crate::LanguageParserJointPosContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserPosEvidence",
            crate::LanguageParserPosEvidence::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerQuery",
            crate::LanguageParserScorerQuery::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScorerContext",
            crate::LanguageParserScorerContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserModelFeatures",
            crate::LanguageParserModelFeatures::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScoredClass",
            crate::LanguageParserScoredClass::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScoredProposal",
            crate::LanguageParserScoredProposal::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserScoreContext",
            crate::LanguageParserScoreContext::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserBufferGrowth",
            crate::LanguageParserBufferGrowth::semantic_type().expect("checked Language Type")
        ),
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
        (
            "LanguageParserHypothesis",
            crate::LanguageParserHypothesis::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserRawHypothesis",
            crate::LanguageParserRawHypothesis::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserBeam",
            crate::LanguageParserBeam::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserPruneRequest",
            crate::LanguageParserPruneRequest::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserAgreementQuery",
            crate::LanguageParserAgreementQuery::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserEdgeVote",
            crate::LanguageParserEdgeVote::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserAgreement",
            crate::LanguageParserAgreement::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserFeatures",
            crate::LanguageParserFeatures::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserVotes",
            crate::LanguageParserVotes::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserVoteCheck",
            crate::LanguageParserVoteCheck::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserFrontier",
            crate::LanguageParserFrontier::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserRawFrontier",
            crate::LanguageParserRawFrontier::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserFrontierRefusal",
            crate::LanguageParserFrontierRefusal::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserAdvanceRequest",
            crate::LanguageParserAdvanceRequest::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserCommitRequest",
            crate::LanguageParserCommitRequest::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserFrontierResult",
            crate::LanguageParserFrontierResult::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserAdmittedBeam",
            crate::LanguageParserAdmittedBeam::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserAdmittedAgreementQuery",
            crate::LanguageParserAdmittedAgreementQuery::semantic_type()
                .expect("checked Language Type")
        ),
    ]
}
