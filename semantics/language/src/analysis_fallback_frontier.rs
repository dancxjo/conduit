//! True all-survivor contiguous stability; no caller-supplied stable Boolean.
use super::FallbackObservationProvider;
use super::{
    io::Replay, FallbackOperationObservation, FallbackRoundRefusal, PreparedFallbackRoundPrograms,
};
use crate::{
    analysis_epoch_owner::{AnalysisEpochOwner, AnalysisEpochRefusal},
    revision::*,
    *,
};
use alloc::vec::Vec;
use conduit_core::revision::*;
pub struct PreparedFallbackFrontier {
    generation: u64,
    beam: LanguageParserScorerBeam,
    lexical: LanguageLexicalTape,
    frontier: LanguageParserRawFrontier,
    observations: Vec<FallbackOperationObservation>,
    cancelled: bool,
    pressure: bool,
}
pub struct FrontierPreparationFailure {
    pub refusal: FallbackRoundRefusal,
    pub observations: Vec<FallbackOperationObservation>,
}
pub struct FrontierApplyFailure {
    pub refusal: AnalysisEpochRefusal,
    pub prepared: PreparedFallbackFrontier,
}
impl PreparedFallbackFrontier {
    pub fn frontier(&self) -> &LanguageParserRawFrontier {
        &self.frontier
    }
    pub fn observations(&self) -> &[FallbackOperationObservation] {
        &self.observations
    }
    pub fn source_id(&self) -> &'static str {
        crate::parser_commitment::COMMITMENT_SOURCE_ID
    }
}
impl<'a> AnalysisEpochOwner<'a> {
    pub fn prepare_fallback_frontier(
        &self,
        programs: &PreparedFallbackRoundPrograms,
        observations: Vec<FallbackOperationObservation>,
        cancelled: bool,
        pressure: bool,
    ) -> Result<PreparedFallbackFrontier, FrontierPreparationFailure> {
        let result = (|| {
            if self.session.is_closed() {
                return Err(FallbackRoundRefusal::Closed);
            }
            let mut replay = Replay::new(programs, &observations)?;
            let result = self.run_frontier(&mut replay, cancelled, pressure)?;
            replay.finish()?;
            Ok(result)
        })();
        match result {
            Ok(frontier) => Ok(PreparedFallbackFrontier {
                generation: self.generation,
                beam: self.beam.clone(),
                lexical: self.lexical().tape().clone(),
                frontier,
                observations,
                cancelled,
                pressure,
            }),
            Err(refusal) => Err(FrontierPreparationFailure {
                refusal,
                observations,
            }),
        }
    }
    pub fn prepare_fallback_frontier_with(
        &self,
        programs: &PreparedFallbackRoundPrograms,
        provider: &mut dyn FallbackObservationProvider,
        cancelled: bool,
        pressure: bool,
    ) -> Result<PreparedFallbackFrontier, FrontierPreparationFailure> {
        if self.session.is_closed() {
            return Err(FrontierPreparationFailure {
                refusal: FallbackRoundRefusal::Closed,
                observations: Vec::new(),
            });
        }
        let mut replay = Replay::with_provider(programs, provider);
        let result = self.run_frontier(&mut replay, cancelled, pressure);
        let observations = replay.into_captured();
        match result {
            Ok(frontier) => Ok(PreparedFallbackFrontier {
                generation: self.generation,
                beam: self.beam.clone(),
                lexical: self.lexical().tape().clone(),
                frontier,
                observations,
                cancelled,
                pressure,
            }),
            Err(refusal) => Err(FrontierPreparationFailure {
                refusal,
                observations,
            }),
        }
    }
    fn run_frontier(
        &self,
        replay: &mut Replay<'_>,
        cancelled: bool,
        pressure: bool,
    ) -> Result<LanguageParserRawFrontier, FallbackRoundRefusal> {
        use FallbackRoundRefusal::Native;
        let basis = self.beam.candidate0().parser().state().basis().clone();
        let beam = LanguageParserAdmittedBeam::new(
            basis.clone(),
            self.beam.candidate0().parser().clone(),
            self.beam.candidate1().parser().clone(),
            self.beam.candidate2().parser().clone(),
            self.beam.candidate3().parser().clone(),
        )
        .map_err(Native)?;
        let committed = self.frontiers().committed.0;
        let mut frontier =
            LanguageParserRawFrontier::new(basis, committed, committed).map_err(Native)?;
        for dependent in committed..*self.beam.candidate0().parser().state().token_count() {
            let query = LanguageParserAdmittedAgreementQuery::new(beam.clone(), dependent)
                .map_err(Native)?;
            let agreement: LanguageParserAgreement =
                replay.run("language-parser-admitted-agreement", &query)?;
            let input =
                LanguageParserAdvanceRequest::new(agreement, cancelled, frontier.clone(), pressure)
                    .map_err(Native)?;
            let actual: LanguageParserFrontierResult =
                replay.run("language-parser-advance-stable", &input)?;
            frontier = actual.frontier().clone();
            if !*actual.accepted() {
                break;
            }
        }
        Ok(frontier)
    }
    pub fn append_fallback_frontier(
        &mut self,
        event: &'a RevisionEvent<'a, DependencyRevisions<'a>>,
        prepared: PreparedFallbackFrontier,
    ) -> Result<(), FrontierApplyFailure> {
        let generation = self.generation.checked_add(1);
        let refusal = if self.session.is_closed() {
            Some(AnalysisEpochRefusal::Closed)
        } else if self.frontier_receipts.len() >= self.round_limit
            || self.frontier_receipts.len() >= self.frontier_receipts.capacity()
        {
            Some(AnalysisEpochRefusal::Capacity)
        } else if prepared.generation != self.generation
            || prepared.beam != self.beam
            || &prepared.lexical != self.lexical().tape()
        {
            Some(AnalysisEpochRefusal::Replay)
        } else if !matches!(event.change(),RevisionChange::Stable{through,..} if through.0==*prepared.frontier.stable())
        {
            Some(AnalysisEpochRefusal::FrontierCustody)
        } else if generation.is_none() {
            Some(AnalysisEpochRefusal::Generation)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            return Err(FrontierApplyFailure { refusal, prepared });
        }
        if let Err(e) = self
            .session
            .append(event, None, prepared.cancelled, prepared.pressure)
        {
            return Err(FrontierApplyFailure {
                refusal: AnalysisEpochRefusal::Session(e),
                prepared,
            });
        }
        self.generation = generation.unwrap_or(self.generation);
        self.frontier_receipts.push(prepared);
        Ok(())
    }
    pub fn fallback_frontier_receipts(&self) -> &[PreparedFallbackFrontier] {
        &self.frontier_receipts
    }
}
