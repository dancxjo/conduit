//! Unpublished revision execution. Every late failure or abandonment cancels
//! the entire fixed Source/model registry, including already consumed ports.
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
    pub(crate) fn model(
        &mut self,
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
            let history = self
                .guard
                .targets()
                .mixed()
                .map_err(R::Source)?
                .execute(query, source_frames, numeric_frames)
                .map_err(R::Mixed)?;
            self.book
                .push_model(history, epoch, model_call)
                .map_err(R::Book)
        })();
        if result.is_err() {
            self.poisoned = true;
            self.guard.targets().cancel_all();
        }
        result
    }
}
