//! Original independent commitment custody. This is not the legacy contiguous
//! commit admission and cannot be cast into a speech committed token role.
//! Only the closed revision stage may install the returned Book admission,
//! after replaying all resolved Source parents under its cancellation guard.
use crate::{
    LanguageParserWindow8IndependentCommitSet as Set,
    LanguageParserWindow8IndependentCommitSetProposal as Proposal,
    LanguageParserWindow8QualifiedDependencyAnalysis as Analysis,
    parser_canonical_refinement::{ParserRefinementLimits, PreparedParserCanonicalRefinement},
    parser_session_window8_book::{
        BookAdmission, BookAdmissionGate, BookAdmissionOutcome, Window8Book,
    },
    parser_session_window8_ports::PORTS,
};
use alloc::{rc::Rc, vec::Vec};
use conduit_core::{
    ValidatedCanonicalStructuredValue as Value, validate_canonical_structured_value,
};
use conduit_plot::rust_binding::{
    NativeBindingRefusal, PreparedNativeFamily, PreparedNativeRustBinding,
};

#[derive(Debug)]
pub(crate) enum Window8CommitRefusal {
    Origin,
    Pressure,
    Refinement,
    Native(NativeBindingRefusal),
}
fn value(bytes: &[u8]) -> Result<Value<'_>, Window8CommitRefusal> {
    validate_canonical_structured_value(bytes).map_err(|_| Window8CommitRefusal::Origin)
}
fn field<'a>(v: Value<'a>, name: &str) -> Result<Value<'a>, Window8CommitRefusal> {
    v.record_field(name)
        .map_err(|_| Window8CommitRefusal::Origin)?
        .ok_or(Window8CommitRefusal::Origin)
}
fn fixed(book: &Window8Book, index: usize, name: &str) -> bool {
    book.source.get(index).is_some_and(|history| {
        PORTS.iter().find(|p| p.name == name).is_some_and(|p| {
            history.matches_fixed(p.original_programs, p.original_custody, p.input, p.output)
        })
    })
}
fn accepted(
    book: &Window8Book,
    index: usize,
    gate: BookAdmissionGate,
) -> Result<&BookAdmission, Window8CommitRefusal> {
    let admission = book
        .admissions
        .get(index)
        .ok_or(Window8CommitRefusal::Origin)?;
    let descriptor = match gate {
        BookAdmissionGate::QualifiedDependency => Analysis::PREPARED_DESCRIPTOR,
        BookAdmissionGate::IndependentCommitSet => Set::PREPARED_DESCRIPTOR,
        _ => return Err(Window8CommitRefusal::Origin),
    };
    if admission.gate != gate
        || admission.outcome != BookAdmissionOutcome::Accepted
        || !core::ptr::eq(admission.descriptor, descriptor)
    {
        return Err(Window8CommitRefusal::Origin);
    }
    Ok(admission)
}

