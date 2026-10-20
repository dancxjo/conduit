//! Hosted compatibility facade over the one retained canonical round traversal.
use super::*;
use alloc::sync::Arc;
use conduit_ai::integer_categorical_step::{
    channel::DispatchedCategoricalInvocation, PreparedCategoricalStep,
};
pub const MAX_LEARNED_REQUESTS_PER_ROUND: usize = 16;
pub const MAX_LEARNED_REQUESTS_PER_EPOCH: u64 = 128;
pub const MAX_KERNEL_POLLS_PER_REQUEST: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserRoundModel {
    AcceptedEnglishV2,
    UnacceptedPunctuationV3,
}
pub type PreparedLearnedRoundChannel = cursor::numerical::PreparedCursorLearnedChannel;
pub struct PendingParserScore {
    pub(super) context: [String; 5],
    pub(super) generation: u64,
    pub(super) prior: LanguageParserScorerBeam,
    pub(super) slot: usize,
    pub(super) choice: u64,
    pub(super) ordinal: usize,
    pub(super) query: LanguageParserScorerChoiceQuery,
    pub(super) features: LanguageParserScorerFeatures,
    pub(super) input: Vec<u8>,
}
impl PendingParserScore {
    pub fn query(&self) -> &LanguageParserScorerChoiceQuery {
        &self.query
    }
    pub fn features(&self) -> &LanguageParserScorerFeatures {
        &self.features
    }
    pub fn input_canonical(&self) -> &[u8] {
        &self.input
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn context(&self) -> &[String; 5] {
        &self.context
    }
    pub fn prior(&self) -> &LanguageParserScorerBeam {
        &self.prior
    }
    pub fn survivor_slot(&self) -> usize {
        self.slot
    }
    pub fn lexical_choice(&self) -> u64 {
        self.choice
    }
    pub fn request_ordinal(&self) -> usize {
        self.ordinal
    }
}
pub struct LearnedBranchEvidence {
    pub(super) request: PendingParserScore,
    pub(super) numerical: DispatchedCategoricalInvocation,
    pub(super) selection: ParserRoundModel,
    pub(super) profile: Arc<PreparedCategoricalStep>,
    pub(super) definition: Arc<conduit_composite::KernelCompositeDefinition>,
}
impl LearnedBranchEvidence {
    pub fn request(&self) -> &PendingParserScore {
        &self.request
    }
    pub fn numerical(&self) -> &DispatchedCategoricalInvocation {
        &self.numerical
    }
    pub fn selection(&self) -> ParserRoundModel {
        self.selection
    }
    pub fn definition(&self) -> &conduit_composite::KernelCompositeDefinition {
        &self.definition
    }
    pub fn profile(&self) -> &PreparedCategoricalStep {
        &self.profile
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
}
pub struct LearnedRoundPreparationFailure {
    pub refusal: FallbackRoundRefusal,
    pub observations: Vec<FallbackOperationObservation>,
    pub learned: Vec<LearnedBranchEvidence>,
    /// Preserve opaque tokens even if hosted Native decoding itself refuses.
    pub retained_numerical: Vec<cursor::numerical::CursorNumericalEvidence>,
}
impl AnalysisEpochOwner<'_> {
    // Return retained numerical tokens/observations directly, including pressure failures.
    #[allow(clippy::result_large_err)]
    pub fn prepare_learned_round_with(
        &self,
        programs: &PreparedFallbackRoundPrograms,
        provider: &mut dyn FallbackObservationProvider,
        channel: &mut PreparedLearnedRoundChannel,
    ) -> Result<PreparedFallbackRound, LearnedRoundPreparationFailure> {
        cursor::hosted::learned(self, programs, provider, channel)
    }
}
