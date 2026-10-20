//! One shared complete-round traversal with explicit scoring provenance.
//! Existing authored Source owns legality, lexical branching, rank and merge.
use super::AnalysisEpochOwner;
use crate::{generated::LanguageAnalysisInitialRawBeam, *};
use alloc::{string::String, vec::Vec};
use conduit_plot::rust_binding::NativeBindingRefusal;
#[path = "analysis_fallback_io.rs"]
pub(super) mod io;

pub use io::{
    FallbackObservationProvider, FallbackOperationObservation, PreparedFallbackRoundPrograms,
    MAXIMUM_ROUND_BYTES, MAXIMUM_ROUND_OBSERVATIONS,
};
#[cfg(target_has_atomic = "ptr")]
#[path = "analysis_round_prelude.rs"]
pub mod canonical_prelude;
#[path = "analysis_round_cursor.rs"]
pub mod cursor;
#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "ptr",
    target_has_atomic = "8"
))]
#[path = "analysis_learned_round.rs"]
pub mod learned;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalysisRoundMode {
    ZeroScoreFallback,
    AuthenticatedCategorical,
}
#[derive(Debug)]
pub enum FallbackRoundRefusal {
    Native(NativeBindingRefusal),
    Program,
    Cursor(cursor::CursorRefusal),
    Evaluation,
    Bound,
    IncompleteTrace {
        operation: alloc::string::String,
        input: Vec<u8>,
    },
    ExtraObservation,
    ForeignObservation,
    Admission,
    Generation,
    Stale,
    Capacity,
    Closed,
    Model,
    Numerical,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    NumericalChannel(conduit_ai::integer_categorical_step::channel::CategoricalChannelRefusal),
    PendingNumerical,
}
pub struct PreparedFallbackRound {
    mode: AnalysisRoundMode,
    context: [String; 5],
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    learned: Vec<learned::LearnedBranchEvidence>,
    generation: u64,
    prior: LanguageParserScorerBeam,
    lexical: LanguageLexicalTape,
    result: LanguageParserScorerBeam,
    next_identity: u64,
    observations: Vec<FallbackOperationObservation>,
}
pub struct FallbackRoundPreparationFailure {
    pub refusal: FallbackRoundRefusal,
    pub observations: Vec<FallbackOperationObservation>,
}
pub struct FallbackRoundApplyFailure {
    pub refusal: FallbackRoundRefusal,
    pub prepared: PreparedFallbackRound,
}
impl PreparedFallbackRound {
    pub fn profile(&self) -> &'static str {
        match self.mode {
            AnalysisRoundMode::ZeroScoreFallback => {
                "language/complete-round/beam4/zero-score-fallback@1"
            }
            AnalysisRoundMode::AuthenticatedCategorical => {
                "language/complete-round/beam4/authenticated-categorical@1"
            }
        }
    }
    pub fn mode(&self) -> AnalysisRoundMode {
        self.mode
    }
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub fn learned_evidence(&self) -> &[learned::LearnedBranchEvidence] {
        &self.learned
    }
    pub fn prior(&self) -> &LanguageParserScorerBeam {
        &self.prior
    }
    pub fn result(&self) -> &LanguageParserScorerBeam {
        &self.result
    }
    pub fn observations(&self) -> &[FallbackOperationObservation] {
        &self.observations
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
}
impl<'a> AnalysisEpochOwner<'a> {
    pub fn prepare_fallback_round(
        &self,
        programs: &PreparedFallbackRoundPrograms,
        observations: Vec<FallbackOperationObservation>,
    ) -> Result<PreparedFallbackRound, FallbackRoundPreparationFailure> {
        cursor::hosted::trace(self, programs, observations)
    }
    pub fn prepare_fallback_round_with(
        &self,
        programs: &PreparedFallbackRoundPrograms,
        provider: &mut dyn FallbackObservationProvider,
    ) -> Result<PreparedFallbackRound, FallbackRoundPreparationFailure> {
        cursor::hosted::fallback(self, programs, provider)
    }
    pub fn apply_fallback_round(
        &mut self,
        prepared: PreparedFallbackRound,
    ) -> Result<(), FallbackRoundApplyFailure> {
        let refusal = if self.session.is_closed() {
            Some(FallbackRoundRefusal::Closed)
        } else if self
            .frontiers()
            .stable_through
            .is_some_and(|stable| stable > self.frontiers().committed)
            || prepared.context != round_context(self)
            || prepared.generation != self.generation
            || prepared.prior != self.beam
            || &prepared.lexical != self.lexical().tape()
        {
            Some(FallbackRoundRefusal::Stale)
        } else if self.rounds.len() >= self.round_limit
            || self.rounds.len() >= self.rounds.capacity()
        {
            Some(FallbackRoundRefusal::Capacity)
        } else if self.generation.checked_add(1).is_none() {
            Some(FallbackRoundRefusal::Generation)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            return Err(FallbackRoundApplyFailure { refusal, prepared });
        }
        self.beam = prepared.result.clone();
        self.identity_cursor = prepared.next_identity;
        self.generation += 1;
        self.rounds.push(prepared);
        Ok(())
    }
    pub fn fallback_rounds(&self) -> &[PreparedFallbackRound] {
        &self.rounds
    }
}
fn slots(beam: &LanguageParserScorerBeam) -> [&LanguageParserScorerHypothesis; 4] {
    [
        beam.candidate0(),
        beam.candidate1(),
        beam.candidate2(),
        beam.candidate3(),
    ]
}
fn numeric(s: &LanguageParserState) -> Result<LanguageParserNumericState, FallbackRoundRefusal> {
    LanguageParserNumericState::new(
        s.basis().clone(),
        *s.committed(),
        *s.depth(),
        *s.heads(),
        s.relation0().clone(),
        s.relation1().clone(),
        s.relation2().clone(),
        s.relation3().clone(),
        *s.stack(),
        *s.token_count(),
        *s.unread(),
    )
    .map_err(FallbackRoundRefusal::Native)
}
fn raw_hyp(
    s: &LanguageParserScorerHypothesis,
) -> Result<LanguageParserScorerRawHypothesis, FallbackRoundRefusal> {
    use FallbackRoundRefusal::Native;
    let p = s.parser();
    let raw = LanguageParserRawHypothesis::new(
        *p.active(),
        *p.identity(),
        *p.score(),
        numeric(p.state())?,
    )
    .map_err(Native)?;
    LanguageParserScorerRawHypothesis::new(*s.choices(), raw, *s.selected()).map_err(Native)
}
fn raw_beam(
    b: &LanguageParserScorerBeam,
) -> Result<LanguageAnalysisInitialRawBeam, FallbackRoundRefusal> {
    LanguageAnalysisInitialRawBeam::new(
        raw_hyp(b.candidate0())?,
        raw_hyp(b.candidate1())?,
        raw_hyp(b.candidate2())?,
        raw_hyp(b.candidate3())?,
    )
    .map_err(FallbackRoundRefusal::Native)
}
#[path = "analysis_fallback_frontier.rs"]
pub mod frontier;

fn round_context(owner: &AnalysisEpochOwner<'_>) -> [String; 5] {
    let context = owner.session.context();
    [
        context.stream,
        context.subject,
        context.epoch,
        context.producer,
        context.policy,
    ]
    .map(|part| String::from(part.as_str()))
}