/// All indices are resolved from this actual Book, then re-resolved immediately
/// before admission. They are never independently usable commitment authority.
#[derive(PartialEq, Eq)]
pub(crate) struct CommitParents {
    proposal: usize,
    sources: [usize; 4],
    source_count: usize,
    analysis: Option<usize>,
    previous: Option<usize>,
}
impl CommitParents {
    pub(crate) fn resolve(
        book: &Window8Book,
        proposal: usize,
    ) -> Result<Self, Window8CommitRefusal> {
        use Window8CommitRefusal as R;
        if book.published || book.revision.is_none() {
            return Err(R::Origin);
        }
        let output = book.source.get(proposal).ok_or(R::Origin)?;
        let query = value(&output.input)?;
        let previous = book
            .admissions
            .iter()
            .enumerate()
            .rev()
            .find(|(_, a)| {
                a.gate == BookAdmissionGate::IndependentCommitSet
                    && a.outcome == BookAdmissionOutcome::Accepted
            })
            .map(|(i, _)| i);
        if fixed(
            book,
            proposal,
            "language-window8-independent-commit-initialize",
        ) {
            // Initializing over a retained commitment would silently discard it.
            if previous.is_some() || book.previous.is_some() {
                return Err(R::Origin);
            }
            let basis = field(query, "basis")?;
            let initializer = book
                .source
                .iter()
                .enumerate()
                .take(proposal)
                .rfind(|(i, h)| {
                    fixed(book, *i, "language-window8-initialize")
                        && value(&h.output).and_then(|v| field(v, "basis")).ok() == Some(basis)
                })
                .map(|(i, _)| i)
                .ok_or(R::Origin)?;
            return Ok(Self {
                proposal,
                sources: [initializer, proposal, 0, 0],
                source_count: 2,
                analysis: None,
                previous: None,
            });
        }
        if !fixed(
            book,
            proposal,
            "language-window8-qualified-independent-commit",
        ) {
            return Err(R::Origin);
        }
        let previous = previous.ok_or(R::Origin)?;
        let prior = accepted(book, previous, BookAdmissionGate::IndependentCommitSet)?;
        if prior.source_execution >= proposal
            || field(query, "previous")? != value(&prior.canonical)?
        {
            return Err(R::Origin);
        }
        let analysis_value = field(query, "analysis")?;
        let analysis = book
            .admissions
            .iter()
            .enumerate()
            .rfind(|(_, a)| {
                a.gate == BookAdmissionGate::QualifiedDependency
                    && a.outcome == BookAdmissionOutcome::Accepted
                    && a.source_execution < proposal
                    && value(&a.canonical).ok() == Some(analysis_value)
            })
            .map(|(i, _)| i)
            .ok_or(R::Origin)?;
        let admitted = accepted(book, analysis, BookAdmissionGate::QualifiedDependency)?;
        if !fixed(
            book,
            admitted.source_execution,
            "language-window8-qualified-dependency-analysis",
        ) {
            return Err(R::Origin);
        }
        let edge = field(query, "edge")?;
        let anchor = book
            .source
            .iter()
            .enumerate()
            .take(proposal)
            .rfind(|(i, h)| {
                *i > admitted.source_execution
                    && fixed(book, *i, "language-window8-qualified-dependency-anchor")
                    && value(&h.input).ok() == Some(analysis_value)
                    && value(&h.output).ok() == Some(edge)
            })
            .map(|(i, _)| i)
            .ok_or(R::Origin)?;
        Ok(Self {
            proposal,
            sources: [
                prior.source_execution,
                admitted.source_execution,
                anchor,
                proposal,
            ],
            source_count: 4,
            analysis: Some(analysis),
            previous: Some(previous),
        })
    }
    pub(crate) fn source_indices(&self) -> &[usize] {
        &self.sources[..self.source_count]
    }
    pub(crate) fn candidate_bytes(
        &self,
        book: &Window8Book,
    ) -> Result<usize, Window8CommitRefusal> {
        Set::PREPARED_DESCRIPTOR
            .type_bytes
            .len()
            .checked_add(
                value(
                    &book
                        .source
                        .get(self.proposal)
                        .ok_or(Window8CommitRefusal::Origin)?
                        .output,
                )?
                .value_node()
                .len(),
            )
            .ok_or(Window8CommitRefusal::Pressure)
    }
    /// Upfront envelope for all full Native conversions performed here. Source
    /// replay and refinement storage are charged separately by their owners.
    pub(crate) fn conversion_bound(
        &self,
        family: &PreparedNativeFamily,
    ) -> Result<usize, Window8CommitRefusal> {
        (2usize + usize::from(self.analysis.is_some()) + usize::from(self.previous.is_some()))
            .checked_mul(family.storage_receipt().conversion_requested_bytes_bound)
            .ok_or(Window8CommitRefusal::Pressure)
    }
}

pub(crate) struct Window8IndependentCommitAdmission(BookAdmission);

