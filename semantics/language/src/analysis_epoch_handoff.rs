//! Shared session gates and retained-prefix handoff; no second parser policy.
use super::*;
type PreparedNextEpoch<'a> = (
    AnalysisSession<'a>,
    PreparedLexicalTape,
    LanguageParserScorerBeam,
    u64,
    Vec<Vec<u8>>,
);
impl<'a> AnalysisEpochOwner<'a> {
    pub(super) fn handoff_generation(
        &self,
        context: RevisionContext<'a>,
    ) -> Result<u64, AnalysisEpochRefusal> {
        use AnalysisEpochRefusal::*;
        if self.session.is_closed() {
            return Err(Closed);
        }
        if self.session.publication_dispute().is_some() {
            return Err(Session(AnalysisSessionRefusal::Corrected));
        }
        let generation = self.generation.checked_add(1).ok_or(Generation)?;
        let previous_context = self.session.context();
        if context.epoch == previous_context.epoch
            || context.stream != previous_context.stream
            || context.subject != previous_context.subject
            || context.producer != previous_context.producer
            || context.policy != previous_context.policy
        {
            return Err(Context);
        }
        Ok(generation)
    }
    pub(super) fn retained_session(
        &self,
        domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<AnalysisSession<'a>, AnalysisEpochRefusal> {
        use AnalysisEpochRefusal::*;
        let session = AnalysisSession::from_retained_prefix(
            domain,
            context,
            limits,
            self.frontiers().committed,
        )
        .map_err(Revision)?;
        Ok(session)
    }
    pub(super) fn prepare_next(
        &self,
        receipts: &[PreparedParserRebaseReplay; 4],
        domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<PreparedNextEpoch<'a>, AnalysisEpochRefusal> {
        use AnalysisEpochRefusal::*;
        let generation = self.handoff_generation(context)?;
        #[cfg(target_has_atomic = "ptr")]
        if self.resource.is_some() {
            return Err(Lexical);
        }
        let slots = [
            self.beam.candidate0(),
            self.beam.candidate1(),
            self.beam.candidate2(),
            self.beam.candidate3(),
        ];
        let next_source = receipts[0].preparation().lexical().tape().source();
        let lexical = prepare_lexical_tape(
            next_source,
            self.lexical().tape().profile(),
            Some(self.lexical()),
        )
        .map_err(|_| Lexical)?;
        if lexical.tape().tokens().len() > 4 {
            return Err(Domain);
        }
        if domain.source_text != next_source.material()
            || domain.tokens != lexical.tape().tokens().len() as u64
        {
            return Err(Domain);
        }
        for (prior, replay) in slots.iter().zip(receipts) {
            let request = replay.preparation().request();
            if request.previous() != prior.parser().state()
                || request.previous_lexical().tape() != self.lexical().tape()
                || replay.preparation().lexical().tape() != lexical.tape()
                || replay.result().basis() != receipts[0].result().basis()
                || (*prior.parser().active()
                    && *replay.result().committed() != self.frontiers().committed.0)
            {
                return Err(Replay);
            }
        }
        let (beam, envelopes) =
            rebase_envelopes(slots, receipts.each_ref().map(|r| r.raw_result()))?;
        let session = self.retained_session(domain, context, limits)?;
        Ok((session, lexical, beam, generation, envelopes))
    }
}
