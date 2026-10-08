//! Feature-context assembly from an actual retained hypothesis parent. Original
//! Source and Native query laws remain the admission boundary.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A, Window8AtomField as F},
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_state::StateParent,
    parser_session_window8_values::{field, view},
};
#[derive(Debug)]
pub(crate) struct FeatureRefusal;
/// `parent` is a closed-driver locator into the actual retained Source book.
/// Choices are borrowed whole from that same hypothesis; no host POS choice.
pub(crate) fn context<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    atoms: &mut PreparedWindow8Atoms,
    queries: &mut PreparedWindow8Queries,
    parent: StateParent,
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
                (
                    "choices",
                    F::Observed(field(hypothesis, "choices").map_err(|_| FeatureRefusal)?),
                ),
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
