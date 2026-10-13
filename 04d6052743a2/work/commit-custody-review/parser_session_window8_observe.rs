//! Per-epoch lexical observations from the actual retained beam. Accepted Native
//! analyses and provisional original-law refusals remain distinct book events.
use crate::{
    LanguageParserWindow8FactQuery, LanguageParserWindow8QualifiedLexicalAnalysis,
    LanguageParserWindow8QualifiedLexicalProposal, LanguageParserWindow8Snapshot,
    parser_canonical_composition::PreparedParserCanonicalComposer,
    parser_canonical_refinement::PreparedParserCanonicalRefinement,
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_ancestry::PreparedWindow8Ancestry,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A},
    parser_session_window8_classes::Window8ClassDerivation,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_snapshot::PreparedSnapshotFrames,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_state::StateParent,
    parser_session_window8_values::{field, unsigned, view},
};
use alloc::vec::Vec;
use conduit_plot::rust_binding::PreparedNativeFamily;
#[derive(Debug)]
pub(crate) struct ObservationRefusal;
pub(crate) struct PreparedObservationFrames {
    snapshot: Vec<u8>,
    candidates: Vec<Vec<u8>>,
}
impl PreparedObservationFrames {
    /// Reserves all candidate backing arrays before any revision is consumed.
    /// Inline owner storage is charged separately by the enclosing factory.
    pub(crate) fn prepare(
        count: usize,
        frame: usize,
        maximum_requested_bytes: usize,
    ) -> Result<Self, ObservationRefusal> {
        let bytes = count
            .checked_add(1)
            .and_then(|n| n.checked_mul(frame))
            .and_then(|n| {
                count
                    .checked_mul(core::mem::size_of::<Vec<u8>>())
                    .and_then(|v| n.checked_add(v))
            })
            .ok_or(ObservationRefusal)?;
        if count == 0
            || frame == 0
            || frame > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || bytes > maximum_requested_bytes
        {
            return Err(ObservationRefusal);
        }
        let mut snapshot = Vec::new();
        snapshot
            .try_reserve_exact(frame)
            .map_err(|_| ObservationRefusal)?;
        let mut candidates = Vec::new();
        candidates
            .try_reserve_exact(count)
            .map_err(|_| ObservationRefusal)?;
        if snapshot.capacity() != frame || candidates.capacity() != count {
            return Err(ObservationRefusal);
        }
        for _ in 0..count {
            let mut buffer = Vec::new();
            buffer
                .try_reserve_exact(frame)
                .map_err(|_| ObservationRefusal)?;
            if buffer.capacity() != frame {
                return Err(ObservationRefusal);
            }
            candidates.push(buffer);
        }
        Ok(Self {
            snapshot,
            candidates,
        })
    }
    pub(crate) fn available_candidates(&self) -> usize {
        self.candidates.len()
    }
    pub(crate) fn retained_capacity_bytes(&self) -> usize {
        self.snapshot.capacity()
            + self.candidates.capacity() * core::mem::size_of::<Vec<u8>>()
            + self.candidates.iter().map(Vec::capacity).sum::<usize>()
    }
}
pub(crate) struct EpochObservations {
    pub(crate) contexts: [usize; 4],
    pub(crate) admissions: [Option<usize>; 8],
    pub(crate) token_count: u8,
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn observe<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut PreparedWindow8Atoms,
    ancestry: &mut PreparedWindow8Ancestry,
    snapshot_frames: &mut PreparedSnapshotFrames,
    frames: &mut PreparedObservationFrames,
    fact: &mut PreparedParserCanonicalComposer,
    refinement: &mut PreparedParserCanonicalRefinement<
        LanguageParserWindow8QualifiedLexicalProposal,
        LanguageParserWindow8QualifiedLexicalAnalysis,
    >,
    family: &alloc::rc::Rc<core::cell::RefCell<PreparedNativeFamily>>,
    classes: &[Window8ClassDerivation; 76],
    beam: usize,
    maximum_native_conversion_requested_bytes: usize,
    epoch: u64,
    calls: u64,
) -> Result<EpochObservations, ObservationRefusal> {
    let result = (|| {
        let count = unsigned(
            field(
                StateParent::BeamCandidate {
                    execution: beam,
                    candidate: 0,
                }
                .state(stage.book())
                .map_err(|_| ObservationRefusal)?,
                "token_count",
            )
            .map_err(|_| ObservationRefusal)?,
        )
        .map_err(|_| ObservationRefusal)?;
        if count > 8 || frames.candidates.len() < count as usize {
            return Err(ObservationRefusal);
        }
        // Snapshot + each FactQuery + two explicit lexical gate conversions.
        // Original Source ingress/replay has its own separately admitted quota.
        let conversions = count
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .ok_or(ObservationRefusal)?;
        let conversion_bytes = family
            .try_borrow()
            .map_err(|_| ObservationRefusal)?
            .storage_receipt()
            .conversion_requested_bytes_bound
            .checked_mul(usize::try_from(conversions).map_err(|_| ObservationRefusal)?)
            .ok_or(ObservationRefusal)?;
        if conversion_bytes > maximum_native_conversion_requested_bytes {
            return Err(ObservationRefusal);
        }
        let mut contexts = [0; 4];
        for (candidate, context) in contexts.iter_mut().enumerate() {
            *context = crate::parser_session_window8_state::derive(
                stage,
                queries,
                atoms,
                ancestry,
                StateParent::BeamCandidate {
                    execution: beam,
                    candidate: candidate as u8,
                },
                &classes[39],
                epoch,
                calls,
            )
            .map_err(|_| ObservationRefusal)?
            .context;
        }
        let snapshot = crate::parser_session_window8_snapshot::compose(
            stage.book(),
            beam,
            contexts,
            snapshot_frames,
            atoms,
        )
        .map_err(|_| ObservationRefusal)?;
        if snapshot.len() > frames.snapshot.capacity() {
            return Err(ObservationRefusal);
        }
        frames.snapshot.clear();
        frames.snapshot.extend_from_slice(snapshot);
        drop(
            family
                .try_borrow_mut()
                .map_err(|_| ObservationRefusal)?
                .decode::<LanguageParserWindow8Snapshot>(&frames.snapshot)
                .map_err(|_| ObservationRefusal)?,
        );
        let mut admissions = [None; 8];
        for position in 0..count {
            atoms.unsigned(position).map_err(|_| ObservationRefusal)?;
            let fact_bytes = fact
                .record(&[
                    view(atoms.encoded(A::Unsigned).map_err(|_| ObservationRefusal)?)
                        .map_err(|_| ObservationRefusal)?,
                    view(&frames.snapshot).map_err(|_| ObservationRefusal)?,
                ])
                .map_err(|_| ObservationRefusal)?;
            drop(
                family
                    .try_borrow_mut()
                    .map_err(|_| ObservationRefusal)?
                    .decode::<LanguageParserWindow8FactQuery>(fact_bytes)
                    .map_err(|_| ObservationRefusal)?,
            );
            let proposed = view(
                stage
                    .book()
                    .revision
                    .as_ref()
                    .ok_or(ObservationRefusal)?
                    .canonical_proposed_tape(),
            )
            .map_err(|_| ObservationRefusal)?;
            let query = queries
                .record_fields(
                    "language-window8-qualified-lexical-proposal",
                    &[
                        ("proposed", proposed),
                        ("fact", view(fact_bytes).map_err(|_| ObservationRefusal)?),
                    ],
                )
                .map_err(|_| ObservationRefusal)?;
            let source = stage
                .source_named(
                    "language-window8-qualified-lexical-proposal",
                    query,
                    epoch,
                    calls,
                )
                .map_err(|_| ObservationRefusal)?;
            let buffer = frames.candidates.pop().ok_or(ObservationRefusal)?;
            admissions[position as usize] = Some(
                stage
                    .admit_qualified_lexical(
                        source,
                        refinement,
                        family,
                        buffer,
                        maximum_native_conversion_requested_bytes,
                        epoch,
                        calls,
                    )
                    .map_err(|_| ObservationRefusal)?,
            );
        }
        Ok(EpochObservations {
            contexts,
            admissions,
            token_count: count as u8,
        })
    })();
    if result.is_err() {
        stage.abort();
    }
    result
}
