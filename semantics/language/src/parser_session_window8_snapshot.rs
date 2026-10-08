//! Snapshot representation from one retained ranked beam and its exact original
//! Source proof parents. Composition grants no Native/fact/commit authority.
use crate::{
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A, Window8AtomField as F},
    parser_session_window8_book::Window8Book,
    parser_session_window8_ports::PORTS,
    parser_session_window8_values::{field, path, unsigned, view, View},
};
#[derive(Clone, Copy, Debug)]
pub(crate) struct SnapshotRefusal;
pub(crate) struct PreparedSnapshotFrames {
    checked: [alloc::vec::Vec<u8>; 4],
    lexical: alloc::vec::Vec<u8>,
}
impl PreparedSnapshotFrames {
    pub(crate) fn prepare(
        maximum_frame_bytes: usize,
        maximum_preparation_bytes: usize,
        maximum_retained_bytes: usize,
    ) -> Result<Self, SnapshotRefusal> {
        let bytes = maximum_frame_bytes.checked_mul(5).ok_or(SnapshotRefusal)?;
        if maximum_frame_bytes == 0
            || maximum_frame_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || bytes > maximum_preparation_bytes
            || bytes > maximum_retained_bytes
        {
            return Err(SnapshotRefusal);
        }
        let mut result = Self {
            checked: core::array::from_fn(|_| alloc::vec::Vec::new()),
            lexical: alloc::vec::Vec::new(),
        };
        for buffer in result
            .checked
            .iter_mut()
            .chain(core::iter::once(&mut result.lexical))
        {
            buffer
                .try_reserve_exact(maximum_frame_bytes)
                .map_err(|_| SnapshotRefusal)?;
            if buffer.capacity() != maximum_frame_bytes {
                return Err(SnapshotRefusal);
            }
        }
        Ok(result)
    }
    pub(crate) fn retained_capacity_bytes(&self) -> usize {
        self.checked
            .iter()
            .map(alloc::vec::Vec::capacity)
            .sum::<usize>()
            + self.lexical.capacity()
    }
}
fn fixed(
    source: &[crate::parser_session_pure_source::PureSourceHistory],
    index: usize,
    name: &str,
) -> bool {
    source.get(index).is_some_and(|h| {
        PORTS.iter().find(|p| p.name == name).is_some_and(|p| {
            h.matches_fixed(p.original_programs, p.original_custody, p.input, p.output)
        })
    })
}
fn observed(bytes: &[u8]) -> Result<View<'_>, SnapshotRefusal> {
    view(bytes).map_err(|_| SnapshotRefusal)
}
fn member<'a>(v: View<'a>, name: &str) -> Result<View<'a>, SnapshotRefusal> {
    field(v, name).map_err(|_| SnapshotRefusal)
}
fn nested<'a>(v: View<'a>, names: &[&str]) -> Result<View<'a>, SnapshotRefusal> {
    path(v, names).map_err(|_| SnapshotRefusal)
}
/// Every selected history precedes the proof-consuming context. Walk values
/// alone are insufficient: all eight follow executions and initialization must
/// be retained with exact complete input/output continuity.
fn walk_parent(
    source: &[crate::parser_session_pure_source::PureSourceHistory],
    before: usize,
    ancestry: View<'_>,
    heads: View<'_>,
    start: usize,
) -> Result<(), SnapshotRefusal> {
    if member(ancestry, "heads")? != heads
        || unsigned(member(ancestry, "start")?).map_err(|_| SnapshotRefusal)? != start as u64
    {
        return Err(SnapshotRefusal);
    }
    let mut index = source
        .iter()
        .enumerate()
        .take(before)
        .rposition(|(i, h)| {
            if !fixed(source, i, "language-window8-walk-follow") {
                return false;
            }
            let Ok(v) = observed(&h.output) else {
                return false;
            };
            nested(v, &["query", "heads"]).ok() == Some(heads)
                && nested(v, &["query", "start"]).ok() == member(ancestry, "start").ok()
                && member(v, "path").ok() == member(ancestry, "path").ok()
        })
        .ok_or(SnapshotRefusal)?;
    for _ in 0..7 {
        let input = observed(&source[index].input)?;
        index = source
            .iter()
            .enumerate()
            .take(index)
            .rposition(|(i, h)| {
                fixed(source, i, "language-window8-walk-follow")
                    && observed(&h.output).ok() == Some(input)
            })
            .ok_or(SnapshotRefusal)?;
    }
    let input = observed(&source[index].input)?;
    let initial = source
        .iter()
        .enumerate()
        .take(index)
        .rposition(|(i, h)| {
            fixed(source, i, "language-window8-walk-initialize")
                && observed(&h.output).ok() == Some(input)
        })
        .ok_or(SnapshotRefusal)?;
    let query = observed(&source[initial].input)?;
    if member(query, "heads")? != heads || member(query, "start")? != member(ancestry, "start")? {
        return Err(SnapshotRefusal);
    }
    Ok(())
}
/// The four proof consumers are original move-context executions from this book,
/// with exact StateProof/state identity for each candidate of the same beam.
/// The resulting full Snapshot still requires ordinary fresh Native admission.
pub(crate) fn compose<'a>(
    book: &Window8Book,
    beam: usize,
    contexts: [usize; 4],
    frames: &mut PreparedSnapshotFrames,
    atoms: &'a mut PreparedWindow8Atoms,
) -> Result<&'a [u8], SnapshotRefusal> {
    if book.published || !fixed(&book.source, beam, "language-window8-rank-1-2") {
        return Err(SnapshotRefusal);
    }
    let revision = book.revision.as_ref().ok_or(SnapshotRefusal)?;
    let proposed = observed(revision.canonical_proposed_tape())?;
    let ranked = observed(&book.source[beam].output)?;
    let names = ["candidate0", "candidate1", "candidate2", "candidate3"];
    let checked = &mut frames.checked;
    for (slot, (name, context)) in names.into_iter().zip(contexts).enumerate() {
        if context <= beam || !fixed(&book.source, context, "language-window8-move-context") {
            return Err(SnapshotRefusal);
        }
        let hypothesis = member(ranked, name)?;
        let state = member(hypothesis, "state")?;
        let proof = member(observed(&book.source[context].input)?, "state")?;
        if member(proof, "state")? != state {
            return Err(SnapshotRefusal);
        }
        let ancestry = member(proof, "ancestry")?;
        for start in 0..8 {
            let ancestor = ancestry
                .collection_index(start as u16)
                .map_err(|_| SnapshotRefusal)?
                .ok_or(SnapshotRefusal)?;
            walk_parent(
                &book.source,
                context,
                ancestor,
                member(state, "heads")?,
                start,
            )?;
        }
        if !book.source.iter().enumerate().take(context).any(|(i, h)| {
            fixed(&book.source, i, "language-window8-root-count")
                && observed(&h.input).ok() == Some(state)
                && observed(&h.output)
                    .ok()
                    .and_then(|v| member(v, "count").ok())
                    == member(proof, "roots").ok()
        }) {
            return Err(SnapshotRefusal);
        }
        atoms
            .record_fields(
                A::CheckedHypothesis,
                &[
                    ("hypothesis", F::Observed(hypothesis)),
                    ("proof", F::Observed(proof)),
                ],
            )
            .map_err(|_| SnapshotRefusal)?;
        let encoded = atoms
            .encoded(A::CheckedHypothesis)
            .map_err(|_| SnapshotRefusal)?;
        if encoded.len() > checked[slot].capacity() {
            return Err(SnapshotRefusal);
        }
        checked[slot].clear();
        checked[slot].extend_from_slice(encoded);
    }
    let state = nested(ranked, &["candidate0", "state"])?;
    atoms
        .record_fields(
            A::Lexical,
            &[
                ("tape", F::Observed(member(proposed, "tape")?)),
                ("token_count", F::Observed(member(state, "token_count")?)),
            ],
        )
        .map_err(|_| SnapshotRefusal)?;
    let encoded = atoms.encoded(A::Lexical).map_err(|_| SnapshotRefusal)?;
    if encoded.len() > frames.lexical.capacity() {
        return Err(SnapshotRefusal);
    }
    frames.lexical.clear();
    frames.lexical.extend_from_slice(encoded);
    let lexical = &frames.lexical;
    atoms
        .record_fields(
            A::Snapshot,
            &[
                ("lexical", F::Observed(observed(&lexical)?)),
                ("basis", F::Observed(member(state, "basis")?)),
                ("candidate0", F::Observed(observed(&checked[0])?)),
                ("candidate1", F::Observed(observed(&checked[1])?)),
                ("candidate2", F::Observed(observed(&checked[2])?)),
                ("candidate3", F::Observed(observed(&checked[3])?)),
            ],
        )
        .map_err(|_| SnapshotRefusal)?;
    atoms.encoded(A::Snapshot).map_err(|_| SnapshotRefusal)
}

#[cfg(test)]
pub(crate) fn verify_fixture_walk(
    source: &[crate::parser_session_pure_source::PureSourceHistory],
    ancestry: View<'_>,
    heads: View<'_>,
    start: usize,
) -> Result<(), SnapshotRefusal> {
    walk_parent(source, source.len(), ancestry, heads, start)
}
