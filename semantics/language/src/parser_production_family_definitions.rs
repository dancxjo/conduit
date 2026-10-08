//! Closed complete Native descriptor families for production Source ports.
//! Descriptor readiness confers no graph ancestry or model execution authority.
use crate::*;
use conduit_plot::rust_binding::{NativeFamilyTypeDescriptor, PreparedNativeRustBinding};
#[derive(Clone, Copy, Debug)]
pub(crate) enum ProductionFamily {
    Derivation,
    Lifecycle,
    StabilizeCommit,
    OriginsRebase,
    BranchMask,
    IndependentCommit,
}
pub(crate) const ALL_FAMILIES: [ProductionFamily; 6] = [
    ProductionFamily::Derivation,
    ProductionFamily::Lifecycle,
    ProductionFamily::StabilizeCommit,
    ProductionFamily::OriginsRebase,
    ProductionFamily::BranchMask,
    ProductionFamily::IndependentCommit,
];
impl ProductionFamily {
    pub(crate) fn roots(self) -> &'static [&'static NativeFamilyTypeDescriptor] {
        match self {
            Self::Derivation => &[
                LanguageDependencyArc::PREPARED_DESCRIPTOR,
                LanguageParserJointBranchQuery::PREPARED_DESCRIPTOR,
                LanguageParserJointBranchResult::PREPARED_DESCRIPTOR,
                LanguageParserJointExpansion::PREPARED_DESCRIPTOR,
                LanguageParserJointRuntimeBeam::PREPARED_DESCRIPTOR,
                LanguageParserJointRuntimeMerge::PREPARED_DESCRIPTOR,
                LanguageParserLegalMask::PREPARED_DESCRIPTOR,
                LanguageParserMaskQuery::PREPARED_DESCRIPTOR,
                LanguageParserScoredClass::PREPARED_DESCRIPTOR,
                LanguageParserScoredProposal::PREPARED_DESCRIPTOR,
                LanguageParserSessionSeedProposal::PREPARED_DESCRIPTOR,
                LanguageParserSessionSeedRequest::PREPARED_DESCRIPTOR,
                LanguageParserV2ModelFeatures::PREPARED_DESCRIPTOR,
                LanguageParserV2ModelScores::PREPARED_DESCRIPTOR,
                LanguageParserV2PosContext::PREPARED_DESCRIPTOR,
            ],
            Self::Lifecycle => &[
                LanguageParserCompletionObservation::PREPARED_DESCRIPTOR,
                LanguageParserAvailability::PREPARED_DESCRIPTOR,
                LanguageParserJointRebaseContext::PREPARED_DESCRIPTOR,
                LanguageParserJointRebaseProposal::PREPARED_DESCRIPTOR,
                LanguageParserRawAvailability::PREPARED_DESCRIPTOR,
                LanguageParserRawWaitState::PREPARED_DESCRIPTOR,
                LanguageParserRevisionContext::PREPARED_DESCRIPTOR,
                LanguageParserRevisionResult::PREPARED_DESCRIPTOR,
                LanguageParserWaitState::PREPARED_DESCRIPTOR,
            ],
            Self::StabilizeCommit => &[
                LanguageParserJointCommitProposal::PREPARED_DESCRIPTOR,
                LanguageParserJointCommitQuery::PREPARED_DESCRIPTOR,
                LanguageParserJointConsensusObservation::PREPARED_DESCRIPTOR,
                LanguageParserJointRuntimeRawBeam::PREPARED_DESCRIPTOR,
                LanguageParserJointScoreBandProposal::PREPARED_DESCRIPTOR,
                LanguageParserJointScoreBandQuery::PREPARED_DESCRIPTOR,
                LanguageParserJointStableFactProposal::PREPARED_DESCRIPTOR,
                LanguageParserRetainedCommitReceipt::PREPARED_DESCRIPTOR,
                LanguageParserRetainedFactReceipt::PREPARED_DESCRIPTOR,
                LanguageParserRetainedSnapshotReceipt::PREPARED_DESCRIPTOR,
                LanguageParserStableDependencyAdmission::PREPARED_DESCRIPTOR,
            ],
            Self::OriginsRebase => &[
                LanguageParserIndependentProtectedAdmission::PREPARED_DESCRIPTOR,
                LanguageParserProtectedRebaseReceipt::PREPARED_DESCRIPTOR,
                LanguageParserProtectedInitialReceipt::PREPARED_DESCRIPTOR,
                LanguageParserProtectedInitializationReceipt::PREPARED_DESCRIPTOR,
                LanguageParserProtectedInsertReceipt::PREPARED_DESCRIPTOR,
                LanguageParserProtectedOriginCorrelation::PREPARED_DESCRIPTOR,
            ],
            Self::BranchMask => &[
                LanguageParserIndependentBranchContext::PREPARED_DESCRIPTOR,
                LanguageParserIndependentMaskQuery::PREPARED_DESCRIPTOR,
                LanguageParserJointBranchResult::PREPARED_DESCRIPTOR,
                LanguageParserProtectedHypothesisCompatibility::PREPARED_DESCRIPTOR,
                LanguageParserProtectionForestProposal::PREPARED_DESCRIPTOR,
            ],
            Self::IndependentCommit => &[
                LanguageParserIndependentCommitRebaseRequest::PREPARED_DESCRIPTOR,
                LanguageParserIndependentCommitRequest::PREPARED_DESCRIPTOR,
                LanguageParserIndependentCommitSetProposal::PREPARED_DESCRIPTOR,
                LanguageParserIndependentProtectedAdmission::PREPARED_DESCRIPTOR,
                LanguageParserJointCommitQuery::PREPARED_DESCRIPTOR,
                LanguageParserProtectedRebaseReceipt::PREPARED_DESCRIPTOR,
                LanguageParserRetainedCommitAnchorProposal::PREPARED_DESCRIPTOR,
                LanguageParserRetainedCommitReceipt::PREPARED_DESCRIPTOR,
            ],
        }
    }
}
pub(crate) fn port_descriptors(
    entry: crate::parser_session_execution::ParserSessionEntry,
) -> Option<(
    &'static NativeFamilyTypeDescriptor,
    &'static NativeFamilyTypeDescriptor,
)> {
    use crate::parser_session_execution::ParserSessionEntry as E;
    match entry {
        E::Availability => Some((
            LanguageParserAvailableLexical::PREPARED_DESCRIPTOR,
            LanguageParserRawAvailability::PREPARED_DESCRIPTOR,
        )),
        E::Completion => Some((
            LanguageParserState::PREPARED_DESCRIPTOR,
            LanguageParserCompletionObservation::PREPARED_DESCRIPTOR,
        )),
        E::V2FeatureIndices | E::V2ScoreObservation => None, // Mixed custody checks bare numeric ports.
        E::DecodeComplete => None, // Primitive output needs the separate Source completion adapter.
        E::IndependentBranch => Some((
            LanguageParserIndependentBranchContext::PREPARED_DESCRIPTOR,
            LanguageParserJointBranchResult::PREPARED_DESCRIPTOR,
        )),
        E::IndependentCommit => Some((
            LanguageParserIndependentCommitRequest::PREPARED_DESCRIPTOR,
            LanguageParserIndependentCommitSetProposal::PREPARED_DESCRIPTOR,
        )),
        E::IndependentCommitRebase => Some((
            LanguageParserIndependentCommitRebaseRequest::PREPARED_DESCRIPTOR,
            LanguageParserIndependentCommitSetProposal::PREPARED_DESCRIPTOR,
        )),
        E::IndependentCommitRebaseSets => Some((
            LanguageParserProtectedRebaseReceipt::PREPARED_DESCRIPTOR,
            LanguageParserIndependentCommitRebaseSets::PREPARED_DESCRIPTOR,
        )),
        E::IndependentMask => Some((
            LanguageParserIndependentMaskQuery::PREPARED_DESCRIPTOR,
            LanguageParserLegalMask::PREPARED_DESCRIPTOR,
        )),
        E::JointBranch => Some((
            LanguageParserJointBranchQuery::PREPARED_DESCRIPTOR,
            LanguageParserJointBranchResult::PREPARED_DESCRIPTOR,
        )),
        E::Commit => Some((
            LanguageParserJointCommitQuery::PREPARED_DESCRIPTOR,
            LanguageParserJointCommitProposal::PREPARED_DESCRIPTOR,
        )),
        E::JointConsensus => Some((
            LanguageParserJointConsensusQuery::PREPARED_DESCRIPTOR,
            LanguageParserJointConsensusObservation::PREPARED_DESCRIPTOR,
        )),
        E::Expansion => Some((
            LanguageParserJointExpansion::PREPARED_DESCRIPTOR,
            LanguageParserJointRuntimeRawHypothesis::PREPARED_DESCRIPTOR,
        )),
        E::Rebase => Some((
            LanguageParserJointRebaseContext::PREPARED_DESCRIPTOR,
            LanguageParserJointRebaseProposal::PREPARED_DESCRIPTOR,
        )),
        E::Merge => Some((
            LanguageParserJointRuntimeMerge::PREPARED_DESCRIPTOR,
            LanguageParserJointRuntimeRawBeam::PREPARED_DESCRIPTOR,
        )),
        E::JointScoreBand1000 => Some((
            LanguageParserJointScoreBandQuery::PREPARED_DESCRIPTOR,
            LanguageParserJointScoreBandProposal::PREPARED_DESCRIPTOR,
        )),
        E::StableFact => Some((
            LanguageParserJointConsensusQuery::PREPARED_DESCRIPTOR,
            LanguageParserJointStableFactProposal::PREPARED_DESCRIPTOR,
        )),
        E::LegalMask => Some((
            LanguageParserMaskQuery::PREPARED_DESCRIPTOR,
            LanguageParserLegalMask::PREPARED_DESCRIPTOR,
        )),
        E::ProtectedOriginEdge => Some((
            LanguageParserIndependentProtectedAdmission::PREPARED_DESCRIPTOR,
            LanguageParserProtectedEdgeProposal::PREPARED_DESCRIPTOR,
        )),
        E::Initialize => Some((
            LanguageParserProtectedEdgeProposal::PREPARED_DESCRIPTOR,
            LanguageParserProtectedSetProposal::PREPARED_DESCRIPTOR,
        )),
        E::ProtectedInsert => Some((
            LanguageParserProtectedInsertContext::PREPARED_DESCRIPTOR,
            LanguageParserProtectedSetProposal::PREPARED_DESCRIPTOR,
        )),
        E::ProtectedRebase => Some((
            LanguageParserProtectedSetRebaseContext::PREPARED_DESCRIPTOR,
            LanguageParserProtectedSetProposal::PREPARED_DESCRIPTOR,
        )),
        E::ProtectionForestProjection => Some((
            LanguageParserProtectionForestQuery::PREPARED_DESCRIPTOR,
            LanguageParserProtectionForestProposal::PREPARED_DESCRIPTOR,
        )),
        E::RetainedCommitAnchor => Some((
            LanguageParserJointCommitQuery::PREPARED_DESCRIPTOR,
            LanguageParserRetainedCommitAnchorProposal::PREPARED_DESCRIPTOR,
        )),
        E::RevisionReset => Some((
            LanguageParserRevisionContext::PREPARED_DESCRIPTOR,
            LanguageParserRevisionResult::PREPARED_DESCRIPTOR,
        )),
        E::ScoreProposal => Some((
            LanguageParserScoredClass::PREPARED_DESCRIPTOR,
            LanguageParserScoredProposal::PREPARED_DESCRIPTOR,
        )),
        E::Seed => Some((
            LanguageParserSessionSeedRequest::PREPARED_DESCRIPTOR,
            LanguageParserSessionSeedProposal::PREPARED_DESCRIPTOR,
        )),
        E::Transition => Some((
            LanguageParserRequest::PREPARED_DESCRIPTOR,
            LanguageParserResult::PREPARED_DESCRIPTOR,
        )),
        E::V2ModelFeatures => Some((
            LanguageParserV2ChoiceQuery::PREPARED_DESCRIPTOR,
            LanguageParserV2ModelFeatures::PREPARED_DESCRIPTOR,
        )),
        E::V2Pos => Some((
            LanguageParserV2ChoiceQuery::PREPARED_DESCRIPTOR,
            LanguageParserV2PosContext::PREPARED_DESCRIPTOR,
        )),
        E::WaitState => Some((
            LanguageParserAvailableState::PREPARED_DESCRIPTOR,
            LanguageParserRawWaitState::PREPARED_DESCRIPTOR,
        )),
    }
}
