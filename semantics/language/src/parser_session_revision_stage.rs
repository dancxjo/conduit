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
    parser_session_stage::{ParserSessionStage, ParserSessionTargets},
    parser_session_target_registry::{ParserSessionTargetRegistry, RegistryRefusal},
    LanguageParserJointStableFact, LanguageParserJointStableFactProposal,
};
use alloc::rc::Rc;
use conduit_plot::rust_binding::PreparedNativeFamily;
#[derive(Debug)]
pub(crate) enum RevisionStageRefusal<E, S, N> {
    Closed,
    Storage(RevisionStorageRefusal),
    Registry(RegistryRefusal),
    Source(FixedRefusal<E>),
    Model(ParserMixedRefusal<S, N>),
    StableCandidate(StableCandidateRefusal),
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
        use RevisionStageRefusal as R;
        if self.poisoned {
            return Err(R::Closed);
        }
        let result = (|| {
            let book = Rc::get_mut(&mut self.book).ok_or(R::Closed)?;
            if entry == ParserSessionEntry::Seed && !seed_matches_original_tape(book, query) {
                return Err(R::Closed);
            }
            let frames = book.source_frame().map_err(R::Storage)?;
            let history = self
                .guard
                .targets()
                .port(entry)
                .map_err(R::Registry)?
                .execute(query, frames)
                .map_err(R::Source)?;
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
