//! Resource-backed entrances into the existing epoch owner and Session lifecycle.
//! Snapshot custody changes; parser numerical policy and currentness stay shared.
use super::*;

#[cfg(target_has_atomic = "ptr")]
pub struct ResourceEpochHandoffFailure<'a> {
    pub owner: AnalysisEpochOwner<'a>,
    pub next: PreparedResourceLexicalSnapshot<'a>,
    pub refusal: AnalysisEpochRefusal,
}
/// Requested next epoch basis and the existing revision-domain limits.
#[cfg(target_has_atomic = "ptr")]
pub struct ResourceEpochTarget<'a> {
    pub analysis: LanguageAnalysisRevisionId,
    pub default_relation: LanguageParserRelation,
    pub domain: &'a DependencyRevisions<'a>,
    pub context: RevisionContext<'a>,
    pub limits: RevisionLimits,
}
#[cfg(target_has_atomic = "ptr")]
struct ResourceEpochPreparation<'a> {
    session: AnalysisSession<'a>,
    receipts: [PreparedResourceParserRebaseReplay; 4],
    beam: LanguageParserScorerBeam,
    generation: u64,
    envelopes: Vec<Vec<u8>>,
}
impl<'a> AnalysisEpochOwner<'a> {
    /// Retain the actual resource owner beside the ordinary initial epoch.
    #[cfg(target_has_atomic = "ptr")]
    pub fn initialize_resource(
        initial: PreparedAnalysisInitialization,
        snapshot: PreparedResourceLexicalSnapshot<'a>,
        domain: &'a DependencyRevisions<'a>,
        context: RevisionContext<'a>,
        limits: RevisionLimits,
    ) -> Result<Self, AnalysisEpochRefusal> {
        if initial.lexical.tape() != snapshot.lexical().tape() || snapshot.previous_tape().is_some()
        {
            return Err(AnalysisEpochRefusal::Lexical);
        }
        let mut owner = Self::initialize(initial, domain, context, limits)?;
        owner.resource = Some(snapshot);
        Ok(owner)
    }
    #[cfg(target_has_atomic = "ptr")]
    pub fn resource_snapshot(&self) -> Option<&PreparedResourceLexicalSnapshot<'a>> {
        self.resource.as_ref()
    }
    #[cfg(target_has_atomic = "ptr")]
    pub fn resource_rebase_receipts(&self) -> &[PreparedResourceParserRebaseReplay] {
        match &self.material {
            Material::ResourceRebased { receipts, .. } => receipts,
            _ => &[],
        }
    }
    /// Actual all-four Source rebasing and the existing epoch/session handoff.
    /// Failure retains both original owners without publishing a next epoch.
    #[cfg(target_has_atomic = "ptr")]
    // Refusal returns both original owners without allocating a recovery box.
    #[allow(clippy::result_large_err)]
    pub fn handoff_resource(
        self,
        next: PreparedResourceLexicalSnapshot<'a>,
        program: &PreparedResourceParserRebaseProgram,
        target: ResourceEpochTarget<'a>,
    ) -> Result<(Self, ArchivedAnalysisEpoch<'a>), ResourceEpochHandoffFailure<'a>> {
        let limits = target.limits;
        let prepared = self.prepare_resource_next(&next, program, target);
        let ResourceEpochPreparation {
            session,
            receipts,
            beam,
            generation,
            envelopes,
        } = match prepared {
            Ok(p) => p,
            Err(refusal) => {
                return Err(ResourceEpochHandoffFailure {
                    owner: self,
                    next,
                    refusal,
                })
            }
        };
        let new = Self {
            instance: AnalysisEpochInstanceHandle(EpochShared::new(())),
            session,
            material: Material::ResourceRebased {
                receipts,
                envelopes,
            },
            resource: Some(next),
            beam,
            generation,
            commitments: Vec::with_capacity(limits.history_events),
            commitment_limit: limits.history_events,
            identity_cursor: self.identity_cursor,
            rounds: Vec::with_capacity(limits.history_events),
            round_limit: limits.history_events,
            frontier_receipts: Vec::with_capacity(limits.history_events),
        };
        Ok((new, ArchivedAnalysisEpoch { owner: self }))
    }
    #[cfg(target_has_atomic = "ptr")]
    fn prepare_resource_next(
        &self,
        next: &PreparedResourceLexicalSnapshot<'a>,
        program: &PreparedResourceParserRebaseProgram,
        target: ResourceEpochTarget<'a>,
    ) -> Result<ResourceEpochPreparation<'a>, AnalysisEpochRefusal> {
        let ResourceEpochTarget {
            analysis,
            default_relation,
            domain,
            context,
            limits,
        } = target;
        use AnalysisEpochRefusal::*;
        let generation = self.handoff_generation(context)?;
        let old = self.resource.as_ref().ok_or(Lexical)?;
        if old.lexical().tape() != self.lexical().tape()
            || next.previous_tape() != Some(old.lexical().tape())
        {
            return Err(Lexical);
        }
        if domain.source_text != next.lexical().tape().source().material()
            || domain.tokens != next.lexical().tape().tokens().len() as u64
        {
            return Err(Domain);
        }
        let slots = [
            self.beam.candidate0(),
            self.beam.candidate1(),
            self.beam.candidate2(),
            self.beam.candidate3(),
        ];
        let mut receipts = Vec::new();
        for slot in slots {
            let receipt = program
                .replay(
                    slot.parser().state(),
                    old,
                    next,
                    analysis.clone(),
                    default_relation.clone(),
                )
                .map_err(ResourceRebase)?;
            if *slot.parser().active()
                && *receipt.result().committed() != self.frontiers().committed.0
            {
                return Err(Replay);
            }
            receipts.push(receipt);
        }
        let receipts: [PreparedResourceParserRebaseReplay; 4] =
            receipts.try_into().map_err(|_| Replay)?;
        let (beam, envelopes) =
            rebase_envelopes(slots, receipts.each_ref().map(|r| r.raw_result()))?;
        let session = self.retained_session(domain, context, limits)?;
        Ok(ResourceEpochPreparation {
            session,
            receipts,
            beam,
            generation,
            envelopes,
        })
    }
}
impl<'a> ArchivedAnalysisEpoch<'a> {
    #[cfg(target_has_atomic = "ptr")]
    pub fn resource_snapshot(&self) -> Option<&PreparedResourceLexicalSnapshot<'a>> {
        self.owner.resource_snapshot()
    }
    #[cfg(target_has_atomic = "ptr")]
    pub fn resource_rebase_receipts(&self) -> &[PreparedResourceParserRebaseReplay] {
        self.owner.resource_rebase_receipts()
    }
}
