//! Ordered complete revision custody. Slot and frame quotas precede producer or
//! target consumption; publication is exclusively the complete Session's duty.
use crate::{
    lexical_proposer_port::token_producer::revision::{AdmittedRevision, PreparedRevisionProducer},
    parser_session_mixed_custody::ParserMixedHistory,
    parser_session_pure_source::PureSourceHistory,
    parser_session_window8_numeric_profile::ProposalWindow8V2NumericProfile,
};
use alloc::{rc::Rc, sync::Arc, vec::Vec};
use conduit_plot::rust_binding::NativeFamilyTypeDescriptor;
use core::{cell::RefCell, mem::size_of};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BookRefusal {
    Capacity,
    Overflow,
    Revision,
    Consumed,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct BookLimits {
    pub(crate) source_executions: usize,
    pub(crate) model_executions: usize,
    pub(crate) admissions: usize,
    pub(crate) events: usize,
    pub(crate) maximum_frame_heap_bytes: usize,
    pub(crate) maximum_proposed_revision_bytes: usize,
    pub(crate) maximum_new_retained_bytes: usize,
    pub(crate) maximum_history_books: usize,
    pub(crate) maximum_all_history_retained_bytes: usize,
}
#[derive(Clone, Copy, Debug)]
pub(crate) enum BookEvent {
    Source {
        index: usize,
        port: usize,
        epoch: u64,
        model_calls: u64,
    },
    Model {
        index: usize,
        epoch: u64,
        model_call: u64,
    },
    Admission {
        index: usize,
        epoch: u64,
        model_calls: u64,
    },
}
/// Full guarded frame and actual producing Source witness. Neither this locator
/// nor individually Native-valid material is a standalone commitment capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BookAdmissionOutcome {
    Accepted,
    /// Only the actual provisional candidate gate may retain this ordinary
    /// original Native law refusal; unrelated failures cancel the whole stage.
    ProvisionalInvariantRefusal {
        original_law_index: usize,
    },
}
pub(crate) struct BookAdmission {
    pub(crate) gate: BookAdmissionGate,
    pub(crate) canonical: Vec<u8>,
    pub(crate) descriptor: &'static NativeFamilyTypeDescriptor,
    pub(crate) source_execution: usize,
    pub(crate) outcome: BookAdmissionOutcome,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BookAdmissionGate {
    QualifiedLexical,
    QualifiedDependency,
    IndependentCommitSet,
}
impl BookAdmissionGate {
    fn admits_origin(self, origin: &PureSourceHistory) -> bool {
        let names: &[&str] = match self {
            Self::QualifiedLexical => &["language-window8-qualified-lexical-proposal"],
            Self::QualifiedDependency => &["language-window8-qualified-dependency-analysis"],
            Self::IndependentCommitSet => &[
                "language-window8-qualified-independent-commit",
                "language-window8-independent-commit-initialize",
                "language-window8-independent-commit-rebase",
            ],
        };
        crate::parser_session_window8_ports::PORTS
            .iter()
            .any(|port| {
                names.contains(&port.name)
                    && origin.matches_fixed(
                        port.original_programs,
                        port.original_custody,
                        port.input,
                        port.output,
                    )
            })
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::QualifiedLexical => "qualified-lexical-native-admission",
            Self::QualifiedDependency => "qualified-dependency-native-admission",
            Self::IndependentCommitSet => "independent-commit-set-native-admission",
        }
    }
    fn descriptor(self) -> &'static NativeFamilyTypeDescriptor {
        use conduit_plot::rust_binding::PreparedNativeRustBinding;
        match self {
            Self::QualifiedLexical => {
                crate::LanguageParserWindow8QualifiedLexicalAnalysis::PREPARED_DESCRIPTOR
            }
            Self::QualifiedDependency => {
                crate::LanguageParserWindow8QualifiedDependencyAnalysis::PREPARED_DESCRIPTOR
            }
            Self::IndependentCommitSet => {
                crate::LanguageParserWindow8IndependentCommitSet::PREPARED_DESCRIPTOR
            }
        }
    }
}
pub(crate) struct Window8Book {
    // Retain the complete sealed declaration, signature and actual resource,
    // independently of Session/target lifetime. Shared storage is charged once.
    pub(crate) selection: Arc<crate::parser_session_window8_model::VerifiedWindow8Model>,
    // Full dictionary shards, original proposer Source and family/definition
    // survive independently; per-token frames alone do not own that resource.
    pub(crate) proposer: Rc<RefCell<PreparedRevisionProducer>>,
    pub(crate) revision: Option<AdmittedRevision>,
    pub(crate) previous: Option<Rc<Window8Book>>,
    pub(crate) source: Vec<PureSourceHistory>,
    pub(crate) model: Vec<ParserMixedHistory<ProposalWindow8V2NumericProfile>>,
    pub(crate) admissions: Vec<BookAdmission>,
    pub(crate) events: Vec<BookEvent>,
    limits: BookLimits,
    slots_bytes: usize,
    frame_heap_bytes: usize,
    history_books: usize,
    prior_retained_bytes: usize,
    pub(crate) published: bool,
}
fn add(a: usize, b: usize) -> Result<usize, BookRefusal> {
    a.checked_add(b).ok_or(BookRefusal::Overflow)
}
fn slots<T>(count: usize) -> Result<usize, BookRefusal> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(BookRefusal::Overflow)
}
fn book_owner_bytes() -> Result<usize, BookRefusal> {
    // Rc strong/weak counts plus payload alignment; payload included once.
    add(
        add(size_of::<Window8Book>(), slots::<usize>(2)?)?,
        core::mem::align_of::<Window8Book>() - 1,
    )
}
fn reserve<T>(count: usize) -> Result<Vec<T>, BookRefusal> {
    let mut v = Vec::new();
    v.try_reserve_exact(count)
        .map_err(|_| BookRefusal::Capacity)?;
    if v.capacity() != count {
        return Err(BookRefusal::Capacity);
    }
    Ok(v)
}
impl Window8Book {
    /// This is called before whole revision production. Full prior books remain
    /// retained independently; their shared owners are separately charged once.
    pub(crate) fn reserve(
        limits: BookLimits,
        previous: Option<Rc<Self>>,
        selection: Arc<crate::parser_session_window8_model::VerifiedWindow8Model>,
        proposer: Rc<RefCell<PreparedRevisionProducer>>,
    ) -> Result<Self, BookRefusal> {
        if previous.as_ref().is_some_and(|prior| {
            !Arc::ptr_eq(&prior.selection, &selection)
                || !Rc::ptr_eq(&prior.proposer, &proposer)
                || !prior.published
        }) {
            return Err(BookRefusal::Revision);
        }
        let history_books = add(previous.as_ref().map_or(0, |prior| prior.history_books), 1)?;
        let prior_retained_bytes = previous
            .as_ref()
            .map_or(Ok(0), |prior| prior.retained_history_bytes())?;
        let bytes = add(
            add(
                slots::<PureSourceHistory>(limits.source_executions)?,
                slots::<ParserMixedHistory<ProposalWindow8V2NumericProfile>>(
                    limits.model_executions,
                )?,
            )?,
            add(
                slots::<BookAdmission>(limits.admissions)?,
                slots::<BookEvent>(limits.events)?,
            )?,
        )?;
        let maximum = add(
            add(
                add(bytes, book_owner_bytes()?)?,
                limits.maximum_frame_heap_bytes,
            )?,
            limits.maximum_proposed_revision_bytes,
        )?;
        if maximum > limits.maximum_new_retained_bytes
            || history_books > limits.maximum_history_books
            || add(prior_retained_bytes, maximum)? > limits.maximum_all_history_retained_bytes
        {
            return Err(BookRefusal::Capacity);
        }
        Ok(Self {
            selection,
            proposer,
            revision: None,
            previous,
            source: reserve(limits.source_executions)?,
            model: reserve(limits.model_executions)?,
            admissions: reserve(limits.admissions)?,
            events: reserve(limits.events)?,
            limits,
            slots_bytes: bytes,
            frame_heap_bytes: 0,
            history_books,
            prior_retained_bytes,
            published: false,
        })
    }
    pub(crate) fn attach_revision(
        &mut self,
        revision: AdmittedRevision,
    ) -> Result<(), BookRefusal> {
        if self.published || self.revision.is_some() {
            return Err(BookRefusal::Consumed);
        }
        if revision.retained_bytes_bound() > self.limits.maximum_proposed_revision_bytes {
            return Err(BookRefusal::Capacity);
        }
        self.revision = Some(revision);
        Ok(())
    }
    /// The driver calls this with a complete frame reservation before invoking
    /// the corresponding Source/model/guard owner. Actual post-call capacities
    /// are checked again before moving immutable history into the book.
    pub(crate) fn admit_next_frames(&self, bytes: usize) -> Result<(), BookRefusal> {
        if self.published || self.revision.is_none() {
            return Err(BookRefusal::Consumed);
        }
        if add(self.frame_heap_bytes, bytes)? > self.limits.maximum_frame_heap_bytes {
            return Err(BookRefusal::Capacity);
        }
        Ok(())
    }
    pub(crate) fn can_source(&self, bytes: usize) -> Result<(), BookRefusal> {
        self.admit_next_frames(bytes)?;
        if self.source.len() == self.source.capacity()
            || self.events.len() == self.events.capacity()
        {
            return Err(BookRefusal::Capacity);
        }
        Ok(())
    }
    pub(crate) fn push_source(
        &mut self,
        history: PureSourceHistory,
        port: usize,
        epoch: u64,
        model_calls: u64,
    ) -> Result<usize, BookRefusal> {
        let selected = crate::parser_session_window8_ports::PORTS
            .get(port)
            .ok_or(BookRefusal::Revision)?;
        if !history.matches_fixed(
            selected.original_programs,
            selected.original_custody,
            selected.input,
            selected.output,
        ) {
            return Err(BookRefusal::Revision);
        }
        let bytes = history
            .retained_frame_capacity_bytes()
            .ok_or(BookRefusal::Overflow)?;
        self.can_source(bytes)?;
        let index = self.source.len();
        self.frame_heap_bytes = add(self.frame_heap_bytes, bytes)?;
        self.source.push(history);
        self.events.push(BookEvent::Source {
            index,
            port,
            epoch,
            model_calls,
        });
        Ok(index)
    }
    pub(crate) fn can_model(&self, bytes: usize) -> Result<(), BookRefusal> {
        self.admit_next_frames(bytes)?;
        if self.model.len() == self.model.capacity() || self.events.len() == self.events.capacity()
        {
            return Err(BookRefusal::Capacity);
        }
        Ok(())
    }
    pub(crate) fn push_model(
        &mut self,
        history: ParserMixedHistory<ProposalWindow8V2NumericProfile>,
        epoch: u64,
        model_call: u64,
    ) -> Result<usize, BookRefusal> {
        let n = &history.numeric;
        if !core::ptr::eq(n.original_model.as_ref(), self.selection.categorical()) {
            return Err(BookRefusal::Revision);
        }
        let bytes = add(
            add(
                add(n.indices.capacity(), n.scores.capacity())?,
                n.output.capacity(),
            )?,
            add(
                n.features
                    .retained_frame_capacity_bytes()
                    .ok_or(BookRefusal::Overflow)?,
                n.feature_guard.capacity(),
            )?,
        )?;
        self.can_model(bytes)?;
        let index = self.model.len();
        self.frame_heap_bytes = add(self.frame_heap_bytes, bytes)?;
        self.model.push(history);
        self.events.push(BookEvent::Model {
            index,
            epoch,
            model_call,
        });
        Ok(index)
    }
    pub(crate) fn can_admit(&self, bytes: usize) -> Result<(), BookRefusal> {
        self.admit_next_frames(bytes)?;
        if self.admissions.len() == self.admissions.capacity()
            || self.events.len() == self.events.capacity()
        {
            return Err(BookRefusal::Capacity);
        }
        Ok(())
    }
    pub(crate) fn push_admission(
        &mut self,
        admission: BookAdmission,
        epoch: u64,
        model_calls: u64,
    ) -> Result<usize, BookRefusal> {
        self.can_admit(admission.canonical.capacity())?;
        if !self
            .source
            .get(admission.source_execution)
            .is_some_and(|origin| admission.gate.admits_origin(origin))
            || !core::ptr::eq(admission.descriptor, admission.gate.descriptor())
            || (admission.gate == BookAdmissionGate::IndependentCommitSet
                && admission.outcome != BookAdmissionOutcome::Accepted)
        {
            return Err(BookRefusal::Revision);
        }
        self.frame_heap_bytes = add(self.frame_heap_bytes, admission.canonical.capacity())?;
        let index = self.admissions.len();
        self.admissions.push(admission);
        self.events.push(BookEvent::Admission {
            index,
            epoch,
            model_calls,
        });
        Ok(index)
    }
    pub(crate) fn retained_new_bytes(&self) -> Result<usize, BookRefusal> {
        add(
            add(
                add(book_owner_bytes()?, self.slots_bytes)?,
                self.frame_heap_bytes,
            )?,
            self.revision
                .as_ref()
                .ok_or(BookRefusal::Revision)?
                .retained_bytes_bound(),
        )
    }
    pub(crate) fn retained_history_bytes(&self) -> Result<usize, BookRefusal> {
        add(self.prior_retained_bytes, self.retained_new_bytes()?)
    }
}
