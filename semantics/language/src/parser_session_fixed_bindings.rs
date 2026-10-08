//! Closed generated-Native dispatch for the fixed public Session factory.
//! Each whole Source endpoint Type is compared before assembling an ingress.
use crate::{
    parser_session_execution::{verification::PreparedSourceVerification, ParserSessionEntry},
    parser_session_fixed_ingress::{
        FixedRefusal, ParserSessionExecutor, PreparedParserFixedIngress,
    },
    *,
};
use alloc::rc::Rc;
use conduit_core::{Plan, StructuredInfoType};
use conduit_plot::rust_binding::PreparedNativeFamily;
use core::cell::RefCell;
pub(crate) fn prepare_fixed_binding<E: ParserSessionExecutor>(
    executor: E,
    entry: ParserSessionEntry,
    original_plan: Rc<Plan>,
    family: Rc<RefCell<PreparedNativeFamily>>,
    verifier: PreparedSourceVerification,
    input: &StructuredInfoType,
    output: &StructuredInfoType,
    maximum_port_encoding_requested_bytes: usize,
    target_contract: crate::parser_session_target_contract::ParserSessionTargetStorageContract,
    maximum_invocations: u32,
) -> Result<PreparedParserFixedIngress<E>, FixedRefusal<E::Error>> {
    macro_rules! prepare {
        ($input:ty, $output:ty) => {
            PreparedParserFixedIngress::from_prepared::<$input, $output>(
                executor,
                entry,
                original_plan,
                family,
                verifier,
                input,
                output,
                maximum_port_encoding_requested_bytes,
                target_contract,
                maximum_invocations,
            )
        };
    }
    match entry {
        ParserSessionEntry::Availability => prepare!(
            LanguageParserAvailableLexical,
            LanguageParserRawAvailability
        ),
        ParserSessionEntry::Completion => {
            prepare!(LanguageParserState, LanguageParserCompletionObservation)
        }
        ParserSessionEntry::IndependentBranch => prepare!(
            LanguageParserIndependentBranchContext,
            LanguageParserJointBranchResult
        ),
        ParserSessionEntry::IndependentCommit => prepare!(
            LanguageParserIndependentCommitRequest,
            LanguageParserIndependentCommitSetProposal
        ),
        ParserSessionEntry::IndependentCommitRebase => prepare!(
            LanguageParserIndependentCommitRebaseRequest,
            LanguageParserIndependentCommitSetProposal
        ),
        ParserSessionEntry::IndependentCommitRebaseSets => prepare!(
            LanguageParserProtectedRebaseReceipt,
            LanguageParserIndependentCommitRebaseSets
        ),
        ParserSessionEntry::IndependentMask => {
            prepare!(LanguageParserIndependentMaskQuery, LanguageParserLegalMask)
        }
        ParserSessionEntry::JointBranch => prepare!(
            LanguageParserJointBranchQuery,
            LanguageParserJointBranchResult
        ),
        ParserSessionEntry::Commit => prepare!(
            LanguageParserJointCommitQuery,
            LanguageParserJointCommitProposal
        ),
        ParserSessionEntry::JointConsensus => prepare!(
            LanguageParserJointConsensusQuery,
            LanguageParserJointConsensusObservation
        ),
        ParserSessionEntry::Expansion => prepare!(
            LanguageParserJointExpansion,
            LanguageParserJointRuntimeRawHypothesis
        ),
        ParserSessionEntry::Rebase => prepare!(
            LanguageParserJointRebaseContext,
            LanguageParserJointRebaseProposal
        ),
        ParserSessionEntry::Merge => prepare!(
            LanguageParserJointRuntimeMerge,
            LanguageParserJointRuntimeRawBeam
        ),
        ParserSessionEntry::JointScoreBand1000 => prepare!(
            LanguageParserJointScoreBandQuery,
            LanguageParserJointScoreBandProposal
        ),
        ParserSessionEntry::StableFact => prepare!(
            LanguageParserJointConsensusQuery,
            LanguageParserJointStableFactProposal
        ),
        ParserSessionEntry::LegalMask => prepare!(LanguageParserMaskQuery, LanguageParserLegalMask),
        ParserSessionEntry::ProtectedOriginEdge => prepare!(
            LanguageParserIndependentProtectedAdmission,
            LanguageParserProtectedEdgeProposal
        ),
        ParserSessionEntry::Initialize => prepare!(
            LanguageParserProtectedEdgeProposal,
            LanguageParserProtectedSetProposal
        ),
        ParserSessionEntry::ProtectedInsert => prepare!(
            LanguageParserProtectedInsertContext,
            LanguageParserProtectedSetProposal
        ),
        ParserSessionEntry::ProtectedRebase => prepare!(
            LanguageParserProtectedSetRebaseContext,
            LanguageParserProtectedSetProposal
        ),
        ParserSessionEntry::ProtectionForestProjection => prepare!(
            LanguageParserProtectionForestQuery,
            LanguageParserProtectionForestProposal
        ),
        ParserSessionEntry::RetainedCommitAnchor => prepare!(
            LanguageParserJointCommitQuery,
            LanguageParserRetainedCommitAnchorProposal
        ),
        ParserSessionEntry::RevisionReset => {
            prepare!(LanguageParserRevisionContext, LanguageParserRevisionResult)
        }
        ParserSessionEntry::ScoreProposal => {
            prepare!(LanguageParserScoredClass, LanguageParserScoredProposal)
        }
        ParserSessionEntry::Seed => prepare!(
            LanguageParserSessionSeedRequest,
            LanguageParserSessionSeedProposal
        ),
        ParserSessionEntry::Transition => prepare!(LanguageParserRequest, LanguageParserResult),
        ParserSessionEntry::V2ModelFeatures => {
            prepare!(LanguageParserV2ChoiceQuery, LanguageParserV2ModelFeatures)
        }
        ParserSessionEntry::V2Pos => {
            prepare!(LanguageParserV2ChoiceQuery, LanguageParserV2PosContext)
        }
        ParserSessionEntry::WaitState => {
            prepare!(LanguageParserAvailableState, LanguageParserRawWaitState)
        }
        ParserSessionEntry::DecodeComplete
        | ParserSessionEntry::V2FeatureIndices
        | ParserSessionEntry::V2ScoreObservation => Err(FixedRefusal::Entry),
    }
}
