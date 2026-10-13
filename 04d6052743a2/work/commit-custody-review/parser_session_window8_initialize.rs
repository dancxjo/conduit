//! First-revision initialization from the actual opaque produced tape. Later
//! revisions must use retained continuation/reanalysis/rebase, never this seed.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A, Window8AtomField as F},
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_values::{field, path, unsigned, view},
};
#[derive(Debug)]
pub(crate) struct InitializationRefusal;
pub(crate) struct Window8InitialExecution {
    pub(crate) availability: usize,
    pub(crate) initial: usize,
    pub(crate) beam: Option<usize>,
    pub(crate) count: u64,
}
pub(crate) fn initialize<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut PreparedWindow8Atoms,
    analysis_identity: &[u8; 64],
) -> Result<Window8InitialExecution, InitializationRefusal> {
    // Reinitializing an existing history would sever accepted facts/protection.
    if stage.book().previous.is_some() || !stage.book().source.is_empty() {
        return Err(InitializationRefusal);
    }
    let tape = field(
        view(
            stage
                .book()
                .revision
                .as_ref()
                .ok_or(InitializationRefusal)?
                .canonical_proposed_tape(),
        )
        .map_err(|_| InitializationRefusal)?,
        "tape",
    )
    .map_err(|_| InitializationRefusal)?;
    let query = queries
        .copy_record("language-window8-available", tape)
        .map_err(|_| InitializationRefusal)?;
    let availability = stage
        .source_named("language-window8-available", query, 0, 0)
        .map_err(|_| InitializationRefusal)?;
    let available = view(stage.book().source[availability].output_bytes())
        .map_err(|_| InitializationRefusal)?;
    let count = unsigned(field(available, "count").map_err(|_| InitializationRefusal)?)
        .map_err(|_| InitializationRefusal)?;
    if count > 8 {
        return Err(InitializationRefusal);
    }
    let tape = field(
        view(
            stage
                .book()
                .revision
                .as_ref()
                .ok_or(InitializationRefusal)?
                .canonical_proposed_tape(),
        )
        .map_err(|_| InitializationRefusal)?,
        "tape",
    )
    .map_err(|_| InitializationRefusal)?;
    atoms
        .record_fields(
            A::Lexical,
            &[
                ("tape", F::Observed(tape)),
                (
                    "token_count",
                    F::Observed(field(available, "count").map_err(|_| InitializationRefusal)?),
                ),
            ],
        )
        .map_err(|_| InitializationRefusal)?;
    atoms
        .leaf(A::AnalysisRevision, analysis_identity)
        .map_err(|_| InitializationRefusal)?;
    atoms
        .record_fields(
            A::Basis,
            &[
                ("analysis_revision", F::Prepared(A::AnalysisRevision)),
                (
                    "source_revision",
                    F::Observed(
                        path(tape, &["source", "material", "revision"])
                            .map_err(|_| InitializationRefusal)?,
                    ),
                ),
                (
                    "text",
                    F::Observed(
                        path(tape, &["source", "material", "identity"])
                            .map_err(|_| InitializationRefusal)?,
                    ),
                ),
            ],
        )
        .map_err(|_| InitializationRefusal)?;
    let basis = view(atoms.encoded(A::Basis).map_err(|_| InitializationRefusal)?)
        .map_err(|_| InitializationRefusal)?;
    let lexical = view(
        atoms
            .encoded(A::Lexical)
            .map_err(|_| InitializationRefusal)?,
    )
    .map_err(|_| InitializationRefusal)?;
    let default = view(
        atoms
            .encoded(A::DefaultRelation)
            .map_err(|_| InitializationRefusal)?,
    )
    .map_err(|_| InitializationRefusal)?;
    if count == 0 {
        let query = queries
            .record_fields(
                "language-window8-session-empty-seed",
                &[
                    ("basis", basis),
                    ("lexical", lexical),
                    ("default_relation", default),
                ],
            )
            .map_err(|_| InitializationRefusal)?;
        let initial = stage
            .source_named("language-window8-session-empty-seed", query, 0, 0)
            .map_err(|_| InitializationRefusal)?;
        return Ok(Window8InitialExecution {
            availability,
            initial,
            beam: None,
            count,
        });
    }
    let query = queries
        .record_fields(
            "language-window8-initialize",
            &[
                ("basis", basis),
                ("default_relation", default),
                (
                    "token_count",
                    field(available, "count").map_err(|_| InitializationRefusal)?,
                ),
            ],
        )
        .map_err(|_| InitializationRefusal)?;
    let initial = stage
        .source_named("language-window8-initialize", query, 0, 0)
        .map_err(|_| InitializationRefusal)?;
    atoms.unsigned(0).map_err(|_| InitializationRefusal)?;
    let query = queries
        .record_fields(
            "language-window8-session-seed",
            &[
                (
                    "initial",
                    view(stage.book().source[initial].output_bytes())
                        .map_err(|_| InitializationRefusal)?,
                ),
                (
                    "lexical",
                    view(
                        atoms
                            .encoded(A::Lexical)
                            .map_err(|_| InitializationRefusal)?,
                    )
                    .map_err(|_| InitializationRefusal)?,
                ),
                (
                    "identity",
                    view(
                        atoms
                            .encoded(A::Unsigned)
                            .map_err(|_| InitializationRefusal)?,
                    )
                    .map_err(|_| InitializationRefusal)?,
                ),
            ],
        )
        .map_err(|_| InitializationRefusal)?;
    let beam = stage
        .source_named("language-window8-session-seed", query, 0, 0)
        .map_err(|_| InitializationRefusal)?;
    Ok(Window8InitialExecution {
        availability,
        initial,
        beam: Some(beam),
        count,
    })
}
