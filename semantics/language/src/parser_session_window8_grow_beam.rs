//! Private successor representation bridge. Only exact retained original grow
//! executions may replace a prior hypothesis's state/choices/selected fields.
//! Identity and score remain byte-for-byte prior Source values. This supplies no
//! fact/protection authority; the Session must rebase protected custody first.
use crate::{
    LanguageParserWindow8RawBeam, LanguageParserWindow8RawHypothesis,
    parser_canonical_composition::{
        ParserCompositionLimits, ParserCompositionReceipt, PreparedParserCanonicalComposer,
    },
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_book::Window8Book,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_values::{View, boolean, field, path, view},
};
use alloc::rc::Rc;
use conduit_plot::rust_binding::{PreparedNativeFamily, PreparedNativeRustBinding};
use core::cell::RefCell;
const CANDIDATES: [&str; 4] = ["candidate0", "candidate1", "candidate2", "candidate3"];
#[derive(Debug)]
pub(crate) struct GrowBeamRefusal;
/// Both locators belong to their explicitly supplied books, never a caller graph.
#[derive(Clone, Copy)]
pub(crate) struct GrowBeamParent {
    pub(crate) previous_candidate: u8,
    pub(crate) grow_execution: usize,
}
pub(crate) struct PreparedGrowBeam {
    hypothesis: PreparedParserCanonicalComposer,
    beam: PreparedParserCanonicalComposer,
    receipt: ParserCompositionReceipt,
}
impl PreparedGrowBeam {
    /// Allocation-free complete reservation. Choices are borrowed whole from
    /// grow's original collection; no collection builder or per-element rewrite.
    pub(crate) fn reservation(
        family: &PreparedNativeFamily,
        frame: usize,
    ) -> Result<ParserCompositionReceipt, GrowBeamRefusal> {
        let a = PreparedParserCanonicalComposer::descriptor_reservation(
            family,
            LanguageParserWindow8RawHypothesis::PREPARED_DESCRIPTOR,
            &[],
            frame,
        )
        .map_err(|_| GrowBeamRefusal)?;
        let b = PreparedParserCanonicalComposer::descriptor_reservation(
            family,
            LanguageParserWindow8RawBeam::PREPARED_DESCRIPTOR,
            &[],
            frame,
        )
        .map_err(|_| GrowBeamRefusal)?;
        Ok(ParserCompositionReceipt {
            preparation_requested_bytes_bound: a
                .preparation_requested_bytes_bound
                .checked_add(b.preparation_requested_bytes_bound)
                .ok_or(GrowBeamRefusal)?,
            retained_requested_bytes_bound: a
                .retained_requested_bytes_bound
                .checked_add(b.retained_requested_bytes_bound)
                .ok_or(GrowBeamRefusal)?,
        })
    }
    pub(crate) fn prepare(
        family: &PreparedNativeFamily,
        limits: ParserCompositionLimits,
    ) -> Result<Self, GrowBeamRefusal> {
        let receipt = Self::reservation(family, limits.maximum_output_bytes)?;
        // Combined two-composer ceiling precedes the first Type allocation.
        if receipt.preparation_requested_bytes_bound > limits.maximum_preparation_requested_bytes
            || receipt.retained_requested_bytes_bound > limits.maximum_retained_requested_bytes
        {
            return Err(GrowBeamRefusal);
        }
        Ok(Self {
            hypothesis: PreparedParserCanonicalComposer::prepare::<
                LanguageParserWindow8RawHypothesis,
            >(family, limits)
            .map_err(|_| GrowBeamRefusal)?,
            beam: PreparedParserCanonicalComposer::prepare::<LanguageParserWindow8RawBeam>(
                family, limits,
            )
            .map_err(|_| GrowBeamRefusal)?,
            receipt,
        })
    }
    pub(crate) fn receipt(&self) -> ParserCompositionReceipt {
        self.receipt
    }