#[derive(Clone, Copy, Debug)]
pub(crate) struct Window8CommitPreparationReceipt {
    pub(crate) preparation_requested_bytes_bound: usize,
    pub(crate) retained_requested_bytes_bound: usize,
}
/// One finite, single-use commit slot. The complete ready family is an incoming
/// owner charged by the factory, including its conversion envelope. Preparing
/// a pool of slots requires summing these ceilings before the first allocation.
pub(crate) struct PreparedWindow8CommitSlot {
    refinement: PreparedParserCanonicalRefinement<Proposal, Set>,
    buffer: Vec<u8>,
    maximum_bytes: usize,
    receipt: Window8CommitPreparationReceipt,
}
impl PreparedWindow8CommitSlot {
    pub(crate) fn prepare(
        family: &PreparedNativeFamily,
        refinement_limits: ParserRefinementLimits,
        maximum_bytes: usize,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_requested_bytes: usize,
    ) -> Result<Self, Window8CommitRefusal> {
        use Window8CommitRefusal as R;
        let inline = core::mem::size_of::<Self>();
        let extra = inline.checked_add(maximum_bytes).ok_or(R::Pressure)?;
        let preparation = extra
            .checked_add(refinement_limits.maximum_preparation_requested_bytes)
            .ok_or(R::Pressure)?;
        let retained = extra
            .checked_add(refinement_limits.maximum_retained_requested_bytes)
            .ok_or(R::Pressure)?;
        if maximum_bytes == 0
            || maximum_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || maximum_bytes < refinement_limits.maximum_node_bytes
            || preparation > maximum_preparation_requested_bytes
            || retained > maximum_retained_requested_bytes
        {
            return Err(R::Pressure);
        }
        let refinement = PreparedParserCanonicalRefinement::prepare(family, refinement_limits)
            .map_err(|_| R::Refinement)?;
        let mut buffer = Vec::new();
        buffer
            .try_reserve_exact(maximum_bytes)
            .map_err(|_| R::Pressure)?;
        if buffer.capacity() != maximum_bytes {
            return Err(R::Pressure);
        }
        let actual = refinement.receipt();
        Ok(Self {
            refinement,
            buffer,
            maximum_bytes,
            receipt: Window8CommitPreparationReceipt {
                preparation_requested_bytes_bound: extra
                    .checked_add(actual.preparation_requested_bytes_bound)
                    .ok_or(R::Pressure)?,
                retained_requested_bytes_bound: extra
                    .checked_add(actual.retained_requested_bytes_bound)
                    .ok_or(R::Pressure)?,
            },
        })
    }
    pub(crate) fn receipt(&self) -> Window8CommitPreparationReceipt {
        self.receipt
    }
    pub(crate) fn candidate_capacity(&self) -> usize {
        self.buffer.capacity()
    }
    pub(crate) fn admit(
        mut self,
        book: &Window8Book,
        parents: &CommitParents,
        family: &mut PreparedNativeFamily,
        maximum_conversion_requested_bytes: usize,
    ) -> Result<Window8IndependentCommitAdmission, Window8CommitRefusal> {
        Window8IndependentCommitAdmission::admit(
            book,
            parents,
            &mut self.refinement,
            family,
            self.buffer,
            self.maximum_bytes,
            maximum_conversion_requested_bytes,
        )
    }
}
impl Window8IndependentCommitAdmission {
    /// `buffer` is a previously reserved exact-capacity candidate slot. Neither
    /// Native conversion nor refinement may start before the cumulative ceiling
    /// and full parent identity checks below. No provisional commit refusals.
    pub(crate) fn admit(
        book: &Window8Book,
        parents: &CommitParents,
        refinement: &mut PreparedParserCanonicalRefinement<Proposal, Set>,
        family: &mut PreparedNativeFamily,
        mut buffer: Vec<u8>,
        maximum_bytes: usize,
        maximum_conversion_requested_bytes: usize,
    ) -> Result<Self, Window8CommitRefusal> {
        use Window8CommitRefusal as R;
        if CommitParents::resolve(book, parents.proposal)? != *parents {
            return Err(R::Origin);
        }
        if parents.candidate_bytes(book)? > buffer.capacity()
            || buffer.capacity() > maximum_bytes
            || parents.conversion_bound(family)? > maximum_conversion_requested_bytes
        {
            return Err(R::Pressure);
        }
        if !family.contains_descriptor(Proposal::PREPARED_DESCRIPTOR)
            || !family.contains_descriptor(Set::PREPARED_DESCRIPTOR)
            || parents.analysis.is_some()
                && !family.contains_descriptor(Analysis::PREPARED_DESCRIPTOR)
        {
            return Err(R::Origin);
        }
        if let Some(index) = parents.analysis {
            drop(
                family
                    .decode::<Analysis>(&book.admissions[index].canonical)
                    .map_err(R::Native)?,
            );
        }
        if let Some(index) = parents.previous {
            drop(
                family
                    .decode::<Set>(&book.admissions[index].canonical)
                    .map_err(R::Native)?,
            );
        }
        let history = &book.source[parents.proposal];
        drop(
            family
                .decode::<Proposal>(&history.output)
                .map_err(R::Native)?,
        );
        let candidate = refinement
            .compose(value(&history.output)?)
            .map_err(|_| R::Refinement)?;
        if candidate.len() != parents.candidate_bytes(book)? {
            return Err(R::Refinement);
        }
        buffer.clear();
        buffer.extend_from_slice(candidate);
        drop(family.decode::<Set>(&buffer).map_err(R::Native)?);
        Ok(Self(BookAdmission {
            gate: BookAdmissionGate::IndependentCommitSet,
            canonical: buffer,
            descriptor: Set::PREPARED_DESCRIPTOR,
            source_execution: parents.proposal,
            outcome: BookAdmissionOutcome::Accepted,
        }))
    }
    pub(crate) fn into_book(self) -> BookAdmission {
        self.0
    }
}

