//! Opaque whole-input/output custody for the parser's fixed Source entries.
//!
//! The target still executes the ordinary retained Plan through its executor.
//! Exact Source re-evaluation is an admission check, not a second scheduler.
//! A receipt proves this one execution, never an arbitrary beam's ancestry.
use crate::parser_session_runtime::{
    ParserSourceExecutor, ParserSourceFlowLimits, ParserSourceFlowRefusal, PreparedParserSourceFlow,
};
use alloc::vec::Vec;
use conduit_plot::{
    rust_binding::{NativeBindingRefusal, NativeRustBinding},
    PreparedExpressionStorageRefusal,
};
#[path = "parser_session_verification.rs"]
pub(crate) mod verification;
pub use verification::ParserSessionVerificationReceipt;
use verification::{PreparedSourceVerification, VerificationRefusal};

/// Closed set: callers cannot substitute a program or an entry name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserSessionEntry {
    Availability,
    DecodeComplete,
    Completion,
    IndependentBranch,
    IndependentCommit,
    IndependentCommitRebase,
    IndependentCommitRebaseSets,
    IndependentMask,
    JointBranch,
    Commit,
    JointConsensus,
    Expansion,
    Rebase,
    Merge,
    JointScoreBand1000,
    StableFact,
    LegalMask,
    ProtectedOriginEdge,
    Initialize,
    ProtectedInsert,
    ProtectedRebase,
    ProtectionForestProjection,
    RetainedCommitAnchor,
    RevisionReset,
    ScoreProposal,
    Seed,
    Transition,
    V2ModelFeatures,
    V2Pos,
    WaitState,
}
impl ParserSessionEntry {
    pub fn name(self) -> &'static str {
        match self {
            Self::Availability => "language-parser-availability",
            Self::DecodeComplete => "language-parser-decode-complete",
            Self::Completion => "language-parser-session-complete",
            Self::IndependentBranch => "language-parser-independent-branch",
            Self::IndependentCommit => "language-parser-independent-commit",
            Self::IndependentCommitRebase => "language-parser-independent-commit-rebase",
            Self::IndependentCommitRebaseSets => "language-parser-independent-commit-rebase-sets",
            Self::IndependentMask => "language-parser-independent-mask",
            Self::JointBranch => "language-parser-joint-branch",
            Self::Commit => "language-parser-joint-commit",
            Self::JointConsensus => "language-parser-joint-consensus",
            Self::Expansion => "language-parser-joint-expansion",
            Self::Rebase => "language-parser-joint-rebase",
            Self::Merge => "language-parser-joint-runtime-merge",
            Self::JointScoreBand1000 => "language-parser-joint-score-band-1000",
            Self::StableFact => "language-parser-joint-stable-fact",
            Self::LegalMask => "language-parser-legal-mask",
            Self::ProtectedOriginEdge => "language-parser-protected-origin-edge",
            Self::Initialize => "language-parser-protected-set-initialize",
            Self::ProtectedInsert => "language-parser-protected-set-insert",
            Self::ProtectedRebase => "language-parser-protected-set-rebase",
            Self::ProtectionForestProjection => "language-parser-protection-forest-projection",
            Self::RetainedCommitAnchor => "language-parser-retained-commit-anchor",
            Self::RevisionReset => "language-parser-revision-reset",
            Self::ScoreProposal => "language-parser-score-proposal",
            Self::Seed => "language-parser-session-seed",
            Self::Transition => "language-parser-transition",
            Self::V2ModelFeatures => "language-parser-v2-model-features",
            Self::V2Pos => "language-parser-v2-pos",
            Self::WaitState => "language-parser-wait-state",
        }
    }
    pub(crate) fn program_hex(self) -> &'static str {
        macro_rules! generated {
            ($file:literal) => {
                include_str!(concat!(env!("OUT_DIR"), $file))
            };
        }
        match self {
            Self::Availability => generated!("/parser_availability.hex"),
            Self::DecodeComplete => generated!("/parser_decode_complete.hex"),
            Self::Completion => generated!("/parser_session_complete.hex"),
            Self::IndependentBranch => generated!("/parser_independent_branch.hex"),
            Self::IndependentCommit => generated!("/parser_independent_commit.hex"),
            Self::IndependentCommitRebase => generated!("/parser_independent_commit_rebase.hex"),
            Self::IndependentCommitRebaseSets => {
                generated!("/parser_independent_commit_rebase_sets.hex")
            }
            Self::IndependentMask => generated!("/parser_independent_mask.hex"),
            Self::JointBranch => generated!("/parser_joint_branch.hex"),
            Self::Commit => generated!("/parser_joint_commit.hex"),
            Self::JointConsensus => generated!("/parser_joint_consensus.hex"),
            Self::Expansion => generated!("/parser_joint_expansion.hex"),
            Self::Rebase => generated!("/parser_joint_rebase.hex"),
            Self::Merge => generated!("/parser_joint_runtime_merge.hex"),
            Self::JointScoreBand1000 => generated!("/parser_joint_score_band_1000.hex"),
            Self::StableFact => generated!("/parser_joint_stable_fact.hex"),
            Self::LegalMask => generated!("/parser_legal_mask.hex"),
            Self::ProtectedOriginEdge => generated!("/parser_protected_origin_edge.hex"),
            Self::Initialize => generated!("/parser_protected_set_initialize.hex"),
            Self::ProtectedInsert => generated!("/parser_protected_set_insert.hex"),
            Self::ProtectedRebase => generated!("/parser_protected_set_rebase.hex"),
            Self::ProtectionForestProjection => {
                generated!("/parser_protection_forest_projection.hex")
            }
            Self::RetainedCommitAnchor => generated!("/parser_retained_commit_anchor.hex"),
            Self::RevisionReset => generated!("/parser_revision_reset.hex"),
            Self::ScoreProposal => generated!("/parser_score_proposal.hex"),
            Self::Seed => generated!("/parser_session_seed.hex"),
            Self::Transition => generated!("/parser_transition.hex"),
            Self::V2ModelFeatures => generated!("/parser_v2_model_features.hex"),
            Self::V2Pos => generated!("/parser_v2_pos.hex"),
            Self::WaitState => generated!("/parser_wait_state.hex"),
        }
    }
    pub(crate) fn chain_custody(self) -> &'static str {
        macro_rules! generated {
            ($file:literal) => {
                include_str!(concat!(env!("OUT_DIR"), $file))
            };
        }
        match self {
            Self::Availability => generated!("/parser_availability.custody"),
            Self::DecodeComplete => generated!("/parser_decode_complete.custody"),
            Self::Completion => generated!("/parser_session_complete.custody"),
            Self::IndependentBranch => generated!("/parser_independent_branch.custody"),
            Self::IndependentCommit => generated!("/parser_independent_commit.custody"),
            Self::IndependentCommitRebase => {
                generated!("/parser_independent_commit_rebase.custody")
            }
            Self::IndependentCommitRebaseSets => {
                generated!("/parser_independent_commit_rebase_sets.custody")
            }
            Self::IndependentMask => generated!("/parser_independent_mask.custody"),
            Self::JointBranch => generated!("/parser_joint_branch.custody"),
            Self::Commit => generated!("/parser_joint_commit.custody"),
            Self::JointConsensus => generated!("/parser_joint_consensus.custody"),
            Self::Expansion => generated!("/parser_joint_expansion.custody"),
            Self::Rebase => generated!("/parser_joint_rebase.custody"),
            Self::Merge => generated!("/parser_joint_runtime_merge.custody"),
            Self::JointScoreBand1000 => generated!("/parser_joint_score_band_1000.custody"),
            Self::StableFact => generated!("/parser_joint_stable_fact.custody"),
            Self::LegalMask => generated!("/parser_legal_mask.custody"),
            Self::ProtectedOriginEdge => generated!("/parser_protected_origin_edge.custody"),
            Self::Initialize => generated!("/parser_protected_set_initialize.custody"),
            Self::ProtectedInsert => generated!("/parser_protected_set_insert.custody"),
            Self::ProtectedRebase => generated!("/parser_protected_set_rebase.custody"),
            Self::ProtectionForestProjection => {
                generated!("/parser_protection_forest_projection.custody")
            }
            Self::RetainedCommitAnchor => generated!("/parser_retained_commit_anchor.custody"),
            Self::RevisionReset => generated!("/parser_revision_reset.custody"),
            Self::ScoreProposal => generated!("/parser_score_proposal.custody"),
            Self::Seed => generated!("/parser_session_seed.custody"),
            Self::Transition => generated!("/parser_transition.custody"),
            Self::V2ModelFeatures => generated!("/parser_v2_model_features.custody"),
            Self::V2Pos => generated!("/parser_v2_pos.custody"),
            Self::WaitState => generated!("/parser_wait_state.custody"),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct ParserSessionVerificationLimits {
    pub decoded_program_bytes: usize,
    pub preparation_peak_bytes: usize,
    pub retained_bytes: usize,
}
#[derive(Debug)]
pub enum ParserSessionExecutionRefusal<E> {
    Entry,
    Program,
    Ports,
    VerificationStorage(PreparedExpressionStorageRefusal),
    Verification,
    DifferentOutput,
    Native(NativeBindingRefusal),
    Flow(ParserSourceFlowRefusal<E>),
}
/// No public constructor: individually valid Native values cannot forge a run.
pub struct ParserSessionExecution<I, O> {
    entry: ParserSessionEntry,
    ordinal: u64,
    input: I,
    output: O,
    input_bytes: Vec<u8>,
    output_bytes: Vec<u8>,
}
impl<I, O> ParserSessionExecution<I, O> {
    pub(crate) fn from_verified_canonical(
        entry: ParserSessionEntry,
        ordinal: u64,
        input: I,
        output: O,
        input_bytes: Vec<u8>,
        output_bytes: Vec<u8>,
    ) -> Self {
        Self {
            entry,
            ordinal,
            input,
            output,
            input_bytes,
            output_bytes,
        }
    }

    pub(crate) fn into_parts(self) -> (ParserSessionEntry, u64, I, O, Vec<u8>, Vec<u8>) {
        (
            self.entry,
            self.ordinal,
            self.input,
            self.output,
            self.input_bytes,
            self.output_bytes,
        )
    }
    pub fn retained_frame_capacity_bytes(&self) -> Option<usize> {
        self.input_bytes
            .capacity()
            .checked_add(self.output_bytes.capacity())
    }
    pub fn source_custody(&self) -> &'static str {
        self.entry.chain_custody()
    }
    pub fn source_programs(&self) -> &'static str {
        self.entry.program_hex()
    }
    pub fn entry(&self) -> ParserSessionEntry {
        self.entry
    }
    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }
    pub fn input(&self) -> &I {
        &self.input
    }
    pub fn output(&self) -> &O {
        &self.output
    }
    pub fn input_bytes(&self) -> &[u8] {
        &self.input_bytes
    }
    pub fn output_bytes(&self) -> &[u8] {
        &self.output_bytes
    }
}
/// Owned typed executor plus the exact fixed Source verifier prepared once.
/// Native conversion and retained receipt storage need their own Session quota;
/// the verifier's receipt does not purport to bound those other allocations.
pub struct PreparedParserSessionPort<I, O, E> {
    entry: ParserSessionEntry,
    flow: PreparedParserSourceFlow<I, O, E>,
    verifier: PreparedSourceVerification,
    verification_storage: ParserSessionVerificationReceipt,
}
impl<I: NativeRustBinding + Clone, O: NativeRustBinding + Clone, E: ParserSourceExecutor>
    PreparedParserSessionPort<I, O, E>
{
    pub fn prepare(
        entry: ParserSessionEntry,
        executor: E,
        flow_limits: ParserSourceFlowLimits,
        storage: ParserSessionVerificationLimits,
    ) -> Result<Self, ParserSessionExecutionRefusal<E::Error>> {
        use ParserSessionExecutionRefusal as R;
        if executor.entry() != entry.name() {
            return Err(R::Entry);
        }
        let (verifier, input_type, output_type, verification_storage) =
            PreparedSourceVerification::prepare(entry, storage).map_err(|error| match error {
                VerificationRefusal::Program => R::Program,
                VerificationRefusal::Ports => R::Ports,
                VerificationRefusal::Storage(value) => R::VerificationStorage(value),
            })?;
        if I::semantic_type().map_err(R::Native)? != input_type
            || O::semantic_type().map_err(R::Native)? != output_type
        {
            return Err(R::Ports);
        }
        let flow = PreparedParserSourceFlow::new(executor, flow_limits).map_err(R::Flow)?;
        Ok(Self {
            entry,
            flow,
            verifier,
            verification_storage,
        })
    }
    pub fn verification_storage(&self) -> ParserSessionVerificationReceipt {
        self.verification_storage
    }
    pub fn cancel(&mut self) {
        self.flow.cancel();
    }
    pub fn is_cancelled(&self) -> bool {
        self.flow.is_cancelled()
    }
    pub fn next_ordinal(&self) -> u64 {
        self.flow.next_ordinal()
    }
    /// The aggregate owner must reserve its complete receipt before this call.
    /// Late verification/encoding refusal permanently closes the consumed port.
    pub(crate) fn execute(
        &mut self,
        input: I,
    ) -> Result<ParserSessionExecution<I, O>, ParserSessionExecutionRefusal<E::Error>> {
        use ParserSessionExecutionRefusal as R;
        let input_bytes = input.clone().encode().map_err(R::Native)?;
        let ordinal = self.flow.next_ordinal();
        let output = self.flow.transact(input.clone()).map_err(R::Flow)?;
        let result = (|| {
            let output_bytes = output.clone().encode().map_err(R::Native)?;
            let expected = self
                .verifier
                .evaluate(&input_bytes)
                .map_err(|_| R::Verification)?;
            if expected != output_bytes {
                return Err(R::DifferentOutput);
            }
            Ok(ParserSessionExecution {
                entry: self.entry,
                ordinal,
                input,
                output,
                input_bytes,
                output_bytes,
            })
        })();
        if result.is_err() {
            self.flow.cancel();
        }
        result
    }
}
