//! Bounded hosted learned preparation, not a resumable installed stream controller.
//! The one round traversal and existing Source keep every parser decision.
use super::*;
use alloc::{sync::Arc, vec};
use conduit_ai::integer_categorical_step::{channel::*, PreparedCategoricalStep};
use conduit_composite::KernelCompositeDefinition;
use conduit_core::{StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::{CheckedPlot, CheckedSyntaxDocument, ProfileCatalog};

pub const MAX_LEARNED_REQUESTS_PER_ROUND: usize = 16;
pub const MAX_LEARNED_REQUESTS_PER_EPOCH: u64 = 128;
/// Hosted preparation bound, never an instruction/fuel promise or busy wait.
pub const MAX_KERNEL_POLLS_PER_REQUEST: usize = 16;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserRoundModel {
    AcceptedEnglishV2,
    UnacceptedPunctuationV3,
}
/// Only the private shared traversal can mint this request lease.
pub struct PendingParserScore {
    context: [String; 5],
    generation: u64,
    prior: LanguageParserScorerBeam,
    slot: usize,
    choice: u64,
    ordinal: usize,
    query: LanguageParserScorerChoiceQuery,
    features: LanguageParserScorerFeatures,
    input: Vec<u8>,
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
    request: PendingParserScore,
    numerical: DispatchedCategoricalInvocation,
    selection: ParserRoundModel,
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
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
}
/// Creates its own fresh actual-kernel channel; no pre-executed-token entrance.
pub struct PreparedLearnedRoundChannel {
    numerical: PreparedCategoricalChannel<PendingParserScore>,
    profile: Arc<PreparedCategoricalStep>,
    selection: ParserRoundModel,
    next: u64,
    refused: Option<PendingParserScore>,
    stopped: bool,
    context: Option<[String; 5]>,
}
impl PreparedLearnedRoundChannel {
    pub fn prepare(
        plot: &CheckedPlot,
        definition: KernelCompositeDefinition,
        profile: Arc<PreparedCategoricalStep>,
        selection: ParserRoundModel,
        capacity: usize,
    ) -> Result<Self, FallbackRoundRefusal> {
        Self::validate_model(&profile, selection)?;
        let numerical =
            PreparedCategoricalChannel::prepare(plot, definition, profile.clone(), capacity)
                .map_err(|_| FallbackRoundRefusal::Model)?;
        Ok(Self::with_numerical(numerical, profile, selection))
    }
    pub fn prepare_canonical(
        document: &CheckedSyntaxDocument,
        entry: &str,
        catalog: &ProfileCatalog,
        definition: KernelCompositeDefinition,
        profile: Arc<PreparedCategoricalStep>,
        selection: ParserRoundModel,
        capacity: usize,
    ) -> Result<Self, FallbackRoundRefusal> {
        Self::validate_model(&profile, selection)?;
        let numerical = PreparedCategoricalChannel::prepare_canonical(
            document,
            entry,
            catalog,
            definition,
            profile.clone(),
            capacity,
        )
        .map_err(|_| FallbackRoundRefusal::Model)?;
        Ok(Self::with_numerical(numerical, profile, selection))
    }
    fn validate_model(
        profile: &PreparedCategoricalStep,
        selection: ParserRoundModel,
    ) -> Result<(), FallbackRoundRefusal> {
        use crate::{parser_model::*, parser_punctuation_model::*};
        let (artifact, signature, bytes) = match selection {
            ParserRoundModel::AcceptedEnglishV2 => {
                let (a, s) = english_parser_scorer_descriptor();
                (a, s, ENGLISH_PARSER_WEIGHTS)
            }
            ParserRoundModel::UnacceptedPunctuationV3 => {
                let (a, s) = punctuation_parser_scorer_descriptor();
                (a, s, PUNCTUATION_PARSER_WEIGHTS)
            }
        };
        let selected_signature = profile
            .resource()
            .signature()
            .semantic_digest()
            .map_err(|_| FallbackRoundRefusal::Model)?;
        let required_signature = signature
            .semantic_digest()
            .map_err(|_| FallbackRoundRefusal::Model)?;
        if profile.dimensions() != ENGLISH_PARSER_DIMENSIONS
            || profile.resource().artifact() != &artifact
            || selected_signature != required_signature
            || profile.resource().bytes() != bytes
        {
            return Err(FallbackRoundRefusal::Model);
        }
        Ok(())
    }
    fn with_numerical(
        numerical: PreparedCategoricalChannel<PendingParserScore>,
        profile: Arc<PreparedCategoricalStep>,
        selection: ParserRoundModel,
    ) -> Self {
        Self {
            numerical,
            profile,
            selection,
            next: 1,
            refused: None,
            stopped: false,
            context: None,
        }
    }
    /// Terminal wrapper state; it does not claim the underlying kernel was cancelled.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }
    pub fn pending(&self) -> Option<&PendingParserScore> {
        self.refused.as_ref().or_else(|| self.numerical.pending())
    }
    pub fn cancel(&mut self) -> Result<(), FallbackRoundRefusal> {
        self.stopped = true;
        self.numerical
            .cancel()
            .map_err(|_| FallbackRoundRefusal::Numerical)
    }
    pub fn numerical(&self) -> &PreparedCategoricalChannel<PendingParserScore> {
        &self.numerical
    }
}
pub struct LearnedRoundPreparationFailure {
    pub refusal: FallbackRoundRefusal,
    pub observations: Vec<FallbackOperationObservation>,
    pub learned: Vec<LearnedBranchEvidence>,
}
struct LearnedScores<'a> {
    channel: &'a mut PreparedLearnedRoundChannel,
    context: [String; 5],
    generation: u64,
    prior: LanguageParserScorerBeam,
    evidence: Vec<LearnedBranchEvidence>,
}
impl RoundScores for LearnedScores<'_> {
    fn rank(
        &mut self,
        replay: &mut Replay<'_>,
        request: RankRequest<'_>,
    ) -> Result<LanguageParserScorerRanking, FallbackRoundRefusal> {
        use FallbackRoundRefusal::*;
        if self.channel.stopped || self.channel.pending().is_some() {
            return Err(PendingNumerical);
        }
        if self.evidence.len() >= MAX_LEARNED_REQUESTS_PER_ROUND
            || self.channel.next > MAX_LEARNED_REQUESTS_PER_EPOCH
        {
            return Err(Bound);
        }
        let query = LanguageParserScorerChoiceQuery::new(
            *request.branch.choices(),
            *request.lexical.tape().source().sequence(),
            self.channel.next,
            request.lexical.clone(),
            request.branch.parser().state().clone(),
        )
        .map_err(Native)?;
        let features: LanguageParserScorerFeatures =
            replay.run("language-parser-scorer-model-features", &query)?;
        if features.basis() != query.state().basis()
            || features.epoch() != query.epoch()
            || features.invocation() != query.invocation()
        {
            return Err(ForeignObservation);
        }
        let input = encode_indices(&self.channel.profile, features.indices())?;
        let lease = PendingParserScore {
            context: self.context.clone(),
            generation: self.generation,
            prior: self.prior.clone(),
            slot: request.slot,
            choice: request.choice,
            ordinal: self.evidence.len(),
            query,
            features,
            input,
        };
        // Hosted preparation allocation, before actual dispatch; channel runtime stays fixed.
        let submitted_input = lease.input.clone();
        match self.channel.numerical.submit(lease, &submitted_input) {
            Ok(CategoricalChannelSubmission::Accepted) => (),
            Ok(CategoricalChannelSubmission::TerminalRetained(reason)) => {
                self.channel.stopped = true;
                return Err(NumericalChannel(reason));
            }
            Err(failure) => {
                let reason = failure.refusal;
                self.channel.refused = Some(failure.lease);
                self.channel.stopped = true;
                return Err(NumericalChannel(reason));
            }
        }
        for _ in 0..MAX_KERNEL_POLLS_PER_REQUEST {
            let completed = match self.channel.numerical.step() {
                Ok(Some(completed)) => completed,
                Ok(None) => continue,
                Err(reason) => {
                    self.channel.stopped = true;
                    return Err(NumericalChannel(reason));
                }
            };
            let (lease, numerical) = completed.into_parts();
            // The production channel returns exactly the moved private lease.
            // Retain even a rejected completion before any further admission.
            self.evidence.push(LearnedBranchEvidence {
                request: lease,
                numerical,
                selection: self.channel.selection,
            });
            let retained = self.evidence.last().ok_or(Numerical)?;
            let lease = &retained.request;
            if lease.context != self.context
                || lease.generation != self.generation
                || lease.prior != self.prior
                || lease.slot != request.slot
                || lease.choice != request.choice
                || lease.ordinal + 1 != self.evidence.len()
                || retained.numerical.invocation() != self.channel.next
            {
                self.channel.stopped = true;
                return Err(Stale);
            }
            self.channel.next = self.channel.next.checked_add(1).ok_or(Generation)?;
            let scores = decode_scores(&retained.numerical, &self.channel.profile)?;
            return LanguageParserScorerRanking::new(
                *request.mask.allowed(),
                0,
                0,
                false,
                0,
                scores,
            )
            .map_err(Native);
        }
        // Do not retry/replay the partial round or silently substitute fallback.
        // Its outstanding request stays in channel; completed evidence is returned on failure.
        self.channel.stopped = true;
        Err(PendingNumerical)
    }
}
impl AnalysisEpochOwner<'_> {
    pub fn prepare_learned_round_with(
        &self,
        programs: &PreparedFallbackRoundPrograms,
        provider: &mut dyn FallbackObservationProvider,
        channel: &mut PreparedLearnedRoundChannel,
    ) -> Result<PreparedFallbackRound, LearnedRoundPreparationFailure> {
        let preflight = if self.session.is_closed() {
            Some(FallbackRoundRefusal::Closed)
        } else if channel.stopped || channel.numerical.is_closed() || channel.pending().is_some() {
            Some(FallbackRoundRefusal::PendingNumerical)
        } else if self.rounds.len() >= self.round_limit
            || self.rounds.len() >= self.rounds.capacity()
        {
            Some(FallbackRoundRefusal::Capacity)
        } else {
            None
        };
        if let Some(refusal) = preflight {
            return Err(LearnedRoundPreparationFailure {
                refusal,
                observations: Vec::new(),
                learned: Vec::new(),
            });
        }
        let context = round_context(self);
        if channel
            .context
            .as_ref()
            .is_some_and(|bound| bound != &context)
        {
            return Err(LearnedRoundPreparationFailure {
                refusal: FallbackRoundRefusal::Stale,
                observations: Vec::new(),
                learned: Vec::new(),
            });
        }
        channel.context = Some(context.clone());
        let mut scoring = LearnedScores {
            channel,
            context: round_context(self),
            generation: self.generation,
            prior: self.beam.clone(),
            evidence: Vec::with_capacity(MAX_LEARNED_REQUESTS_PER_ROUND),
        };
        let mut replay = Replay::with_provider(programs, provider);
        let result = self.run_round(&mut replay, &mut scoring);
        if result.is_err() {
            // Any failed partial traversal is terminal for this wrapper, including
            // failures after a numerical ACK or later Source/Native admission.
            // Evidence remains retained; caller may explicitly cancel the kernel.
            scoring.channel.stopped = true;
        }
        let observations = replay.into_captured();
        let learned = scoring.evidence;
        match result {
            Ok((result, next_identity)) => Ok(PreparedFallbackRound {
                mode: AnalysisRoundMode::AuthenticatedCategorical,
                context: round_context(self),
                learned,
                generation: self.generation,
                prior: self.beam.clone(),
                lexical: self.lexical().tape().clone(),
                result,
                next_identity,
                observations: io::compact_observations(observations),
            }),
            Err(refusal) => Err(LearnedRoundPreparationFailure {
                refusal,
                observations,
                learned,
            }),
        }
    }
}
fn encode_indices(
    profile: &PreparedCategoricalStep,
    indices: &[u64; 25],
) -> Result<Vec<u8>, FallbackRoundRefusal> {
    let StructuredInfoTypeShape::Collection { element, .. } = profile.indices_type().shape() else {
        return Err(FallbackRoundRefusal::Model);
    };
    let values = indices
        .iter()
        .map(|value| StructuredInfoValue::leaf(element.clone(), value.to_le_bytes().to_vec()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| FallbackRoundRefusal::Numerical)?;
    StructuredInfoValue::collection(profile.indices_type().clone(), values)
        .and_then(|value| value.canonical_bytes())
        .map_err(|_| FallbackRoundRefusal::Numerical)
}
fn decode_scores(
    receipt: &DispatchedCategoricalInvocation,
    profile: &PreparedCategoricalStep,
) -> Result<[i64; 76], FallbackRoundRefusal> {
    let mut bytes = vec![0; receipt.output_len()];
    receipt
        .copy_output_into(&mut bytes)
        .map_err(|_| FallbackRoundRefusal::Numerical)?;
    let value = StructuredInfoValue::from_canonical_bytes(&bytes)
        .map_err(|_| FallbackRoundRefusal::Numerical)?;
    if value.value_type() != profile.scores_type() {
        return Err(FallbackRoundRefusal::Model);
    }
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        return Err(FallbackRoundRefusal::Numerical);
    };
    if values.len() != 76 {
        return Err(FallbackRoundRefusal::Numerical);
    }
    let mut scores = [0; 76];
    for (score, value) in scores.iter_mut().zip(values) {
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            return Err(FallbackRoundRefusal::Numerical);
        };
        *score = i64::from_le_bytes(
            bytes
                .try_into()
                .map_err(|_| FallbackRoundRefusal::Numerical)?,
        );
    }
    Ok(scores)
}
