//! Unpublished revision execution. Every late failure or abandonment cancels
//! the entire fixed Source/model registry, including already consumed ports.
use conduit_plot::rust_binding::PreparedNativeRustBinding;
use crate::{
    parser_session_canonical_ingress::{
        ParserCanonicalSourceExecutor, PreparedParserExecutionFrames,
    },
    parser_session_mixed_custody::ParserMixedRefusal,
    parser_session_numeric_custody::{ParserNumericExecutor, ParserNumericFrames},
    parser_session_pure_source::{PureSourceFrames, PureSourceRefusal},
    parser_session_stage::{ParserSessionStage, ParserSessionTargets},
    parser_session_window8_book::{BookRefusal, Window8Book},
    parser_session_window8_candidate::{QualifiedLexicalRefusal, Window8QualifiedLexicalAdmission},
    parser_session_window8_registry::Window8Registry,
};
use alloc::vec::Vec;
use core::mem::size_of;

#[derive(Debug)]
pub(crate) enum Window8StageRefusal {
    Book(BookRefusal),
    Source(PureSourceRefusal),
    Candidate(QualifiedLexicalRefusal),
    Dependency(crate::parser_session_window8_dependency_candidate::QualifiedDependencyRefusal),
}
#[derive(Debug)]
pub(crate) enum Window8ModelStageRefusal<S, N> {
    Book(BookRefusal),
    Source(PureSourceRefusal),
    Mixed(ParserMixedRefusal<S, N>),
}
pub(crate) struct Window8RevisionStage<
    'a,
    S: ParserCanonicalSourceExecutor,
    N: ParserNumericExecutor,
