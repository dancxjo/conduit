//! Unpublished revision execution. Each successful component is retained before
//! another ingress can run; every error poisons and cancels the whole registry.
use crate::{
    parser_canonical_refinement::PreparedParserCanonicalRefinement,
    parser_session_candidate_admission::{
        ParserStableCandidateAdmission, StableCandidateOutcome, StableCandidateRefusal,
    },
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_execution::ParserSessionEntry,
    parser_session_fixed_ingress::{FixedRefusal, ParserSessionExecutor},
    parser_session_mixed_custody::ParserMixedRefusal,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_revision_custody::{ParserRevisionCustody, RevisionStorageRefusal},
    parser_session_seed_admission::{ParserSeedBeamAdmission, SeedBeamRefusal},
    parser_session_stage::{ParserSessionStage, ParserSessionTargets},
    parser_session_target_registry::{ParserSessionTargetRegistry, RegistryRefusal},
    LanguageParserJointRuntimeRawBeam, LanguageParserJointStableFact,
    LanguageParserJointStableFactProposal, LanguageParserSessionSeedProposal,
};
use alloc::rc::Rc;
use conduit_plot::rust_binding::PreparedNativeFamily;
#[derive(Debug)]
pub(crate) enum RevisionStageRefusal<E, S, N> {
    Closed,
    Storage(RevisionStorageRefusal),
    Registry(RegistryRefusal),
    Queries(crate::parser_session_queries::ParserQueryRefusal),
    ParentReplay(FixedRefusal<core::convert::Infallible>),
    ModelParentReplay(
        crate::parser_session_numeric_custody::ParserNumericRefusal<core::convert::Infallible>,
    ),
    Source(FixedRefusal<E>),
    Model(ParserMixedRefusal<S, N>),
    StableCandidate(StableCandidateRefusal),
    SeedBeam(SeedBeamRefusal),
}
pub(crate) struct ParserRevisionStage<
    'a,
    E: ParserSessionExecutor,
    S: ParserCanonicalSourceExecutor,
    N: ParserNumericExecutor,