/// Original immutable admission and all exact execution/lexical/model owners
/// remain alive together. This exposes canonical Window8 commitments only;
/// a separate original speech admission is required for token-role projection.
pub(crate) struct OwnedWindow8IndependentCommit {
    book: Rc<Window8Book>,
    admission: usize,
}
impl OwnedWindow8IndependentCommit {
    pub(crate) fn from_published(
        book: Rc<Window8Book>,
        admission: usize,
        maximum_owner_bytes: usize,
    ) -> Result<Self, Window8CommitRefusal> {
        // Rc cloning shares the original complete Book and allocates nothing.
        // The embedding Session/queue must still reserve this inline owner slot.
        if core::mem::size_of::<Self>() > maximum_owner_bytes {
            return Err(Window8CommitRefusal::Pressure);
        }
        if !book.published {
            return Err(Window8CommitRefusal::Origin);
        }
        let entry = accepted(&book, admission, BookAdmissionGate::IndependentCommitSet)?;
        if !fixed(
            &book,
            entry.source_execution,
            "language-window8-qualified-independent-commit",
        ) {
            return Err(Window8CommitRefusal::Origin);
        }
        Ok(Self { book, admission })
    }
    pub(crate) fn canonical(&self) -> &[u8] {
        &self.book.admissions[self.admission].canonical
    }
    pub(crate) fn book(&self) -> &Window8Book {
        &self.book
    }
}

