//! Unpublished revision execution. Each successful component is retained before
//! another ingress can run; every error poisons and cancels the whole registry.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_execution::ParserSessionEntry,
    parser_session_fixed_ingress::{FixedRefusal, ParserSessionExecutor},
    parser_session_mixed_custody::ParserMixedRefusal,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_revision_custody::{ParserRevisionCustody, RevisionStorageRefusal},
    parser_session_stage::{ParserSessionStage, ParserSessionTargets},
    parser_session_target_registry::{ParserSessionTargetRegistry, RegistryRefusal},
};
use alloc::rc::Rc;
#[derive(Debug)]
pub(crate) enum RevisionStageRefusal<E, S, N> {
    Closed,
    Storage(RevisionStorageRefusal),
    Registry(RegistryRefusal),
    Source(FixedRefusal<E>),
    Model(ParserMixedRefusal<S, N>),
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