> {
    guard: ParserSessionStage<'a, ParserSessionTargetRegistry<E, S, N>>,
    book: Rc<ParserRevisionCustody>,
    poisoned: bool,
}
impl<'a, E: ParserSessionExecutor, S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>
    ParserRevisionStage<'a, E, S, N>
{
    pub(crate) fn new(
        targets: &'a mut ParserSessionTargetRegistry<E, S, N>,
        book: Rc<ParserRevisionCustody>,
    ) -> Result<Self, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        if book.published || Rc::strong_count(&book) != 1 {
            targets.cancel_all();
            return Err(RevisionStageRefusal::Closed);
        }
        Ok(Self {
            guard: ParserSessionStage::new(targets),
            book,
            poisoned: false,
        })
    }
    fn poison(&mut self) {
        self.poisoned = true;
        self.guard.targets().cancel_all();
    }
    pub(crate) fn source(
        &mut self,
        entry: ParserSessionEntry,
        query: &[u8],
    ) -> Result<usize, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        self.source_with_parents(entry, query, &[])
    }
    /// Compose every whole query field from actual retained Source output.
    /// Neither a caller-supplied frame nor an individually valid Native snapshot
    /// enters this path. Fixed driver routes and the prepared exact schema
    /// determine the record; subsequent whole readmission/replay remains required.
    pub(crate) fn source_from_parents(
        &mut self,
        entry: ParserSessionEntry,
        links: &[crate::parser_session_fixed_ingress::ParserSourceParentLink],
        queries: &mut crate::parser_session_queries::PreparedParserSessionQueries,
    ) -> Result<usize, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        use RevisionStageRefusal as R;
        if self.poisoned {
            return Err(R::Closed);
        }
        let result = compose_source_parent_query(&self.book, entry, links, queries);
        match result {
            Ok(query) => self.source_with_parents(entry, query, links),
            Err(error) => {
                self.poison();
                Err(match error {
                    SourceParentCompositionRefusal::Parent => R::Closed,
                    SourceParentCompositionRefusal::Query(error) => R::Queries(error),
                })
            }
        }
    }
    /// Closed drivers select these fixed paths; this is not a public query API.
    /// Replay every prior retained Source frame before consuming the new query.
    pub(crate) fn source_with_parents(
        &mut self,
        entry: ParserSessionEntry,
        query: &[u8],
        links: &[crate::parser_session_fixed_ingress::ParserSourceParentLink],
    ) -> Result<usize, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        self.source_with_witnesses(entry, query, links, None)
    }
    /// Class and score come from the actual retained mixed execution and Source
    /// mask. The driver still supplies every other field through fixed parents.
    pub(crate) fn ranked_source(
        &mut self,
        query: &[u8],
        links: &[crate::parser_session_fixed_ingress::ParserSourceParentLink],
        parent: crate::parser_session_fixed_ingress::ParserRankParent,
    ) -> Result<usize, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        self.source_with_witnesses(
            ParserSessionEntry::ScoreProposal,
            query,
            links,
            Some(parent),
        )
    }
    fn source_with_witnesses(
        &mut self,
        entry: ParserSessionEntry,
        query: &[u8],
        links: &[crate::parser_session_fixed_ingress::ParserSourceParentLink],
        rank_parent: Option<crate::parser_session_fixed_ingress::ParserRankParent>,
    ) -> Result<usize, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        use RevisionStageRefusal as R;
        if self.poisoned {
            return Err(R::Closed);
        }
        let result = (|| {
            let book = Rc::get_mut(&mut self.book).ok_or(R::Closed)?;
            if entry == ParserSessionEntry::Seed && !seed_matches_original_tape(book, query) {
                return Err(R::Closed);
            }
            if links.len() > crate::parser_session_fixed_ingress::MAXIMUM_SOURCE_PARENT_LINKS {
                return Err(R::Closed);
            }
            if rank_parent.is_some_and(|parent| !book.rank_parent_matches(parent, query)) {
                return Err(R::Closed);
            }
            if !links.is_empty() || rank_parent.is_some() {
                book.validate_event_order().map_err(R::Storage)?;
                for link in links {
                    let parent = book
                        .source_parent(link, book.source_histories.len())
                        .ok_or(R::Closed)?;
                    if !link.matches(&parent.output, query) {
                        return Err(R::Closed);
                    }
                }
                let mut ancestors = Some(&*book);
                while let Some(ancestor) = ancestors {
                    ancestor.validate_event_order().map_err(R::Storage)?;
                    for parent in &ancestor.source_histories {
                        self.guard
                            .targets()
                            .port(parent.entry)
                            .map_err(R::Registry)?
                            .replay_history(parent)
                            .map_err(R::ParentReplay)?;
                    }
                    for parent in &ancestor.mixed_histories {
                        self.guard
                            .targets()
                            .mixed()
                            .map_err(R::Registry)?
                            .replay_history(parent)
                            .map_err(R::ModelParentReplay)?;
                    }
                    ancestors = ancestor.previous.as_deref();
                }
            }
            let frames = book.source_frame().map_err(R::Storage)?;
            let mut history = self
                .guard
                .targets()
                .port(entry)
                .map_err(R::Registry)?
                .execute(query, frames)
                .map_err(R::Source)?;
            for (slot, link) in history.parent_links.iter_mut().zip(links) {
                *slot = Some(*link);
            }
            history.rank_parent = rank_parent;
            book.retain_source(history).map_err(R::Storage)
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
    pub(crate) fn model(
        &mut self,
        query: &[u8],
    ) -> Result<usize, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        use RevisionStageRefusal as R;
        if self.poisoned {
            return Err(R::Closed);
        }
        let result = (|| {
            let book = Rc::get_mut(&mut self.book).ok_or(R::Closed)?;
            if book.seed_admission.is_none() || !original_tape_matches(book, query) {
                return Err(R::Closed);
            }
            let frames = book.model_frames().map_err(R::Storage)?;
            let history = self
                .guard
                .targets()
                .mixed()
                .map_err(R::Registry)?
                .execute(query, frames.source, frames.numeric)
                .map_err(R::Model)?;
            book.retain_model(history).map_err(R::Storage)
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
    pub(crate) fn source_output(&self, execution: usize) -> Option<&[u8]> {
        self.book
            .source_histories
            .get(execution)
            .map(|history| history.output.as_slice())
    }
    /// Only the actual fixed Source initializer can supply the first beam.
    /// Its template may seed a later revision, but cannot authorize publication
    /// over previous commitments without the original protected-rebase chain.
    pub(crate) fn admit_seed_beam(
        &mut self,
        execution: usize,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserSessionSeedProposal,
            LanguageParserJointRuntimeRawBeam,
        >,
        family: &mut PreparedNativeFamily,
    ) -> Result<(), RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        use RevisionStageRefusal as R;
        if self.poisoned {
            return Err(R::Closed);
        }
        let result = (|| {
            let book = Rc::get_mut(&mut self.book).ok_or(R::Closed)?;
            if book.seed_admission.is_some() {
                return Err(R::Closed);
            }
            let buffer = book
                .source_frame()
                .map_err(R::Storage)?
                .into_candidate_buffer();
            let origin = book.source_histories.get(execution).ok_or(R::Closed)?;
            let admission =
                ParserSeedBeamAdmission::admit(execution, origin, refinement, family, buffer)
                    .map_err(R::SeedBeam)?;
            book.retain_seed_admission(admission).map_err(R::Storage)?;
            Ok(())
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
    pub(crate) fn seed_beam(&self) -> Option<&[u8]> {
        if self.poisoned {
            return None;
        }
        self.book
            .seed_admission
            .as_ref()
            .map(|admission| admission.complete_beam.as_slice())
    }
    /// Admit only the complete proposal returned by this revision's actual
    /// stable-fact Source execution. Semantic refusal is retained as evidence;
    /// every other refusal closes the entire unpublished revision.
    pub(crate) fn admit_stable_candidate(
        &mut self,
        execution: usize,
        refinement: &mut PreparedParserCanonicalRefinement<
            LanguageParserJointStableFactProposal,
            LanguageParserJointStableFact,
        >,
        family: &mut PreparedNativeFamily,
    ) -> Result<(usize, StableCandidateOutcome), RevisionStageRefusal<E::Error, S::Error, N::Error>>
    {
        use RevisionStageRefusal as R;
        if self.poisoned {
            return Err(R::Closed);
        }
        let result = (|| {
            let book = Rc::get_mut(&mut self.book).ok_or(R::Closed)?;
            // One reserved frame supplies the retained candidate buffer. No
            // candidate-buffer allocation follows consumption. Full Native
            // readmission still uses its separately admitted conversion bound.
            let buffer = book
                .source_frame()
                .map_err(R::Storage)?
                .into_candidate_buffer();
            let origin = book.source_histories.get(execution).ok_or(R::Closed)?;
            let admission = ParserStableCandidateAdmission::admit(
                execution, origin, refinement, family, buffer,
            )
            .map_err(R::StableCandidate)?;
            let outcome = admission.outcome;
            let index = book
                .retain_stable_admission(admission)
                .map_err(R::Storage)?;
            Ok((index, outcome))
        })();
        if result.is_err() {
            self.poison();
        }
        result
    }
    /// An accepted candidate is borrowed from retained custody. Its index is
    /// only a locator; rejected proposals never become commitment input.
    pub(crate) fn accepted_stable_candidate(&self, admission: usize) -> Option<&[u8]> {
        if self.poisoned {
            return None;
        }
        let admission = self.book.stable_admissions.get(admission)?;
        (admission.outcome == StableCandidateOutcome::Accepted)
            .then_some(admission.original_candidate.as_slice())
    }
    /// The fixed Session policy calls this only after seed/advance/rebase and
    /// commitment witnesses are complete. No intermediate component publishes.
    pub(crate) fn publish(
        mut self,
    ) -> Result<Rc<ParserRevisionCustody>, RevisionStageRefusal<E::Error, S::Error, N::Error>> {
        if self.poisoned {
            return Err(RevisionStageRefusal::Closed);
        }
        let book = Rc::get_mut(&mut self.book).ok_or(RevisionStageRefusal::Closed)?;
        book.validate_event_order()
            .map_err(RevisionStageRefusal::Storage)?;
        book.published = true;
        self.guard.publication_complete();
        Ok(self.book)
    }
}

// The Source seed still performs its original exact basis/token-count laws.
// This additional custody check ties its whole lexical input to the opaque
// original producer owned by this revision, before target consumption.
fn seed_matches_original_tape(book: &ParserRevisionCustody, query: &[u8]) -> bool {
    if book
        .source_histories
        .iter()
        .any(|history| history.entry == ParserSessionEntry::Seed)
    {
        return false;
    }
    original_tape_matches(book, query)
}
fn original_tape_matches(book: &ParserRevisionCustody, query: &[u8]) -> bool {
    fn field<'a>(
        value: conduit_core::ValidatedCanonicalStructuredValue<'a>,
        name: &str,
    ) -> Option<conduit_core::ValidatedCanonicalStructuredValue<'a>> {
        value.record_field(name).ok().flatten()
    }
    let Some(request) = conduit_core::validate_canonical_structured_value(query).ok() else {
        return false;
    };
    let Some(lexical) = field(request, "lexical") else {
        return false;
    };
    let Some(tape) = field(lexical, "tape") else {
        return false;
    };
    let Ok(original) =
        conduit_core::validate_canonical_structured_value(book.original_lexical_bytes.as_slice())
    else {
        return false;
    };
    tape == original
}

// The fixed stack array is finite representation scratch; composition itself
// reuses the prepared query bank and makes no heap allocation.
enum SourceParentCompositionRefusal {
    Parent,
    Query(crate::parser_session_queries::ParserQueryRefusal),
}
fn compose_source_parent_query<'a>(
    book: &ParserRevisionCustody,
    entry: ParserSessionEntry,
    links: &[crate::parser_session_fixed_ingress::ParserSourceParentLink],
    queries: &'a mut crate::parser_session_queries::PreparedParserSessionQueries,
) -> Result<&'a [u8], SourceParentCompositionRefusal> {
    if links.is_empty()
        || links.len() > crate::parser_session_fixed_ingress::MAXIMUM_SOURCE_PARENT_LINKS
    {
        return Err(SourceParentCompositionRefusal::Parent);
    }
    let before = book.source_histories.len();
    let first = book
        .source_parent(&links[0], before)
        .ok_or(SourceParentCompositionRefusal::Parent)?;
    let first = links[0]
        .output(&first.output)
        .ok_or(SourceParentCompositionRefusal::Parent)?;
    let mut fields = [first; crate::parser_session_fixed_ingress::MAXIMUM_SOURCE_PARENT_LINKS];
    for (slot, link) in fields.iter_mut().zip(links) {
        if !matches!(
            link.input_path,
            [crate::parser_canonical_schema::SchemaStep::Field(_)]
        ) {
            return Err(SourceParentCompositionRefusal::Parent);
        }
        let parent = book
            .source_parent(link, before)
            .ok_or(SourceParentCompositionRefusal::Parent)?;
        *slot = link
            .output(&parent.output)
            .ok_or(SourceParentCompositionRefusal::Parent)?;
    }
    queries
        .record(entry, &fields[..links.len()])
        .map_err(SourceParentCompositionRefusal::Query)
}
