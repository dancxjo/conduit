//! First-revision execution through the original bounded parser. The armed
//! cancellation guard and unpublished book return to the higher Session; this
//! function neither commits facts nor treats an epoch limit as completion.
use crate::{
    LanguageParserWindow8QualifiedLexicalAnalysis, LanguageParserWindow8QualifiedLexicalProposal,
    parser_canonical_composition::PreparedParserCanonicalComposer,
    parser_canonical_refinement::PreparedParserCanonicalRefinement,
    parser_canonical_u64_collection::PreparedParserU64Collection,
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_stage::ParserSessionStage,
    parser_session_window8_ancestry::PreparedWindow8Ancestry,
    parser_session_window8_atoms::PreparedWindow8Atoms, parser_session_window8_book::Window8Book,
    parser_session_window8_collection::PreparedWindow8CanonicalCollection,
    parser_session_window8_model_frames::Window8ModelFrameLimits,
    parser_session_window8_observe::PreparedObservationFrames,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_registry::Window8Registry,
    parser_session_window8_snapshot::PreparedSnapshotFrames,
    parser_session_window8_stage::Window8RevisionStage,
};
use alloc::rc::Rc;
use conduit_plot::rust_binding::PreparedNativeFamily;
use core::cell::RefCell;
pub(crate) struct InitialRunBanks<'a> {
    pub(crate) queries: &'a mut PreparedWindow8Queries,
    pub(crate) atoms: &'a mut PreparedWindow8Atoms,
    pub(crate) ancestry: &'a mut PreparedWindow8Ancestry,
    pub(crate) tokens: &'a mut PreparedWindow8CanonicalCollection,
    pub(crate) choices: &'a mut PreparedParserU64Collection,
    pub(crate) snapshots: &'a mut PreparedSnapshotFrames,
    pub(crate) observations: &'a mut PreparedObservationFrames,
    pub(crate) fact: &'a mut PreparedParserCanonicalComposer,
    pub(crate) refinement: &'a mut PreparedParserCanonicalRefinement<
        LanguageParserWindow8QualifiedLexicalProposal,
        LanguageParserWindow8QualifiedLexicalAnalysis,
    >,
    pub(crate) family: &'a Rc<RefCell<PreparedNativeFamily>>,
}
pub(crate) struct InitialRunLimits<'a> {
    pub(crate) maximum_epochs: u64,
    pub(crate) model_frames: &'a Window8ModelFrameLimits,
    pub(crate) maximum_observation_native_conversion_requested_bytes: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InitialRunStop {
    WaitingEmpty,
    Complete,
    EpochLimit,
}
pub(crate) struct InitialRunResult {
    pub(crate) stop: InitialRunStop,
    pub(crate) beam: Option<usize>,
    pub(crate) epochs: u64,
    pub(crate) model_calls: u64,
}
#[derive(Debug)]
pub(crate) struct InitialRunRefusal;

pub(crate) fn execute<'a, S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    mut stage: Window8RevisionStage<'a, S, N>,
    banks: InitialRunBanks<'_>,
    analysis_identity: &[u8; 64],
    limits: InitialRunLimits<'_>,
) -> Result<
    (
        ParserSessionStage<'a, Window8Registry<S, N>>,
        Window8Book,
        InitialRunResult,
    ),
    InitialRunRefusal,
> {
    let result = (|| {
        if limits.maximum_epochs == 0 {
            return Err(InitialRunRefusal);
        }
        let initial = crate::parser_session_window8_initialize::initialize(
            &mut stage,
            banks.queries,
            banks.atoms,
            analysis_identity,
        )
        .map_err(|_| InitialRunRefusal)?;
        let Some(seed) = initial.beam else {
            return Ok(InitialRunResult {
                stop: InitialRunStop::WaitingEmpty,
                beam: None,
                epochs: 0,
                model_calls: 0,
            });
        };
        let observations = limits
            .maximum_epochs
            .checked_mul(initial.count)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(InitialRunRefusal)?;
        if observations > banks.observations.available_candidates() {
            return Err(InitialRunRefusal);
        }
        crate::parser_session_window8_lexical::project(
            &mut stage,
            banks.queries,
            banks.atoms,
            banks.tokens,
            initial.count,
        )
        .map_err(|_| InitialRunRefusal)?;
        let classes = crate::parser_session_window8_classes::derive_all(
            &mut stage,
            banks.queries,
            banks.atoms,
        )
        .map_err(|_| InitialRunRefusal)?;
        let mut beam = seed;
        // Original seed identities occupy zero through three; accepted Source
        // score-advance results alone consume each subsequent identity.
        let mut identity = 4;
        let mut calls = 0;
        let mut epochs = 0;
        let mut stop = InitialRunStop::EpochLimit;
        for epoch in 0..limits.maximum_epochs {
            beam = crate::parser_session_window8_epoch::execute(
                &mut stage,
                banks.queries,
                banks.atoms,
                banks.ancestry,
                banks.choices,
                limits.model_frames,
                &classes,
                seed,
                beam,
                &mut identity,
                epoch,
                &mut calls,
            )
            .map_err(|_| InitialRunRefusal)?;
            epochs = epoch.checked_add(1).ok_or(InitialRunRefusal)?;
            let completion = crate::parser_session_window8_epoch::completion(
                &mut stage,
                banks.queries,
                beam,
                epoch,
                calls,
            )
            .map_err(|_| InitialRunRefusal)?;
            if completion.active == 0 {
                return Err(InitialRunRefusal);
            }
            crate::parser_session_window8_observe::observe(
                &mut stage,
                banks.queries,
                banks.atoms,
                banks.ancestry,
                banks.snapshots,
                banks.observations,
                banks.fact,
                banks.refinement,
                banks.family,
                &classes,
                beam,
                limits.maximum_observation_native_conversion_requested_bytes,
                epoch,
                calls,
            )
            .map_err(|_| InitialRunRefusal)?;
            if completion.all_complete {
                stop = InitialRunStop::Complete;
                break;
            }
        }
        Ok(InitialRunResult {
            stop,
            beam: Some(beam),
            epochs,
            model_calls: calls,
        })
    })();
    match result {
        Err(error) => {
            stage.abort();
            Err(error)
        }
        Ok(result) => {
            let (guard, book) = stage
                .into_unpublished_parts()
                .map_err(|_| InitialRunRefusal)?;
            Ok((guard, book, result))
        }
    }
}
