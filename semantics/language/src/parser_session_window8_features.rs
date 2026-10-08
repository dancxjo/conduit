//! Feature-context assembly from an actual retained hypothesis parent. Original
//! Source and Native query laws remain the admission boundary.
use crate::{
    parser_canonical_u64_collection::PreparedParserU64Collection,
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A, Window8AtomField as F},
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_state::StateParent,
    parser_session_window8_values::{field, unsigned, view},
};
#[derive(Debug)]
pub(crate) struct FeatureRefusal;
/// `parent` is a closed-driver locator into the actual retained Source book.
/// All prior choices remain intact; an enumerated alternative can change only
/// the current unread occurrence, within its original Source-derived count.
#[allow(clippy::too_many_arguments)]
pub(crate) fn context<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    atoms: &mut PreparedWindow8Atoms,
    queries: &mut PreparedWindow8Queries,
    parent: StateParent,
    choice: Option<u64>,
    choices_buffer: &mut PreparedParserU64Collection,
    epoch: u64,
    model_calls: u64,
) -> Result<usize, FeatureRefusal> {
    let (execution, candidate) = match parent {
        StateParent::Hypothesis(execution) => (execution, None),
        StateParent::BeamCandidate {
            execution,
            candidate,
        } => (execution, Some(candidate)),
        StateParent::State(_) => return Err(FeatureRefusal),
    };
    // state() verifies the original output descriptor before the same frame is
    // used for its sibling choices. All parent fields remain canonical.
    let state = parent.state(stage.book()).map_err(|_| FeatureRefusal)?;
    let original = view(
        stage
            .book()
            .source
            .get(execution)
            .ok_or(FeatureRefusal)?
            .output_bytes(),
    )
    .map_err(|_| FeatureRefusal)?;
    let hypothesis = match candidate {
        None => original,
        Some(c) => field(
            original,
            *["candidate0", "candidate1", "candidate2", "candidate3"]
                .get(usize::from(c))
                .ok_or(FeatureRefusal)?,
        )
        .map_err(|_| FeatureRefusal)?,
    };
    let original_choices = field(hypothesis, "choices").map_err(|_| FeatureRefusal)?;
    let choices = if let Some(choice) = choice {
        let unread = unsigned(field(state, "unread").map_err(|_| FeatureRefusal)?)
            .map_err(|_| FeatureRefusal)?;
        let selected = unsigned(field(hypothesis, "selected").map_err(|_| FeatureRefusal)?)
            .map_err(|_| FeatureRefusal)?;
        let projection = view(atoms.encoded(A::Projection).map_err(|_| FeatureRefusal)?)
            .map_err(|_| FeatureRefusal)?;
        let count = unsigned(field(projection, "token_count").map_err(|_| FeatureRefusal)?)
            .map_err(|_| FeatureRefusal)?;
        if unread >= count || unread >= 8 || selected != unread {
            return Err(FeatureRefusal);
        }
        let codes = field(projection, "tokens")
            .map_err(|_| FeatureRefusal)?
            .collection_index(unread as u16)
            .map_err(|_| FeatureRefusal)?
            .ok_or(FeatureRefusal)?;
        if choice
            >= unsigned(field(codes, "count").map_err(|_| FeatureRefusal)?)
                .map_err(|_| FeatureRefusal)?
        {
            return Err(FeatureRefusal);
        }
        let mut values = [0; 8];
        for (index, destination) in values.iter_mut().enumerate() {
            *destination = unsigned(
                original_choices
                    .collection_index(index as u16)
                    .map_err(|_| FeatureRefusal)?
                    .ok_or(FeatureRefusal)?,
            )
            .map_err(|_| FeatureRefusal)?;
        }
        values[unread as usize] = choice;
        view(
            choices_buffer
                .compose(&values)
                .map_err(|_| FeatureRefusal)?,
        )
        .map_err(|_| FeatureRefusal)?
    } else {
        original_choices
    };
    atoms
        .record_fields(
            A::RawFeatureQuery,
            &[
                ("projection", F::Prepared(A::Projection)),
                ("state", F::Observed(state)),
                (
                    "expected_basis",
                    F::Observed(field(state, "basis").map_err(|_| FeatureRefusal)?),
                ),
                ("choices", F::Observed(choices)),
            ],
        )
        .map_err(|_| FeatureRefusal)?;
    let raw = view(
        atoms
            .encoded(A::RawFeatureQuery)
            .map_err(|_| FeatureRefusal)?,
    )
    .map_err(|_| FeatureRefusal)?;
    let origins =
        view(atoms.encoded(A::Origins).map_err(|_| FeatureRefusal)?).map_err(|_| FeatureRefusal)?;
    let query = queries
        .record_fields(
            "language-proposal-window8-feature-context",
            &[("raw", raw), ("origins", origins)],
        )
        .map_err(|_| FeatureRefusal)?;
    stage
        .source_named(
            "language-proposal-window8-feature-context",
            query,
            epoch,
            model_calls,
        )
        .map_err(|_| FeatureRefusal)
}

pub(crate) fn frontier<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    parent: StateParent,
    context: usize,
    epoch: u64,
    model_calls: u64,
) -> Result<usize, FeatureRefusal> {
    let result = (|| {
        let prior = parent
            .hypothesis(stage.book())
            .map_err(|_| FeatureRefusal)?;
        let original = stage.book().source.get(context).ok_or(FeatureRefusal)?;
        let selected = crate::parser_session_window8_ports::PORTS
            .iter()
            .find(|port| port.name == "language-proposal-window8-feature-context")
            .ok_or(FeatureRefusal)?;
        if !original.matches_fixed(
            selected.original_programs,
            selected.original_custody,
            selected.input,
            selected.output,
        ) {
            return Err(FeatureRefusal);
        }
        let raw = crate::parser_session_window8_values::path(
            view(original.output_bytes()).map_err(|_| FeatureRefusal)?,
            &["query", "raw"],
        )
        .map_err(|_| FeatureRefusal)?;
        let query = queries
            .record_fields(
                "language-window8-choice-frontier",
                &[
                    ("features", raw),
                    (
                        "prior_choices",
                        field(prior, "choices").map_err(|_| FeatureRefusal)?,
                    ),
                    (
                        "selected",
                        field(prior, "selected").map_err(|_| FeatureRefusal)?,
                    ),
                ],
            )
            .map_err(|_| FeatureRefusal)?;
        stage
            .source_named(
                "language-window8-choice-frontier",
                query,
                epoch,
                model_calls,
            )
            .map_err(|_| FeatureRefusal)
    })();
    if result.is_err() {
        stage.abort();
    }
    result
}
