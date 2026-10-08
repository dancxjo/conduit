//! One original four-slot epoch. Every transition and merge retains its full
//! original Source execution; numerical scores never authorize commitment.
use crate::{
    parser_canonical_u64_collection::PreparedParserU64Collection,
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_ancestry::PreparedWindow8Ancestry,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A},
    parser_session_window8_classes::Window8ClassDerivation,
    parser_session_window8_model_frames::Window8ModelFrameLimits,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_state::StateParent,
    parser_session_window8_values::{boolean, field, unsigned, view},
};
#[derive(Debug)]
pub(crate) struct EpochRefusal;

fn merge<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    beam: usize,
    parent: StateParent,
    epoch: u64,
    calls: u64,
) -> Result<usize, EpochRefusal> {
    use crate::parser_session_window8_rank as rank;
    let beam = rank::rank(stage, queries, beam, epoch, calls).map_err(|_| EpochRefusal)?;
    let query = rank::merge_query(
        queries,
        view(stage.book().source[beam].output_bytes()).map_err(|_| EpochRefusal)?,
        parent.hypothesis(stage.book()).map_err(|_| EpochRefusal)?,
    )
    .map_err(|_| EpochRefusal)?;
    let inserted = stage
        .source_named("language-window8-rank-insert", query, epoch, calls)
        .map_err(|_| EpochRefusal)?;
    rank::rank(stage, queries, inserted, epoch, calls).map_err(|_| EpochRefusal)
}

/// Caller admits finite epoch/book/model quotas before entering. This function
/// owns no growable working storage and never retries a fatal admission error.
#[allow(clippy::too_many_arguments)]
pub(crate) fn execute<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut PreparedWindow8Atoms,
    ancestry: &mut PreparedWindow8Ancestry,
    choices: &mut PreparedParserU64Collection,
    frames: &Window8ModelFrameLimits,
    classes: &[Window8ClassDerivation; 76],
    seed: usize,
    beam: usize,
    identity: &mut u64,
    epoch: u64,
    calls: &mut u64,
) -> Result<usize, EpochRefusal> {
    let result = (|| {
        let mut next = crate::parser_session_window8_rank::empty_accumulator(
            stage, queries, atoms, seed, epoch, *calls,
        )
        .map_err(|_| EpochRefusal)?;
        for candidate in 0..4 {
            let parent = StateParent::BeamCandidate {
                execution: beam,
                candidate,
            };
            let prior = parent.hypothesis(stage.book()).map_err(|_| EpochRefusal)?;
            if !boolean(field(prior, "active").map_err(|_| EpochRefusal)?)
                .map_err(|_| EpochRefusal)?
            {
                continue;
            }
            let query = queries
                .copy_record(
                    "language-window8-complete",
                    parent.state(stage.book()).map_err(|_| EpochRefusal)?,
                )
                .map_err(|_| EpochRefusal)?;
            let completion = stage
                .source_named("language-window8-complete", query, epoch, *calls)
                .map_err(|_| EpochRefusal)?;
            if boolean(
                field(
                    view(stage.book().source[completion].output_bytes())
                        .map_err(|_| EpochRefusal)?,
                    "complete",
                )
                .map_err(|_| EpochRefusal)?,
            )
            .map_err(|_| EpochRefusal)?
            {
                next = merge(stage, queries, next, parent, epoch, *calls)?;
                continue;
            }
            let derivation = crate::parser_session_window8_state::derive(
                stage,
                queries,
                atoms,
                ancestry,
                parent,
                &classes[39],
                epoch,
                *calls,
            )
            .map_err(|_| EpochRefusal)?;
            let unread = unsigned(
                field(
                    parent.state(stage.book()).map_err(|_| EpochRefusal)?,
                    "unread",
                )
                .map_err(|_| EpochRefusal)?,
            )
            .map_err(|_| EpochRefusal)?;
            let selected = unsigned(
                field(
                    parent.hypothesis(stage.book()).map_err(|_| EpochRefusal)?,
                    "selected",
                )
                .map_err(|_| EpochRefusal)?,
            )
            .map_err(|_| EpochRefusal)?;
            let projection = view(atoms.encoded(A::Projection).map_err(|_| EpochRefusal)?)
                .map_err(|_| EpochRefusal)?;
            let count = unsigned(field(projection, "token_count").map_err(|_| EpochRefusal)?)
                .map_err(|_| EpochRefusal)?;
            let choose = unread < count && selected == unread;
            let alternatives = if choose {
                if unread >= 8 {
                    return Err(EpochRefusal);
                }
                let codes = field(projection, "tokens")
                    .map_err(|_| EpochRefusal)?
                    .collection_index(unread as u16)
                    .map_err(|_| EpochRefusal)?
                    .ok_or(EpochRefusal)?;
                unsigned(field(codes, "count").map_err(|_| EpochRefusal)?)
                    .map_err(|_| EpochRefusal)?
            } else {
                1
            };
            for choice in 0..alternatives {
                let context = crate::parser_session_window8_features::context(
                    stage,
                    atoms,
                    queries,
                    parent,
                    choose.then_some(choice),
                    choices,
                    epoch,
                    *calls,
                )
                .map_err(|_| EpochRefusal)?;
                let frontier = crate::parser_session_window8_features::frontier(
                    stage, queries, parent, context, epoch, *calls,
                )
                .map_err(|_| EpochRefusal)?;
                let model = crate::parser_session_window8_model_call::execute(
                    stage, queries, frames, context, epoch, calls,
                )
                .map_err(|_| EpochRefusal)?;
                for class in classes {
                    let Some(outcome) = crate::parser_session_window8_transition::propose(
                        stage,
                        queries,
                        &derivation,
                        class,
                        epoch,
                        *calls,
                    )
                    .map_err(|_| EpochRefusal)?
                    else {
                        continue;
                    };
                    let proposal = crate::parser_session_window8_transition::advance(
                        stage,
                        queries,
                        atoms,
                        &derivation,
                        class,
                        context,
                        frontier,
                        model,
                        outcome,
                        identity,
                        epoch,
                        *calls,
                    )
                    .map_err(|_| EpochRefusal)?;
                    next = merge(
                        stage,
                        queries,
                        next,
                        StateParent::Hypothesis(proposal),
                        epoch,
                        *calls,
                    )?;
                }
            }
        }
        Ok(next)
    })();
    if result.is_err() {
        stage.abort();
    }
    result
}