    /// Returns a current-book RawBeam Source locator. All fixed inputs, new
    /// conversions and input ancestry are checked before first rank consumption.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn execute<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
        &mut self,
        stage: &mut Window8RevisionStage<'_, S, N>,
        previous: &Window8Book,
        previous_beam: usize,
        previous_seed: usize,
        parents: [Option<GrowBeamParent>; 4],
        family: &Rc<RefCell<PreparedNativeFamily>>,
        queries: &mut PreparedWindow8Queries,
        maximum_native_conversion_bytes: usize,
        epoch: u64,
        calls: u64,
    ) -> Result<usize, GrowBeamRefusal> {
        let result = (|| {
            if !previous.published
                || !stage
                    .book()
                    .previous
                    .as_deref()
                    .is_some_and(|p| core::ptr::eq(p, previous))
            {
                return Err(GrowBeamRefusal);
            }
            let count = parents.iter().flatten().count();
            if count == 0 {
                return Err(GrowBeamRefusal);
            }
            // One full hypothesis conversion per active parent, plus inactive
            // template and whole beam. Source ports admit their own conversions.
            let conversion = family
                .try_borrow()
                .map_err(|_| GrowBeamRefusal)?
                .storage_receipt()
                .conversion_requested_bytes_bound
                .checked_mul(count.checked_add(2).ok_or(GrowBeamRefusal)?)
                .ok_or(GrowBeamRefusal)?;
            if conversion > maximum_native_conversion_bytes {
                return Err(GrowBeamRefusal);
            }
            let old = previous.source.get(previous_beam).ok_or(GrowBeamRefusal)?;
            if !core::ptr::eq(
                old.output_descriptor(),
                LanguageParserWindow8RawBeam::PREPARED_DESCRIPTOR,
            ) {
                return Err(GrowBeamRefusal);
            }
            if ![
                "language-window8-session-seed",
                "language-window8-rank-0-1",
                "language-window8-rank-2-3",
                "language-window8-rank-0-2",
                "language-window8-rank-1-3",
                "language-window8-rank-1-2",
                "language-window8-rank-insert",
            ]
            .iter()
            .any(|name| matches_port(old, name).is_ok())
            {
                return Err(GrowBeamRefusal);
            }
            let prior_beam = view(old.output_bytes()).map_err(|_| GrowBeamRefusal)?;
            let original_seed = previous.source.get(previous_seed).ok_or(GrowBeamRefusal)?;
            matches_port(original_seed, "language-window8-session-seed")?;
            let inactive = field(
                view(original_seed.output_bytes()).map_err(|_| GrowBeamRefusal)?,
                "candidate1",
            )
            .map_err(|_| GrowBeamRefusal)?;
            if boolean(field(inactive, "active").map_err(|_| GrowBeamRefusal)?)
                .map_err(|_| GrowBeamRefusal)?
            {
                return Err(GrowBeamRefusal);
            }
            let mut seen = [false; 4];
            for parent in parents.iter().flatten() {
                let slot = usize::from(parent.previous_candidate);
                if slot >= 4 || seen[slot] {
                    return Err(GrowBeamRefusal);
                }
                seen[slot] = true;
                checked_grow(stage.book(), previous, prior_beam, *parent)?;
            }
            // No active predecessor may silently disappear from continuation.
            for (slot, name) in CANDIDATES.iter().enumerate() {
                if boolean(
                    field(
                        field(prior_beam, name).map_err(|_| GrowBeamRefusal)?,
                        "active",
                    )
                    .map_err(|_| GrowBeamRefusal)?,
                )
                .map_err(|_| GrowBeamRefusal)?
                    != seen[slot]
                {
                    return Err(GrowBeamRefusal);
                }
            }
            let first = parents.iter().flatten().next().ok_or(GrowBeamRefusal)?;
            let grown = view(stage.book().source[first.grow_execution].output_bytes())
                .map_err(|_| GrowBeamRefusal)?;
            let inactive_bytes = self
                .hypothesis
                .record(&[
                    field(inactive, "active").map_err(|_| GrowBeamRefusal)?,
                    field(grown, "choices").map_err(|_| GrowBeamRefusal)?,
                    field(inactive, "identity").map_err(|_| GrowBeamRefusal)?,
                    field(inactive, "score").map_err(|_| GrowBeamRefusal)?,
                    field(grown, "selected").map_err(|_| GrowBeamRefusal)?,
                    field(grown, "state").map_err(|_| GrowBeamRefusal)?,
                ])
                .map_err(|_| GrowBeamRefusal)?;
            drop(
                family
                    .try_borrow_mut()
                    .map_err(|_| GrowBeamRefusal)?
                    .decode::<LanguageParserWindow8RawHypothesis>(inactive_bytes)
                    .map_err(|_| GrowBeamRefusal)?,
            );
            let inactive = view(inactive_bytes).map_err(|_| GrowBeamRefusal)?;
            let empty = self
                .beam
                .record(&[inactive, inactive, inactive, inactive])
                .map_err(|_| GrowBeamRefusal)?;
            drop(
                family
                    .try_borrow_mut()
                    .map_err(|_| GrowBeamRefusal)?
                    .decode::<LanguageParserWindow8RawBeam>(empty)
                    .map_err(|_| GrowBeamRefusal)?,
            );
            let query = queries
                .copy_record(
                    "language-window8-rank-0-1",
                    view(empty).map_err(|_| GrowBeamRefusal)?,
                )
                .map_err(|_| GrowBeamRefusal)?;
            let mut beam = stage
                .source_named("language-window8-rank-0-1", query, epoch, calls)
                .map_err(|_| GrowBeamRefusal)?;
            for name in [
                "language-window8-rank-2-3",
                "language-window8-rank-0-2",
                "language-window8-rank-1-3",
                "language-window8-rank-1-2",
            ] {
                let query = queries
                    .copy_record(
                        name,
                        view(stage.book().source[beam].output_bytes())
                            .map_err(|_| GrowBeamRefusal)?,
                    )
                    .map_err(|_| GrowBeamRefusal)?;
                beam = stage
                    .source_named(name, query, epoch, calls)
                    .map_err(|_| GrowBeamRefusal)?;
            }
            for parent in parents.iter().flatten() {
                let prior = field(
                    prior_beam,
                    CANDIDATES[usize::from(parent.previous_candidate)],
                )
                .map_err(|_| GrowBeamRefusal)?;
                let grown = view(stage.book().source[parent.grow_execution].output_bytes())
                    .map_err(|_| GrowBeamRefusal)?;
                let hypothesis = self
                    .hypothesis
                    .record(&[
                        field(prior, "active").map_err(|_| GrowBeamRefusal)?,
                        field(grown, "choices").map_err(|_| GrowBeamRefusal)?,
                        field(prior, "identity").map_err(|_| GrowBeamRefusal)?,
                        field(prior, "score").map_err(|_| GrowBeamRefusal)?,
                        field(grown, "selected").map_err(|_| GrowBeamRefusal)?,
                        field(grown, "state").map_err(|_| GrowBeamRefusal)?,
                    ])
                    .map_err(|_| GrowBeamRefusal)?;
                drop(
                    family
                        .try_borrow_mut()
                        .map_err(|_| GrowBeamRefusal)?
                        .decode::<LanguageParserWindow8RawHypothesis>(hypothesis)
                        .map_err(|_| GrowBeamRefusal)?,
                );
                beam = crate::parser_session_window8_rank::rank(stage, queries, beam, epoch, calls)
                    .map_err(|_| GrowBeamRefusal)?;
                let query = crate::parser_session_window8_rank::merge_query(
                    queries,
                    view(stage.book().source[beam].output_bytes()).map_err(|_| GrowBeamRefusal)?,
                    view(hypothesis).map_err(|_| GrowBeamRefusal)?,
                )
                .map_err(|_| GrowBeamRefusal)?;
                let inserted = stage
                    .source_named("language-window8-rank-insert", query, epoch, calls)
                    .map_err(|_| GrowBeamRefusal)?;
                beam = crate::parser_session_window8_rank::rank(
                    stage, queries, inserted, epoch, calls,
                )
                .map_err(|_| GrowBeamRefusal)?;
            }
            Ok(beam)
        })();
        if result.is_err() {
            stage.abort();
        }
        result
    }
}
fn matches_port(
    history: &crate::parser_session_pure_source::PureSourceHistory,
    name: &str,
) -> Result<(), GrowBeamRefusal> {
    let port = crate::parser_session_window8_ports::PORTS
        .iter()
        .find(|p| p.name == name)
        .ok_or(GrowBeamRefusal)?;
    if !history.matches_fixed(
        port.original_programs,
        port.original_custody,
        port.input,
        port.output,
    ) {
        return Err(GrowBeamRefusal);
    }
    Ok(())
}
fn checked_grow(
    current: &Window8Book,
    previous: &Window8Book,
    beam: View<'_>,
    parent: GrowBeamParent,
) -> Result<(), GrowBeamRefusal> {
    let execution = current
        .source
        .get(parent.grow_execution)
        .ok_or(GrowBeamRefusal)?;
    matches_port(execution, "language-window8-grow")?;
    let input = view(execution.input_bytes()).map_err(|_| GrowBeamRefusal)?;
    let prior = field(beam, CANDIDATES[usize::from(parent.previous_candidate)])
        .map_err(|_| GrowBeamRefusal)?;
    let old_tape = field(
        view(
            previous
                .revision
                .as_ref()
                .ok_or(GrowBeamRefusal)?
                .canonical_proposed_tape(),
        )
        .map_err(|_| GrowBeamRefusal)?,
        "tape",
    )
    .map_err(|_| GrowBeamRefusal)?;
    let next_tape = field(
        view(
            current
                .revision
                .as_ref()
                .ok_or(GrowBeamRefusal)?
                .canonical_proposed_tape(),
        )
        .map_err(|_| GrowBeamRefusal)?,
        "tape",
    )
    .map_err(|_| GrowBeamRefusal)?;
    if path(input, &["previous", "context", "state", "state"]).map_err(|_| GrowBeamRefusal)?
        != field(prior, "state").map_err(|_| GrowBeamRefusal)?
        || field(input, "choices").map_err(|_| GrowBeamRefusal)?
            != field(prior, "choices").map_err(|_| GrowBeamRefusal)?
        || field(input, "selected").map_err(|_| GrowBeamRefusal)?
            != field(prior, "selected").map_err(|_| GrowBeamRefusal)?
        || path(input, &["previous", "context", "lexical", "tape"]).map_err(|_| GrowBeamRefusal)?
            != old_tape
        || path(input, &["next", "tape"]).map_err(|_| GrowBeamRefusal)? != next_tape
        || path(input, &["lineage", "previous"]).map_err(|_| GrowBeamRefusal)?
            != field(old_tape, "source").map_err(|_| GrowBeamRefusal)?
        || path(input, &["lineage", "next"]).map_err(|_| GrowBeamRefusal)?
            != field(next_tape, "source").map_err(|_| GrowBeamRefusal)?
    {
        return Err(GrowBeamRefusal);
    }
    Ok(())
}
