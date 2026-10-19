//! One complete deterministic fallback round, never learned model evidence.
//! Existing authored Source owns legality, lexical branching, rank and merge.
use super::AnalysisEpochOwner;
use crate::{analysis_epoch_initialization::admit, generated::LanguageAnalysisInitialRawBeam, *};
use alloc::vec::Vec;
use conduit_plot::rust_binding::NativeBindingRefusal;
#[path = "analysis_fallback_io.rs"]
pub(super) mod io;
use io::Replay;
pub use io::{
    FallbackObservationProvider, FallbackOperationObservation, PreparedFallbackRoundPrograms,
    MAXIMUM_ROUND_BYTES, MAXIMUM_ROUND_OBSERVATIONS,
};
#[derive(Debug)]
pub enum FallbackRoundRefusal {
    Native(NativeBindingRefusal),
    Program,
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
}
pub struct PreparedFallbackRound {
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
        "language/complete-round/beam4/zero-score-fallback@1"
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
        let result = (|| {
            if self.session.is_closed() {
                return Err(FallbackRoundRefusal::Closed);
            }
            let mut replay = Replay::new(programs, &observations)?;
            let result = self.run_fallback_round(&mut replay)?;
            replay.finish()?;
            Ok(result)
        })();
        match result {
            Ok((result, next_identity)) => Ok(PreparedFallbackRound {
                generation: self.generation,
                prior: self.beam.clone(),
                lexical: self.lexical().tape().clone(),
                result,
                next_identity,
                observations,
            }),
            Err(refusal) => Err(FallbackRoundPreparationFailure {
                refusal,
                observations,
            }),
        }
    }
    pub fn prepare_fallback_round_with(
        &self,
        programs: &PreparedFallbackRoundPrograms,
        provider: &mut dyn FallbackObservationProvider,
    ) -> Result<PreparedFallbackRound, FallbackRoundPreparationFailure> {
        if self.session.is_closed() {
            return Err(FallbackRoundPreparationFailure {
                refusal: FallbackRoundRefusal::Closed,
                observations: Vec::new(),
            });
        }
        let mut replay = Replay::with_provider(programs, provider);
        let result = self.run_fallback_round(&mut replay);
        let observations = replay.into_captured();
        match result {
            Ok((result, next_identity)) => Ok(PreparedFallbackRound {
                generation: self.generation,
                prior: self.beam.clone(),
                lexical: self.lexical().tape().clone(),
                result,
                next_identity,
                observations,
            }),
            Err(refusal) => Err(FallbackRoundPreparationFailure {
                refusal,
                observations,
            }),
        }
    }
    fn run_fallback_round(
        &self,
        replay: &mut Replay<'_>,
    ) -> Result<(LanguageParserScorerBeam, u64), FallbackRoundRefusal> {
        use FallbackRoundRefusal::*;
        let lexical = LanguageParserAvailableLexical::new(
            self.lexical().tape().clone(),
            *self.beam.candidate0().parser().state().token_count(),
        )
        .map_err(Native)?;
        let raw = raw_beam(&self.beam)?;
        let seed: LanguageAnalysisInitialRawBeam =
            replay.run("language/analysis-round-empty", &raw)?;
        let mut next = admit_beam(&seed)?;
        let mut identity = self.identity_cursor;
        for prior in slots(&self.beam) {
            if !*prior.parser().active() {
                continue;
            }
            let complete =
                replay.run_bool("language-parser-decode-complete", prior.parser().state())?;
            if complete {
                next = merge(replay, next, prior.clone())?;
                continue;
            }
            let mut results = Vec::new();
            for class in [0, 1, 2, 39, 73] {
                let scored = scored(prior, class, 0)?;
                let proposal: LanguageParserScoredProposal =
                    replay.run("language-parser-score-proposal", &scored)?;
                let actual: LanguageParserResult =
                    replay.run("language-parser-transition", proposal.request())?;
                results.push(actual);
            }
            let legality = LanguageParserScorerLegality::new(
                prior.parser().state().basis().clone(),
                results[2].clone(),
                results[1].clone(),
                results[3].clone(),
                results[4].clone(),
                results[0].clone(),
            )
            .map_err(Native)?;
            let mask: LanguageParserScorerRanking =
                replay.run("language-parser-scorer-legality", &legality)?;
            // Enumerate every bounded alternative; only checked branch Source
            // decides acceptance. No caller-selected candidate list is accepted.
            let alternatives = if prior.parser().state().unread() < lexical.token_count() {
                4
            } else {
                1
            };
            for choice in 0..alternatives {
                let branch = if prior.parser().state().unread() < lexical.token_count() {
                    let query =
                        LanguageParserScorerBranch::new(choice, lexical.clone(), prior.clone())
                            .map_err(Native)?;
                    let actual: LanguageParserScorerBranchResult =
                        replay.run("language-parser-scorer-branch", &query)?;
                    if !*actual.accepted() {
                        continue;
                    }
                    admit(actual.candidate()).map_err(|_| Admission)?
                } else {
                    prior.clone()
                };
                // Fallback Source overwrites every score; no numerical/model
                // result can enter this deliberately reduced proof profile.
                let mut rank: LanguageParserScorerRanking =
                    replay.run("language-parser-scorer-fallback", &mask)?;
                for _ in 0..4 {
                    rank = replay.run("language-parser-scorer-rank", &rank)?;
                    if !*rank.found() {
                        break;
                    }
                    let scored = scored(&branch, *rank.class(), *rank.score())?;
                    let proposal: LanguageParserScoredProposal =
                        replay.run("language-parser-score-proposal", &scored)?;
                    let actual: LanguageParserResult =
                        replay.run("language-parser-transition", proposal.request())?;
                    identity = identity.checked_add(1).ok_or(Generation)?;
                    let query = LanguageParserScorerExpansion::new(
                        identity,
                        branch.clone(),
                        actual,
                        *rank.score(),
                    )
                    .map_err(Native)?;
                    let raw: LanguageParserScorerRawHypothesis =
                        replay.run("language-parser-scorer-expand", &query)?;
                    if *raw.parser().active() {
                        let candidate = admit(&raw).map_err(|_| Admission)?;
                        next = merge(replay, next, candidate)?;
                    }
                    rank = replay.run("language-parser-scorer-ranking-next", &rank)?;
                }
            }
        }
        Ok((next, identity))
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
fn admit_beam(
    b: &LanguageAnalysisInitialRawBeam,
) -> Result<LanguageParserScorerBeam, FallbackRoundRefusal> {
    let conv = |h| admit(h).map_err(|_| FallbackRoundRefusal::Admission);
    LanguageParserScorerBeam::new(
        conv(b.candidate0())?,
        conv(b.candidate1())?,
        conv(b.candidate2())?,
        conv(b.candidate3())?,
    )
    .map_err(FallbackRoundRefusal::Native)
}
fn scored(
    h: &LanguageParserScorerHypothesis,
    class: u64,
    score: i64,
) -> Result<LanguageParserScoredClass, FallbackRoundRefusal> {
    use FallbackRoundRefusal::Native;
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).map_err(Native)?,
    )
    .map_err(Native)?;
    LanguageParserScoredClass::new(
        h.parser().state().basis().clone(),
        class,
        relation,
        score,
        numeric(h.parser().state())?,
    )
    .map_err(Native)
}
fn merge(
    replay: &mut Replay<'_>,
    beam: LanguageParserScorerBeam,
    candidate: LanguageParserScorerHypothesis,
) -> Result<LanguageParserScorerBeam, FallbackRoundRefusal> {
    let input =
        LanguageParserScorerMerge::new(beam, candidate).map_err(FallbackRoundRefusal::Native)?;
    replay.run("language-parser-scorer-merge", &input)
}

#[path = "analysis_fallback_frontier.rs"]
pub mod frontier;
