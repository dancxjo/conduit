//! Closed canonical-revision Session ownership. This initial publication path
//! retains complete original execution history and never commits an analysis.
//! Continuation/rebase and effect frontiers are separate required transitions.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_stage::ParserSessionTargets,
    parser_session_target_contract::ParserSessionPreparedTarget,
    parser_session_window8_book::Window8Book,
    parser_session_window8_model_frames::Window8ModelFrameLimits,
    parser_session_window8_preparation::PreparedWindow8Factory,
    parser_session_window8_run::{
        self, InitialRunBanks, InitialRunCursor, InitialRunLimits, InitialRunResult,
    },
};
use alloc::rc::Rc;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8SessionRefusal;
pub(crate) struct Window8RunPolicy {
    pub(crate) maximum_revision_bytes: usize,
    pub(crate) maximum_epochs: u64,
    pub(crate) model_frames: Window8ModelFrameLimits,
    pub(crate) maximum_observation_conversion_requested_bytes: usize,
}
pub(crate) struct PreparedWindow8Session<
    S: ParserSessionPreparedTarget,
    N: ParserSessionPreparedTarget + ParserNumericExecutor,
> {
    factory: PreparedWindow8Factory<S, N>,
    current: Option<RetainedWindow8Revision>,
    prepared_snapshot: Option<alloc::vec::Vec<u8>>,
    policy: Window8RunPolicy,
    closed: bool,
}
struct RetainedWindow8Revision {
    book: Rc<Window8Book>,
    cursor: InitialRunCursor,
}
pub(crate) struct PublishedWindow8Revision {
    pub(crate) book: Rc<Window8Book>,
    pub(crate) progress: InitialRunResult,
}
impl<S, N> PreparedWindow8Session<S, N>
where
    S: ParserSessionPreparedTarget,
    N: ParserSessionPreparedTarget
        + ParserNumericExecutor<Error = <N as ParserCanonicalSourceExecutor>::Error>,
{
    pub(crate) fn from_prepared(
        mut factory: PreparedWindow8Factory<S, N>,
        policy: Window8RunPolicy,
    ) -> Result<Self, Window8SessionRefusal> {
        if policy.maximum_revision_bytes == 0
            || policy.maximum_revision_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || policy.maximum_epochs == 0
        {
            factory.registry.cancel_all();
            return Err(Window8SessionRefusal);
        }
        // The prepared factory already charged its own inline owner. Charge
        // only the additional Session inline bytes plus exact snapshot backing.
        let frame = factory.working.observations.snapshot_capacity();
        let inline = core::mem::size_of::<Self>()
            .checked_sub(core::mem::size_of::<PreparedWindow8Factory<S, N>>())
            .ok_or(Window8SessionRefusal)?;
        let delta = inline.checked_add(frame).ok_or(Window8SessionRefusal)?;
        if factory.working.reserve_owner_delta(delta, delta).is_err() {
            factory.registry.cancel_all();
            return Err(Window8SessionRefusal);
        }
        let mut snapshot = alloc::vec::Vec::new();
        if snapshot.try_reserve_exact(frame).is_err() || snapshot.capacity() != frame {
            factory.registry.cancel_all();
            return Err(Window8SessionRefusal);
        }
        Ok(Self {
            factory,
            prepared_snapshot: Some(snapshot),
            current: None,
            policy,
            closed: false,
        })
    }
    /// Sole initial entrance is full original canonical LanguageTextRevision.
    /// No public Source query, caller Native snapshot or numeric locator enters.
    pub(crate) fn advance_initial(
        &mut self,
        canonical_revision: &[u8],
    ) -> Result<PublishedWindow8Revision, Window8SessionRefusal> {
        let result = (|| {
            if self.closed
                || self.current.is_some()
                || canonical_revision.len() > self.policy.maximum_revision_bytes
            {
                return Err(Window8SessionRefusal);
            }
            // Every book slot, complete response and retained frame quota is
            // admitted before revision production or any parser/model ingress.
            let book = self
                .factory
                .working
                .prepare_book(
                    None,
                    alloc::sync::Arc::clone(&self.factory.model),
                    Rc::clone(&self.factory.proposer),
                )
                .map_err(|_| Window8SessionRefusal)?;
            let mut stage = crate::parser_session_window8_stage::Window8RevisionStage::new(
                &mut self.factory.registry,
                book,
            );
            let revision = self
                .factory
                .proposer
                .try_borrow_mut()
                .map_err(|_| Window8SessionRefusal)?
                .propose(canonical_revision, None, 0)
                .map_err(|_| Window8SessionRefusal)?;
            stage
                .attach_revision(revision)
                .map_err(|_| Window8SessionRefusal)?;
            // An identity is only a label. Full opaque revision/model/Source
            // parents remain retained and independently checked by the driver.
            let digest = conduit_core::semantic_digest(
                "language/window8-session-initial-analysis@1",
                canonical_revision,
            );
            let mut identity = [0u8; 64];
            let hex = b"0123456789abcdef";
            for (pair, byte) in identity.chunks_exact_mut(2).zip(digest) {
                pair[0] = hex[usize::from(byte >> 4)];
                pair[1] = hex[usize::from(byte & 15)];
            }
            let working = &mut self.factory.working;
            let banks = InitialRunBanks {
                queries: &mut working.queries,
                atoms: &mut working.atoms,
                ancestry: &mut working.ancestry,
                tokens: &mut working.tokens,
                choices: &mut working.choices,
                snapshots: &mut working.snapshots,
                observations: &mut working.observations,
                fact: &mut working.fact,
                refinement: &mut working.refinement,
                family: &working.observation_family,
            };
            let limits = InitialRunLimits {
                maximum_epochs: self.policy.maximum_epochs,
                model_frames: &self.policy.model_frames,
                maximum_observation_native_conversion_requested_bytes: self
                    .policy
                    .maximum_observation_conversion_requested_bytes,
            };
            let snapshot = self.prepared_snapshot.take().ok_or(Window8SessionRefusal)?;
            let (guard, mut book, cursor) =
                parser_session_window8_run::execute(stage, banks, &identity, limits, snapshot)
                    .map_err(|_| Window8SessionRefusal)?;
            // All fallible Source/model/admission work precedes this single
            // publication. The Book reservation included its Rc allocation.
            book.published = true;
            let book = Rc::new(book);
            let progress = cursor.progress;
            self.current = Some(RetainedWindow8Revision {
                book: Rc::clone(&book),
                cursor,
            });
            guard.publication_complete();
            Ok(PublishedWindow8Revision { book, progress })
        })();
        if result.is_err() {
            self.closed = true;
            self.factory.registry.cancel_all();
        }
        result
    }
    pub(crate) fn progress(&self) -> Option<InitialRunResult> {
        self.current.as_ref().map(|current| current.cursor.progress)
    }
    pub(crate) fn cancel(&mut self) {
        self.closed = true;
        self.factory.registry.cancel_all();
    }
    pub(crate) fn current(&self) -> Option<&Window8Book> {
        self.current.as_ref().map(|current| current.book.as_ref())
    }
}
