//! Qualified dependency admission from complete retained lexical/Source parents.
//! A Native-valid anchor alone is never ancestry or commitment authority.
use crate::{
    parser_canonical_refinement::PreparedParserCanonicalRefinement,
    parser_session_pure_source::PureSourceHistory,
    parser_session_window8_book::{
        BookAdmission, BookAdmissionGate, BookAdmissionOutcome, Window8Book,
    },
    parser_session_window8_ports::PORTS,
    LanguageParserWindow8QualifiedDependencyAnalysis as Analysis,
    LanguageParserWindow8QualifiedDependencyProposal as Proposal,
    LanguageParserWindow8QualifiedLexicalAnalysis as LexicalAnalysis,
};
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, ValidatedCanonicalStructuredValue as Value,
};
use conduit_plot::rust_binding::{
    NativeBindingRefusal, PreparedNativeFamily, PreparedNativeRustBinding,
};

#[derive(Debug)]
pub(crate) enum QualifiedDependencyRefusal {
    Origin,
    Pressure,
    Refinement,
    Native(NativeBindingRefusal),
}
/// Fixed replay locators, resolved only against this actual unpublished book.
/// Every selected original Source and accepted lexical Analysis is replayed by
/// the stage before this witness is consumed. No caller-provided truth flag.
pub(crate) struct DependencyParents {
    source: [usize; 11],
    source_count: usize,
    lexical: [usize; 2],
    lexical_count: usize,
    proposal: usize,
}
fn value(bytes: &[u8]) -> Result<Value<'_>, QualifiedDependencyRefusal> {
    validate_canonical_structured_value(bytes).map_err(|_| QualifiedDependencyRefusal::Origin)
}
fn field<'a>(v: Value<'a>, name: &str) -> Result<Value<'a>, QualifiedDependencyRefusal> {
    v.record_field(name)
        .map_err(|_| QualifiedDependencyRefusal::Origin)?
        .ok_or(QualifiedDependencyRefusal::Origin)
}
fn fixed(history: &PureSourceHistory, name: &str) -> bool {
    PORTS.iter().find(|p| p.name == name).is_some_and(|p| {
        history.matches_fixed(p.original_programs, p.original_custody, p.input, p.output)
    })
}
fn projection(
    book: &Window8Book,
    name: &str,
    input: Value<'_>,
    output: Value<'_>,
    before: usize,
) -> Result<usize, QualifiedDependencyRefusal> {
    book.source
        .iter()
        .take(before)
        .position(|h| {
            fixed(h, name)
                && value(&h.input).ok() == Some(input)
                && value(&h.output).ok() == Some(output)
        })
        .ok_or(QualifiedDependencyRefusal::Origin)
}
impl DependencyParents {
    fn add_source(&mut self, index: usize) -> Result<(), QualifiedDependencyRefusal> {
        if !self.source[..self.source_count].contains(&index) {
            let slot = self
                .source
                .get_mut(self.source_count)
                .ok_or(QualifiedDependencyRefusal::Pressure)?;
            *slot = index;
            self.source_count += 1;
        }
        Ok(())
    }
    fn endpoint(
        &mut self,
        book: &Window8Book,
        anchor: Value<'_>,
        proposed: Value<'_>,
        snapshot: Value<'_>,
    ) -> Result<(), QualifiedDependencyRefusal> {
        // The anchor itself must have actually been produced by its fixed Source.
        let anchor_index = book
            .source
            .iter()
            .take(self.proposal)
            .rposition(|h| {
                fixed(h, "language-window8-qualified-lexical-anchor")
                    && value(&h.output).ok() == Some(anchor)
            })
            .ok_or(QualifiedDependencyRefusal::Origin)?;
        let context = value(&book.source[anchor_index].input)?;
        let scalars = field(context, "scalars")?;
        let token = field(context, "token")?;
        let origin = field(context, "origin")?;
        // All three projection entrances must be the SAME complete accepted
        // qualified lexical Analysis, not independently valid lookalike values.
        let mut found = None;
        for (index, admission) in book.admissions.iter().enumerate() {
            if admission.gate != BookAdmissionGate::QualifiedLexical
                || admission.outcome != BookAdmissionOutcome::Accepted
                || !core::ptr::eq(admission.descriptor, LexicalAnalysis::PREPARED_DESCRIPTOR)
            {
                continue;
            }
            let analysis = value(&admission.canonical)?;
            let query = field(analysis, "query")?;
            if field(query, "proposed")? != proposed
                || field(field(query, "fact")?, "snapshot")? != snapshot
            {
                continue;
            }
            let a = projection(
                book,
                "language-window8-qualified-lexical-anchor-scalars",
                analysis,
                scalars,
                anchor_index,
            );
            let b = projection(
                book,
                "language-window8-qualified-lexical-token",
                analysis,
                token,
                anchor_index,
            );
            let c = projection(
                book,
                "language-window8-qualified-lexical-origin",
                analysis,
                origin,
                anchor_index,
            );
            if let (Ok(a), Ok(b), Ok(c)) = (a, b, c) {
                found = Some((index, a, b, c));
                break;
            }
        }
        let (lexical, a, b, c) = found.ok_or(QualifiedDependencyRefusal::Origin)?;
        let admission = &book.admissions[lexical];
        let producing = book
            .source
            .get(admission.source_execution)
            .ok_or(QualifiedDependencyRefusal::Origin)?;
        if admission.source_execution >= a.min(b).min(c)
            || !fixed(producing, "language-window8-qualified-lexical-proposal")
            || field(value(&producing.output)?, "query")?
                != field(value(&admission.canonical)?, "query")?
        {
            return Err(QualifiedDependencyRefusal::Origin);
        }
        if !self.lexical[..self.lexical_count].contains(&lexical) {
            *self
                .lexical
                .get_mut(self.lexical_count)
                .ok_or(QualifiedDependencyRefusal::Pressure)? = lexical;
            self.lexical_count += 1;
        }
        for index in [admission.source_execution, a, b, c, anchor_index] {
            self.add_source(index)?;
        }
        Ok(())
    }
    pub(crate) fn resolve(
        book: &Window8Book,
        proposal: usize,
    ) -> Result<Self, QualifiedDependencyRefusal> {
        if book.published {
            return Err(QualifiedDependencyRefusal::Origin);
        }
        let revision = book
            .revision
            .as_ref()
            .ok_or(QualifiedDependencyRefusal::Origin)?;
        let proposed = value(revision.canonical_proposed_tape())?;
        let history = book
            .source
            .get(proposal)
            .ok_or(QualifiedDependencyRefusal::Origin)?;
        if !fixed(history, "language-window8-qualified-dependency-analysis") {
            return Err(QualifiedDependencyRefusal::Origin);
        }
        let query = field(value(&history.output)?, "query")?;
        if query != value(&history.input)? {
            return Err(QualifiedDependencyRefusal::Origin);
        }
        let snapshot = field(field(field(query, "context")?, "query")?, "snapshot")?;
        if field(field(snapshot, "lexical")?, "tape")? != field(proposed, "tape")? {
            return Err(QualifiedDependencyRefusal::Origin);
        }
        let mut parents = Self {
            source: [0; 11],
            source_count: 0,
            lexical: [0; 2],
            lexical_count: 0,
            proposal,
        };
        parents.endpoint(book, field(query, "dependent")?, proposed, snapshot)?;
        let head = field(query, "head")?;
        match head
            .variant_tag()
            .map_err(|_| QualifiedDependencyRefusal::Origin)?
        {
            "none" => {}
            "some" => parents.endpoint(
                book,
                head.variant_payload("some")
                    .map_err(|_| QualifiedDependencyRefusal::Origin)?
                    .ok_or(QualifiedDependencyRefusal::Origin)?,
                proposed,
                snapshot,
            )?,
            _ => return Err(QualifiedDependencyRefusal::Origin),
        }
        parents.add_source(proposal)?;
        Ok(parents)
    }
    /// The refinement preserves the complete record value node; only its
    /// original outer Type prefix changes. Admit backing size before replay.
    pub(crate) fn candidate_bytes(
        &self,
        book: &Window8Book,
    ) -> Result<usize, QualifiedDependencyRefusal> {
        let history = book
            .source
            .get(self.proposal)
            .ok_or(QualifiedDependencyRefusal::Origin)?;
        Analysis::PREPARED_DESCRIPTOR
            .type_bytes
            .len()
            .checked_add(value(&history.output)?.value_node().len())
            .ok_or(QualifiedDependencyRefusal::Pressure)
    }
    pub(crate) fn source_indices(&self) -> &[usize] {
        &self.source[..self.source_count]
    }
    pub(crate) fn lexical_indices(&self) -> &[usize] {
        &self.lexical[..self.lexical_count]
    }
}
pub(crate) struct Window8QualifiedDependencyAdmission {
    admission: BookAdmission,
}
impl Window8QualifiedDependencyAdmission {
    pub(crate) fn admit(
        book: &Window8Book,
        parents: &DependencyParents,
        refinement: &mut PreparedParserCanonicalRefinement<Proposal, Analysis>,
        family: &mut PreparedNativeFamily,
        mut buffer: Vec<u8>,
        maximum_bytes: usize,
    ) -> Result<Self, QualifiedDependencyRefusal> {
        use QualifiedDependencyRefusal as R;
        if buffer.capacity() > maximum_bytes || parents.candidate_bytes(book)? > buffer.capacity() {
            return Err(R::Pressure);
        }
        if !family.contains_descriptor(Analysis::PREPARED_DESCRIPTOR)
            || !family.contains_descriptor(Proposal::PREPARED_DESCRIPTOR)
            || !family.contains_descriptor(LexicalAnalysis::PREPARED_DESCRIPTOR)
        {
            return Err(R::Origin);
        }
        // A future schema change must not turn a nested Query refusal into an
        // outer provisional rejection. Both entrances have the exact same full
        // Query Type, and the original proposal is freshly admitted below.
        let source_query = crate::parser_canonical_schema::select_field(
            Proposal::PREPARED_DESCRIPTOR.type_bytes,
            &["query"],
        )
        .map_err(|_| R::Refinement)?;
        let target_query = crate::parser_canonical_schema::select_field(
            Analysis::PREPARED_DESCRIPTOR.type_bytes,
            &["query"],
        )
        .map_err(|_| R::Refinement)?;
        if source_query != target_query {
            return Err(R::Refinement);
        }
        // Re-resolve against the current actual book; stale locators do not grant
        // authority. There are at most two endpoints and eleven Source parents.
        let current = DependencyParents::resolve(book, parents.proposal)?;
        if current.source_indices() != parents.source_indices()
            || current.lexical_indices() != parents.lexical_indices()
        {
            return Err(R::Origin);
        }
        for index in parents.lexical_indices() {
            drop(
                family
                    .decode::<LexicalAnalysis>(&book.admissions[*index].canonical)
                    .map_err(R::Native)?,
            );
        }
        let history = &book.source[parents.proposal];
        // Fresh full proposal admission proves every unchanged nested query law
        // before any outer Analysis invariant can be a provisional rejection.
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
        if candidate.len() > buffer.capacity() {
            return Err(R::Pressure);
        }
        buffer.clear();
        buffer.extend_from_slice(candidate);
        if field(value(&buffer)?, "query")? != field(value(&history.output)?, "query")? {
            return Err(R::Origin);
        }
        let outcome = match family.decode::<Analysis>(&buffer) {
            Ok(v) => {
                drop(v);
                BookAdmissionOutcome::Accepted
            }
            Err(NativeBindingRefusal::ViolatedInvariant { index })
                if index < Analysis::PREPARED_DESCRIPTOR.laws.len() =>
            {
                BookAdmissionOutcome::ProvisionalInvariantRefusal {
                    original_law_index: index,
                }
            }
            Err(e) => return Err(R::Native(e)),
        };
        Ok(Self {
            admission: BookAdmission {
                gate: BookAdmissionGate::QualifiedDependency,
                canonical: buffer,
                descriptor: Analysis::PREPARED_DESCRIPTOR,
                source_execution: parents.proposal,
                outcome,
            },
        })
    }
    pub(crate) fn into_book(self) -> BookAdmission {
        self.admission
    }
}
