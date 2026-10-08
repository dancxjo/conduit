//! Original Source token-code and origin projection from the whole opaque
//! produced revision. Unknown origin remains intact; this grants no lexical fact.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A, Window8AtomField as F},
    parser_session_window8_collection::PreparedWindow8CanonicalCollection,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_values::{field, path, view},
};
#[derive(Debug)]
pub(crate) struct LexicalProjectionRefusal;
pub(crate) struct Window8LexicalDerivation {
    pub(crate) codes: [usize; 8],
    pub(crate) origins: usize,
}
pub(crate) fn project<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut PreparedWindow8Atoms,
    tokens: &mut PreparedWindow8CanonicalCollection,
    count: u64,
) -> Result<Window8LexicalDerivation, LexicalProjectionRefusal> {
    if count > 8 {
        return Err(LexicalProjectionRefusal);
    }
    let mut codes = [0usize; 8];
    for (ordinal, destination) in codes.iter_mut().enumerate() {
        atoms
            .unsigned(ordinal as u64)
            .map_err(|_| LexicalProjectionRefusal)?;
        let ordinal_frame = view(
            atoms
                .encoded(A::Unsigned)
                .map_err(|_| LexicalProjectionRefusal)?,
        )
        .map_err(|_| LexicalProjectionRefusal)?;
        let name = if (ordinal as u64) < count {
            "language-window8-token-codes"
        } else {
            "language-window8-empty-codes"
        };
        let query = if (ordinal as u64) < count {
            let proposed = view(
                stage
                    .book()
                    .revision
                    .as_ref()
                    .ok_or(LexicalProjectionRefusal)?
                    .canonical_proposed_tape(),
            )
            .map_err(|_| LexicalProjectionRefusal)?;
            let token = path(proposed, &["tape", "tokens"])
                .map_err(|_| LexicalProjectionRefusal)?
                .collection_index(ordinal as u16)
                .map_err(|_| LexicalProjectionRefusal)?
                .ok_or(LexicalProjectionRefusal)?;
            queries.record_fields(name, &[("ordinal", ordinal_frame), ("token", token)])
        } else {
            queries.record_fields(name, &[("ordinal", ordinal_frame)])
        }
        .map_err(|_| LexicalProjectionRefusal)?;
        *destination = stage
            .source_named(name, query, 0, 0)
            .map_err(|_| LexicalProjectionRefusal)?;
    }
    let first =
        view(stage.book().source[codes[0]].output_bytes()).map_err(|_| LexicalProjectionRefusal)?;
    let mut values = [first; 8];
    for (value, index) in values.iter_mut().zip(codes) {
        *value = view(stage.book().source[index].output_bytes())
            .map_err(|_| LexicalProjectionRefusal)?;
    }
    let token_frame = tokens
        .compose(values.into_iter())
        .map_err(|_| LexicalProjectionRefusal)?;
    let proposed = view(
        stage
            .book()
            .revision
            .as_ref()
            .ok_or(LexicalProjectionRefusal)?
            .canonical_proposed_tape(),
    )
    .map_err(|_| LexicalProjectionRefusal)?;
    let tape = field(proposed, "tape").map_err(|_| LexicalProjectionRefusal)?;
    atoms
        .unsigned(count)
        .map_err(|_| LexicalProjectionRefusal)?;
    atoms
        .record_fields(
            A::Projection,
            &[
                (
                    "profile_identity",
                    F::Observed(
                        path(tape, &["profile", "identity"])
                            .map_err(|_| LexicalProjectionRefusal)?,
                    ),
                ),
                (
                    "sequence",
                    F::Observed(
                        path(tape, &["source", "sequence"])
                            .map_err(|_| LexicalProjectionRefusal)?,
                    ),
                ),
                (
                    "source_revision",
                    F::Observed(
                        path(tape, &["source", "material", "revision"])
                            .map_err(|_| LexicalProjectionRefusal)?,
                    ),
                ),
                (
                    "text",
                    F::Observed(
                        path(tape, &["source", "material", "identity"])
                            .map_err(|_| LexicalProjectionRefusal)?,
                    ),
                ),
                ("token_count", F::Prepared(A::Unsigned)),
                (
                    "tokens",
                    F::Observed(view(token_frame).map_err(|_| LexicalProjectionRefusal)?),
                ),
            ],
        )
        .map_err(|_| LexicalProjectionRefusal)?;
    let query = queries
        .record_fields(
            "language-proposal-window8-origins",
            &[
                (
                    "lexical",
                    view(
                        atoms
                            .encoded(A::Lexical)
                            .map_err(|_| LexicalProjectionRefusal)?,
                    )
                    .map_err(|_| LexicalProjectionRefusal)?,
                ),
                ("proposed", proposed),
            ],
        )
        .map_err(|_| LexicalProjectionRefusal)?;
    let origins = stage
        .source_named("language-proposal-window8-origins", query, 0, 0)
        .map_err(|_| LexicalProjectionRefusal)?;
    atoms
        .record_fields(
            A::Origins,
            &[(
                "raw",
                F::Observed(
                    view(stage.book().source[origins].output_bytes())
                        .map_err(|_| LexicalProjectionRefusal)?,
                ),
            )],
        )
        .map_err(|_| LexicalProjectionRefusal)?;
    Ok(Window8LexicalDerivation { codes, origins })
}
