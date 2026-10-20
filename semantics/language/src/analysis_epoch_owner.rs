//! Finite semantic epoch custody; not an installed decoder or playback owner.
//! General learned decoder updates deliberately have no public acceptance API.
use crate::{
    analysis_epoch_initialization::*,
    analysis_session::*,
    lexical::{prepare_lexical_tape, PreparedLexicalTape},
    parser_rebase_replay::PreparedParserRebaseReplay,
    revision::*,
    *,
};
#[cfg(target_has_atomic = "ptr")]
use crate::{
    resource_lexicon_snapshot::PreparedResourceLexicalSnapshot, resource_parser_rebase::*,
};
#[cfg(not(target_has_atomic = "ptr"))]
use alloc::rc::Rc as EpochShared;
#[cfg(target_has_atomic = "ptr")]
use alloc::sync::Arc as EpochShared;
use alloc::vec::Vec;
/// Opaque epoch-owner instance membership, independent of currentness/authority.
#[derive(Clone)]
pub struct AnalysisEpochInstanceHandle(EpochShared<()>);
impl AnalysisEpochInstanceHandle {
    pub fn matches(&self, owner: &AnalysisEpochOwner<'_>) -> bool {
        EpochShared::ptr_eq(&self.0, &owner.instance.0)
    }
}
use conduit_core::revision::*;
use conduit_plot::rust_binding::NativeBindingRefusal;
#[derive(Debug)]
pub enum AnalysisEpochRefusal {
    Closed,
    Revision(RevisionRefusal),
    Session(AnalysisSessionRefusal),
    Initialization(AnalysisInitializationRefusal),
    Native(NativeBindingRefusal),
    Domain,
    Generation,
    Replay,
    #[cfg(target_has_atomic = "ptr")]
    ResourceRebase(ResourceParserRebaseRefusal),
    Lexical,
    Context,
    Capacity,
    FrontierCustody,
}
#[path = "analysis_epoch_retained.rs"]
mod retained;
use retained::{Material, PreparedNextEpoch};
#[path = "analysis_beam_slot.rs"]
mod beam_slot;
use beam_slot::SharedBeam;
pub struct AnalysisEpochOwner<'a> {
    instance: AnalysisEpochInstanceHandle,
    session: AnalysisSession<'a>,
    material: Material,
    #[cfg(target_has_atomic = "ptr")]
    resource: Option<PreparedResourceLexicalSnapshot<'a>>,
    beam: SharedBeam,
    generation: u64,
    commitments: Vec<&'a crate::dependency_commitment::PreparedCommittedDependency>,
    commitment_limit: usize,
    identity_cursor: u64,
    rounds: Vec<fallback_round::PreparedFallbackRound>,
    round_limit: usize,
    frontier_receipts: Vec<fallback_round::frontier::PreparedFallbackFrontier>,
}
pub struct ArchivedAnalysisEpoch<'a> {
    owner: AnalysisEpochOwner<'a>,
}
pub struct AnalysisEpochHandoffFailure<'a> {
    pub owner: AnalysisEpochOwner<'a>,
    pub refusal: AnalysisEpochRefusal,
    pub replays: [PreparedParserRebaseReplay; 4],
}
impl<'a> AnalysisEpochOwner<'a> {
    pub fn instance_handle(&self) -> AnalysisEpochInstanceHandle {
        self.instance.clone()
    }
    pub fn initialize(
        initial: PreparedAnalysisInitialization,
        domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<Self, AnalysisEpochRefusal> {
        if domain.source_text != initial.lexical.tape().source().material()
            || domain.tokens != initial.lexical.tape().tokens().len() as u64
        {
            return Err(AnalysisEpochRefusal::Domain);
        }
        let session = AnalysisSession::new(domain, context, limits)
            .map_err(AnalysisEpochRefusal::Revision)?;
        let beam = SharedBeam::prepare(initial.beam.clone());
        let identity_cursor = [
            beam.candidate0(),
            beam.candidate1(),
            beam.candidate2(),
            beam.candidate3(),
        ]
        .iter()
        .map(|s| *s.parser().identity())
        .max()
        .ok_or(AnalysisEpochRefusal::Generation)?;
        Ok(Self {
            instance: AnalysisEpochInstanceHandle(EpochShared::new(())),
            session,
            material: Material::Initial(initial),
            #[cfg(target_has_atomic = "ptr")]
            resource: None,
            beam,
            generation: 0,
            commitments: Vec::with_capacity(limits.history_events),
            commitment_limit: limits.history_events,
            identity_cursor,
            rounds: Vec::with_capacity(limits.history_events),
            round_limit: limits.history_events,
            frontier_receipts: Vec::with_capacity(limits.history_events),
        })
    }
    pub fn beam(&self) -> &LanguageParserScorerBeam {
        self.beam.get()
    }
    pub fn lexical(&self) -> &PreparedLexicalTape {
        match &self.material {
            Material::Initial(i) => &i.lexical,
            Material::Rebased { lexical, .. } => lexical,
            #[cfg(target_has_atomic = "ptr")]
            Material::ResourceRebased { .. } => self
                .resource
                .as_ref()
                .expect("resource owner invariant")
                .lexical(),
        }
    }
    pub fn commitment_receipts(
        &self,
    ) -> &[&'a crate::dependency_commitment::PreparedCommittedDependency] {
        &self.commitments
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn initialization_receipt(&self) -> Option<&PreparedAnalysisInitialization> {
        match &self.material {
            Material::Initial(i) => Some(i),
            Material::Rebased { .. } => None,
            #[cfg(target_has_atomic = "ptr")]
            Material::ResourceRebased { .. } => None,
        }
    }
    pub fn rebase_receipts(&self) -> &[PreparedParserRebaseReplay] {
        match &self.material {
            Material::Initial(_) => &[],
            #[cfg(target_has_atomic = "ptr")]
            Material::ResourceRebased { .. } => &[],
            Material::Rebased { receipts, .. } => receipts,
        }
    }
    pub fn rebase_envelope_outputs(&self) -> &[Vec<u8>] {
        match &self.material {
            Material::Initial(_) => &[],
            Material::Rebased { envelopes, .. } => envelopes,
            #[cfg(target_has_atomic = "ptr")]
            Material::ResourceRebased { envelopes, .. } => envelopes,
        }
    }
    pub fn frontiers(&self) -> RevisionFrontiers<TokenFrontier> {
        self.session.frontiers()
    }
    pub fn history(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a RevisionEvent<'a, DependencyRevisions<'a>>> + '_ {
        self.session.history()
    }
    /// No arbitrary beam setter exists. This initial owner cannot yet consume
    /// learned transition output: that requires opaque stage-by-stage custody.
    pub fn current<'s>(
        &'s self,
        reference: RevisionReference<'a>,
        receipt: &'s crate::dependency_commitment::PreparedCommittedDependency,
    ) -> Result<CurrentCommittedDependency<'s, 'a>, AnalysisSessionRefusal> {
        self.session.current(reference, receipt)
    }
    /// Admit ordinary semantic lifecycle events; commit has its own correlated
    /// entrance so it cannot advance the journal without replacing every slot.
    pub fn append_event(
        &mut self,
        event: &'a RevisionEvent<'a, DependencyRevisions<'a>>,
        cancelled: bool,
        pressure: bool,
    ) -> Result<(), AnalysisEpochRefusal> {
        if matches!(event.change(), RevisionChange::Committed { .. }) {
            return Err(AnalysisEpochRefusal::Replay);
        }
        if matches!(event.change(), RevisionChange::Stable { .. }) {
            return Err(AnalysisEpochRefusal::FrontierCustody);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(AnalysisEpochRefusal::Generation)?;
        self.session
            .append(event, None, cancelled, pressure)
            .map_err(AnalysisEpochRefusal::Session)?;
        self.generation = generation;
        Ok(())
    }
    pub fn append_commitment(
        &mut self,
        event: &'a RevisionEvent<'a, DependencyRevisions<'a>>,
        receipt: &'a crate::dependency_commitment::PreparedCommittedDependency,
        cancelled: bool,
        pressure: bool,
    ) -> Result<(), AnalysisEpochRefusal> {
        use AnalysisEpochRefusal::*;
        require_commitment_capacity(
            self.commitments.len(),
            self.commitment_limit,
            self.commitments.capacity(),
        )?;
        if !matches!(event.change(), RevisionChange::Committed { .. }) {
            return Err(Replay);
        }
        let slots = [
            self.beam.candidate0(),
            self.beam.candidate1(),
            self.beam.candidate2(),
            self.beam.candidate3(),
        ];
        let expected = LanguageParserAdmittedBeam::new(
            slots[0].parser().state().basis().clone(),
            slots[0].parser().clone(),
            slots[1].parser().clone(),
            slots[2].parser().clone(),
            slots[3].parser().clone(),
        )
        .map_err(Native)?;
        if receipt.commitment().request().beam() != &expected
            || receipt.lexical() != self.lexical().tape()
        {
            return Err(Replay);
        }
        let actual = receipt.commitment().result();
        let outputs = [
            actual.candidate0(),
            actual.candidate1(),
            actual.candidate2(),
            actual.candidate3(),
        ];
        let mut updated = Vec::new();
        for (prior, output) in slots.into_iter().zip(outputs) {
            updated.push(
                crate::analysis_epoch_initialization::admit(
                    &LanguageParserScorerRawHypothesis::new(
                        *prior.choices(),
                        output.clone(),
                        *prior.selected(),
                    )
                    .map_err(Native)?,
                )
                .map_err(Initialization)?,
            );
        }
        let mut it = updated.into_iter();
        let beam = LanguageParserScorerBeam::new(
            it.next().ok_or(Replay)?,
            it.next().ok_or(Replay)?,
            it.next().ok_or(Replay)?,
            it.next().ok_or(Replay)?,
        )
        .map_err(Native)?;
        let generation = self.generation.checked_add(1).ok_or(Generation)?;
        self.session
            .append(event, Some(receipt), cancelled, pressure)
            .map_err(Session)?;
        self.commitments.push(receipt);
        self.beam = SharedBeam::prepare(beam);
        self.generation = generation;
        Ok(())
    }
    /// Consuming handoff preserves old history privately and cannot coexist with
    /// a live authority borrow. Failure returns the old owner unchanged.
    // Refusal returns the original owner and four replay receipts without allocation.
    #[allow(clippy::result_large_err)]
    pub fn handoff(
        self,
        receipts: [PreparedParserRebaseReplay; 4],
        next_domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<(Self, ArchivedAnalysisEpoch<'a>), AnalysisEpochHandoffFailure<'a>> {
        let prepared = self.prepare_next(&receipts, next_domain, context, limits);
        let (session, lexical, beam, generation, envelopes) = match prepared {
            Ok(p) => p,
            Err(refusal) => {
                return Err(AnalysisEpochHandoffFailure {
                    owner: self,
                    refusal,
                    replays: receipts,
                })
            }
        };
        let next = Self {
            instance: AnalysisEpochInstanceHandle(EpochShared::new(())),
            session,
            #[cfg(target_has_atomic = "ptr")]
            resource: None,
            material: Material::Rebased {
                lexical,
                receipts,
                envelopes,
            },
            beam: SharedBeam::prepare(beam),
            generation,
            commitments: Vec::with_capacity(limits.history_events),
            commitment_limit: limits.history_events,
            identity_cursor: self.identity_cursor,
            rounds: Vec::with_capacity(limits.history_events),
            round_limit: limits.history_events,
            frontier_receipts: Vec::with_capacity(limits.history_events),
        };
        Ok((next, ArchivedAnalysisEpoch { owner: self }))
    }
    fn handoff_generation(
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
    fn retained_session(
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
    fn prepare_next(
        &self,
        receipts: &[PreparedParserRebaseReplay; 4],
        domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<PreparedNextEpoch<'a>, AnalysisEpochRefusal> {
        use AnalysisEpochRefusal::*;
        #[cfg(target_has_atomic = "ptr")]
        if self.resource.is_some() {
            return Err(Lexical);
        }
        let generation = self.handoff_generation(context)?;
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
impl<'a> ArchivedAnalysisEpoch<'a> {
    pub fn initialization_receipt(&self) -> Option<&PreparedAnalysisInitialization> {
        self.owner.initialization_receipt()
    }
    pub fn rebase_receipts(&self) -> &[PreparedParserRebaseReplay] {
        self.owner.rebase_receipts()
    }
    pub fn beam(&self) -> &LanguageParserScorerBeam {
        self.owner.beam.get()
    }
    pub fn commitment_receipts(
        &self,
    ) -> &[&'a crate::dependency_commitment::PreparedCommittedDependency] {
        self.owner.commitment_receipts()
    }
    pub fn lexical(&self) -> &PreparedLexicalTape {
        self.owner.lexical()
    }
    pub fn frontiers(&self) -> RevisionFrontiers<TokenFrontier> {
        self.owner.frontiers()
    }
    pub fn generation(&self) -> u64 {
        self.owner.generation
    }
    pub fn history(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a RevisionEvent<'a, DependencyRevisions<'a>>> + '_ {
        self.owner.history()
    }
}

fn require_commitment_capacity(
    len: usize,
    limit: usize,
    capacity: usize,
) -> Result<(), AnalysisEpochRefusal> {
    if len >= limit || len >= capacity {
        Err(AnalysisEpochRefusal::Capacity)
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod capacity_tests {
    use super::*;
    #[test]
    fn logical_archive_limit_precedes_spare_allocator_capacity() {
        assert!(matches!(
            require_commitment_capacity(2, 2, 8),
            Err(AnalysisEpochRefusal::Capacity)
        ));
        assert!(matches!(
            require_commitment_capacity(0, 4, 0),
            Err(AnalysisEpochRefusal::Capacity)
        ));
        assert!(require_commitment_capacity(1, 2, 8).is_ok());
    }
}

#[path = "analysis_fallback_round.rs"]
pub mod fallback_round;

#[cfg(target_has_atomic = "ptr")]
#[path = "analysis_epoch_resource.rs"]
mod resource;
#[cfg(target_has_atomic = "ptr")]
pub use resource::{ResourceEpochHandoffFailure, ResourceEpochTarget};

#[path = "analysis_epoch_rebase.rs"]
mod rebase;
use rebase::rebase_envelopes;