> {
    guard: ParserSessionStage<'a, Window8Registry<S, N>>,
    book: Window8Book,
    poisoned: bool,
}
impl<'a, S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>
    Window8RevisionStage<'a, S, N>
{
    pub(crate) fn new(registry: &'a mut Window8Registry<S, N>, book: Window8Book) -> Self {
        Self {
            guard: ParserSessionStage::new(registry),
            book,
            poisoned: false,
        }
    }
    pub(crate) fn source(
        &mut self,
        port: usize,
        query: &[u8],
        frames: PureSourceFrames,
        epoch: u64,
        model_calls: u64,
    ) -> Result<usize, Window8StageRefusal> {
        if self.poisoned {
            return Err(Window8StageRefusal::Book(BookRefusal::Consumed));
        }
        let result = (|| {
            // Charge the complete supplied backing allocations before consumption;
            // successful output lengths do not replace their reserved capacities.
            let bytes = frames
                .input
                .capacity()
                .checked_add(frames.output.capacity())
                .and_then(|n| {
                    frames
                        .intermediates
                        .capacity()
                        .checked_mul(size_of::<Vec<u8>>())
                        .and_then(|slots| n.checked_add(slots))
                })
                .and_then(|n| {
                    frames
                        .intermediates
                        .iter()
                        .try_fold(n, |sum, frame| sum.checked_add(frame.capacity()))
                })
                .ok_or(Window8StageRefusal::Book(BookRefusal::Overflow))?;
            self.book
                .can_source(bytes)
                .map_err(Window8StageRefusal::Book)?;
            let history = self
                .guard
                .targets()
                .source(port)
                .map_err(Window8StageRefusal::Source)?
                .execute(query, frames)
                .map_err(Window8StageRefusal::Source)?;
            self.book
                .push_source(history, port, epoch, model_calls)
                .map_err(Window8StageRefusal::Book)
        })();
        if result.is_err() {
            self.poisoned = true;
            self.guard.targets().cancel_all();
        }
        result
    }
    /// Normal driver entrance: book slots and complete backing requests are
    /// admitted before constructing any execution buffers.
    pub(crate) fn source_reserved(
        &mut self,
        port: usize,
        query: &[u8],
        epoch: u64,
        model_calls: u64,
    ) -> Result<usize, Window8StageRefusal> {
        if self.poisoned {
            return Err(Window8StageRefusal::Book(BookRefusal::Consumed));
        }
        let frames = (|| {
            let owner = self
                .guard
                .targets()
                .source(port)
                .map_err(Window8StageRefusal::Source)?;
            let bytes = owner
                .frame_reservation(query.len())
                .map_err(Window8StageRefusal::Source)?;
            self.book
                .can_source(bytes)
                .map_err(Window8StageRefusal::Book)?;
            owner
                .prepare_frames(query.len(), bytes)
                .map_err(Window8StageRefusal::Source)
        })();
        match frames {
            Ok(frames) => self.source(port, query, frames, epoch, model_calls),
            Err(refusal) => {
                self.poisoned = true;
                self.guard.targets().cancel_all();
                Err(refusal)
            }
        }
    }
    pub(crate) fn source_named(
        &mut self,
        name: &str,
        query: &[u8],
        epoch: u64,
        model_calls: u64,
    ) -> Result<usize, Window8StageRefusal> {
        match crate::parser_session_window8_ports::port_index(name) {
            Some(port) => self.source_reserved(port, query, epoch, model_calls),
            None => {
                self.poisoned = true;
                self.guard.targets().cancel_all();
                Err(Window8StageRefusal::Source(PureSourceRefusal::Descriptor))
            }
        }
    }
    pub(crate) fn book(&self) -> &Window8Book {
        &self.book
    }
    /// The closed driver calls this for representation/metadata failures before
    /// a target invocation too. Failed revisions cannot be resumed or published.
    pub(crate) fn abort(&mut self) {
        self.poisoned = true;
        self.guard.targets().cancel_all();
    }
    /// Attach only the actual opaque producer result. A failed attachment
    /// poisons the entire unpublished revision just like a late target failure.
    pub(crate) fn attach_revision(
        &mut self,
        revision: crate::lexical_proposer_port::token_producer::revision::AdmittedRevision,
    ) -> Result<(), Window8StageRefusal> {
        if self.poisoned {
            return Err(Window8StageRefusal::Book(BookRefusal::Consumed));
        }
        match self.book.attach_revision(revision) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.poisoned = true;
                self.guard.targets().cancel_all();
                Err(Window8StageRefusal::Book(error))
            }
        }
    }
    /// Transfer an unpoisoned, still-unpublished stage to the closed driver.
    /// The active guard remains owned and cancels on abandonment. This method
    /// neither publishes the book nor disarms cancellation.
    pub(crate) fn into_unpublished_parts(
        self,
    ) -> Result<(ParserSessionStage<'a, Window8Registry<S, N>>, Window8Book), Window8StageRefusal>
    {
        if self.poisoned || self.book.revision.is_none() || self.book.published {
            return Err(Window8StageRefusal::Book(BookRefusal::Consumed));
        }
        Ok((self.guard, self.book))
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit_qualified_lexical(
        &mut self,
        source_execution: usize,
        refinement: &mut crate::parser_canonical_refinement::PreparedParserCanonicalRefinement<
            crate::LanguageParserWindow8QualifiedLexicalProposal,
            crate::LanguageParserWindow8QualifiedLexicalAnalysis,
        >,
        family: &mut conduit_plot::rust_binding::PreparedNativeFamily,
        buffer: Vec<u8>,
        maximum_native_conversion_requested_bytes: usize,
        epoch: u64,
        model_calls: u64,
    ) -> Result<usize, Window8StageRefusal> {
        if self.poisoned {
            return Err(Window8StageRefusal::Book(BookRefusal::Consumed));
        }
        let result = (|| {
            self.book
                .can_admit(buffer.capacity())
                .map_err(Window8StageRefusal::Book)?;
            let origin = self
                .book
                .source
                .get(source_execution)
                .ok_or(Window8StageRefusal::Book(BookRefusal::Revision))?;
            let retained = self
                .book
                .revision
                .as_ref()
                .ok_or(Window8StageRefusal::Book(BookRefusal::Revision))?;
            let proposed = conduit_core::validate_canonical_structured_value(
                retained.canonical_proposed_tape(),
            )
            .map_err(|_| Window8StageRefusal::Book(BookRefusal::Revision))?;
            let output = conduit_core::validate_canonical_structured_value(&origin.output)
                .map_err(|_| Window8StageRefusal::Book(BookRefusal::Revision))?;
            let query = output
                .record_field("query")
                .map_err(|_| Window8StageRefusal::Book(BookRefusal::Revision))?
                .ok_or(Window8StageRefusal::Book(BookRefusal::Revision))?;
            if query
                .record_field("proposed")
                .map_err(|_| Window8StageRefusal::Book(BookRefusal::Revision))?
                != Some(proposed)
            {
                return Err(Window8StageRefusal::Book(BookRefusal::Revision));
            }
            let native_bytes = family.storage_receipt().conversion_requested_bytes_bound
                .checked_mul(2).ok_or(Window8StageRefusal::Candidate(
                    crate::parser_session_window8_candidate::QualifiedLexicalRefusal::Pressure))?;
            let candidate_bytes = crate::LanguageParserWindow8QualifiedLexicalAnalysis::PREPARED_DESCRIPTOR
                .type_bytes.len().checked_add(output.value_node().len())
                .ok_or(Window8StageRefusal::Candidate(
                    crate::parser_session_window8_candidate::QualifiedLexicalRefusal::Pressure))?;
            if native_bytes > maximum_native_conversion_requested_bytes || candidate_bytes > buffer.capacity() {
                return Err(Window8StageRefusal::Candidate(
                    crate::parser_session_window8_candidate::QualifiedLexicalRefusal::Pressure));
            }
            let port = crate::parser_session_window8_ports::PORTS
                .iter()
                .position(|port| port.name == "language-window8-qualified-lexical-proposal")
                .ok_or(Window8StageRefusal::Book(BookRefusal::Revision))?;
            // Replay the actual original full proposal before its guarded
            // refinement. No independently Native-valid snapshot is accepted.
            self.guard
                .targets()
                .source(port)
                .map_err(Window8StageRefusal::Source)?
                .replay(origin)
                .map_err(Window8StageRefusal::Source)?;
            let admission = Window8QualifiedLexicalAdmission::admit(
                source_execution,
                origin,
                refinement,
                family,
                buffer,
            )
            .map_err(Window8StageRefusal::Candidate)?;
            self.book
                .push_admission(admission.into_book(), epoch, model_calls)
                .map_err(Window8StageRefusal::Book)
        })();
        if result.is_err() {
            self.poisoned = true;
            self.guard.targets().cancel_all();
        }
        result
    }
    /// Replay the complete retained endpoint projections before fresh dependency
    /// admission. Source evaluator/Native owner envelopes are already reserved;
    /// this call additionally bounds historical replay count and direct Native
    /// conversion requests before invoking any parent owner.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn admit_qualified_dependency(
        &mut self,
        source_execution: usize,
        refinement: &mut crate::parser_canonical_refinement::PreparedParserCanonicalRefinement<
            crate::LanguageParserWindow8QualifiedDependencyProposal,
            crate::LanguageParserWindow8QualifiedDependencyAnalysis,
        >,
        family: &mut conduit_plot::rust_binding::PreparedNativeFamily,
        buffer: Vec<u8>,
        maximum_bytes: usize,
        maximum_source_replays: usize,
        maximum_native_conversion_requested_bytes: usize,
        epoch: u64,
        model_calls: u64,
    ) -> Result<usize, Window8StageRefusal> {
        use crate::parser_session_window8_dependency_candidate::{
            DependencyParents, QualifiedDependencyRefusal, Window8QualifiedDependencyAdmission,
        };
        if self.poisoned {
            return Err(Window8StageRefusal::Book(BookRefusal::Consumed));
        }
        let result = (|| {
            self.book
                .can_admit(buffer.capacity())
                .map_err(Window8StageRefusal::Book)?;
            let parents = DependencyParents::resolve(&self.book, source_execution)
                .map_err(Window8StageRefusal::Dependency)?;
            let direct_conversions = parents
                .lexical_indices()
                .len()
                .checked_add(2)
                .and_then(|count| {
                    count.checked_mul(family.storage_receipt().conversion_requested_bytes_bound)
                })
                .ok_or(Window8StageRefusal::Dependency(
                    QualifiedDependencyRefusal::Pressure,
                ))?;
            if parents.source_indices().len() > maximum_source_replays
                || direct_conversions > maximum_native_conversion_requested_bytes
                || buffer.capacity() > maximum_bytes
                || parents
                    .candidate_bytes(&self.book)
                    .map_err(Window8StageRefusal::Dependency)?
                    > buffer.capacity()
            {
                return Err(Window8StageRefusal::Dependency(
                    QualifiedDependencyRefusal::Pressure,
                ));
            }
            for index in parents.source_indices() {
                let history = &self.book.source[*index];
                let port = crate::parser_session_window8_ports::PORTS
                    .iter()
                    .position(|port| {
                        history.matches_fixed(
                            port.original_programs,
                            port.original_custody,
                            port.input,
                            port.output,
                        )
                    })
                    .ok_or(Window8StageRefusal::Book(BookRefusal::Revision))?;
                self.guard
                    .targets()
                    .source(port)
                    .map_err(Window8StageRefusal::Source)?
                    .replay(history)
                    .map_err(Window8StageRefusal::Source)?;
            }
            let admission = Window8QualifiedDependencyAdmission::admit(
                &self.book,
                &parents,
                refinement,
                family,
                buffer,
                maximum_bytes,
            )
            .map_err(Window8StageRefusal::Dependency)?;
            self.book
                .push_admission(admission.into_book(), epoch, model_calls)
                .map_err(Window8StageRefusal::Book)
        })();
        if result.is_err() {
            self.poisoned = true;
            self.guard.targets().cancel_all();
        }
        result
    }
    pub(crate) fn model(
        &mut self,
        parent_source: usize,
        query: &[u8],
        source_frames: PreparedParserExecutionFrames,
        numeric_frames: ParserNumericFrames,
        epoch: u64,
        model_call: u64,
    ) -> Result<usize, Window8ModelStageRefusal<S::Error, N::Error>> {
        use Window8ModelStageRefusal as R;
        if self.poisoned {
            return Err(R::Book(BookRefusal::Consumed));
        }
        let result = (|| {
            let bytes = numeric_frames
                .retained_capacity_bytes()
                .and_then(|n| n.checked_add(source_frames.retained_capacity_bytes()))
                .ok_or(R::Book(BookRefusal::Overflow))?;
            self.book.can_model(bytes).map_err(R::Book)?;
            let parent = self
                .book
                .source
                .get(parent_source)
                .ok_or(R::Book(BookRefusal::Revision))?;
            let port = crate::parser_session_window8_ports::PORTS
                .iter()
                .position(|port| port.name == "language-proposal-window8-feature-context")
                .ok_or(R::Book(BookRefusal::Revision))?;
            let selected = &crate::parser_session_window8_ports::PORTS[port];
            if !parent.matches_fixed(
                selected.original_programs,
                selected.original_custody,
                selected.input,
                selected.output,
            ) || parent.output.as_slice() != query
            {
                return Err(R::Book(BookRefusal::Revision));
            }
            self.guard
                .targets()
                .source(port)
                .map_err(R::Source)?
                .replay(parent)
                .map_err(R::Source)?;
            let history = self
                .guard
                .targets()
                .mixed()
                .map_err(R::Source)?
                .execute(query, source_frames, numeric_frames)
                .map_err(R::Mixed)?;
            self.book
                .push_model(history, parent_source, epoch, model_call)
                .map_err(R::Book)
        })();
        if result.is_err() {
            self.poisoned = true;
            self.guard.targets().cancel_all();
        }
        result
    }
}