/// Complete finite replay closure, including every original qualified dependency
/// parent and the previous independent-set ancestry. Exact full canonical values
/// correlate every historical hop; recursion is a bounded iterative walk.
pub(crate) struct CommitReplayPlan {
    indices: [usize; 256],
    count: usize,
}
impl CommitReplayPlan {
    fn add(&mut self, index: usize) -> Result<(), Window8CommitRefusal> {
        if self.indices[..self.count].contains(&index) {
            return Ok(());
        }
        *self
            .indices
            .get_mut(self.count)
            .ok_or(Window8CommitRefusal::Pressure)? = index;
        self.count += 1;
        Ok(())
    }
    pub(crate) fn indices(&self) -> &[usize] {
        &self.indices[..self.count]
    }
    pub(crate) fn resolve(
        book: &Window8Book,
        parents: &CommitParents,
    ) -> Result<Self, Window8CommitRefusal> {
        use Window8CommitRefusal as R;
        let mut plan = Self {
            indices: [0; 256],
            count: 0,
        };
        for i in parents.source_indices() {
            plan.add(*i)?;
        }
        let mut cursor = 0;
        while cursor < plan.count {
            let index = plan.indices[cursor];
            cursor += 1;
            let history = book.source.get(index).ok_or(R::Origin)?;
            if fixed(
                book,
                index,
                "language-window8-qualified-dependency-analysis",
            ) {
                let dependency =
                    crate::parser_session_window8_dependency_candidate::DependencyParents::resolve(
                        book, index,
                    )
                    .map_err(|_| R::Origin)?;
                for i in dependency.source_indices() {
                    plan.add(*i)?;
                }
            } else if fixed(book, index, "language-window8-qualified-independent-commit") {
                let query = value(&history.input)?;
                let prior_value = field(query, "previous")?;
                let previous = book
                    .admissions
                    .iter()
                    .rfind(|a| {
                        a.gate == BookAdmissionGate::IndependentCommitSet
                            && a.outcome == BookAdmissionOutcome::Accepted
                            && a.source_execution < index
                            && value(&a.canonical).ok() == Some(prior_value)
                    })
                    .ok_or(R::Origin)?;
                accepted(
                    book,
                    book.admissions
                        .iter()
                        .position(|a| core::ptr::eq(a, previous))
                        .ok_or(R::Origin)?,
                    BookAdmissionGate::IndependentCommitSet,
                )?;
                if !fixed(
                    book,
                    previous.source_execution,
                    "language-window8-qualified-independent-commit",
                ) && !fixed(
                    book,
                    previous.source_execution,
                    "language-window8-independent-commit-initialize",
                ) {
                    // Rebase needs its actual opaque predecessor/rebase graph;
                    // never silently treat it as an independent terminal leaf.
                    return Err(R::Origin);
                }
                plan.add(previous.source_execution)?;
                let analysis_value = field(query, "analysis")?;
                let analysis = book
                    .admissions
                    .iter()
                    .enumerate()
                    .rfind(|(_, a)| {
                        a.gate == BookAdmissionGate::QualifiedDependency
                            && a.outcome == BookAdmissionOutcome::Accepted
                            && a.source_execution < index
                            && value(&a.canonical).ok() == Some(analysis_value)
                    })
                    .ok_or(R::Origin)?;
                let analysis = accepted(book, analysis.0, BookAdmissionGate::QualifiedDependency)?;
                plan.add(analysis.source_execution)?;
                let edge = field(query, "edge")?;
                let anchor = book
                    .source
                    .iter()
                    .enumerate()
                    .take(index)
                    .rfind(|(i, h)| {
                        *i > analysis.source_execution
                            && fixed(book, *i, "language-window8-qualified-dependency-anchor")
                            && value(&h.input).ok() == Some(analysis_value)
                            && value(&h.output).ok() == Some(edge)
                    })
                    .ok_or(R::Origin)?;
                plan.add(anchor.0)?;
            } else if fixed(
                book,
                index,
                "language-window8-independent-commit-initialize",
            ) {
                let basis = field(value(&history.input)?, "basis")?;
                let initializer = book
                    .source
                    .iter()
                    .enumerate()
                    .take(index)
                    .rfind(|(i, h)| {
                        fixed(book, *i, "language-window8-initialize")
                            && value(&h.output).and_then(|v| field(v, "basis")).ok() == Some(basis)
                    })
                    .ok_or(R::Origin)?;
                plan.add(initializer.0)?;
            }
        }
        // Replay historical executions in their actual original order.
        plan.indices[..plan.count].sort_unstable();
        Ok(plan)
    }
}
