//! Unchanged original five-hop rank / insert / five-hop rank chain. Complete
//! canonical frames receive fresh Source and Native admission at every hop.
//! Ranking never authorizes a graph, fact or commitment.
use crate::{
    LanguageParserWindow8RawBeam,
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_values::{View, view},
};
use conduit_plot::rust_binding::PreparedNativeRustBinding;
const RANK: [&str; 5] = [
    "language-window8-rank-0-1",
    "language-window8-rank-2-3",
    "language-window8-rank-0-2",
    "language-window8-rank-1-3",
    "language-window8-rank-1-2",
];
#[derive(Debug)]
pub(crate) struct RankRefusal;
pub(crate) fn rank<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    mut beam: usize,
    epoch: u64,
    model_calls: u64,
) -> Result<usize, RankRefusal> {
    for name in RANK {
        let original = stage.book().source.get(beam).ok_or(RankRefusal)?;
        if !core::ptr::eq(
            original.output_descriptor(),
            LanguageParserWindow8RawBeam::PREPARED_DESCRIPTOR,
        ) {
            return Err(RankRefusal);
        }
        let query = queries
            .copy_record(
                name,
                view(original.output_bytes()).map_err(|_| RankRefusal)?,
            )
            .map_err(|_| RankRefusal)?;
        beam = stage
            .source_named(name, query, epoch, model_calls)
            .map_err(|_| RankRefusal)?;
    }
    Ok(beam)
}
/// The proposal is borrowed from the closed driver's actual retained prior or
/// accepted score-advance result, never a caller-supplied Native snapshot.
pub(crate) fn merge_query<'a>(
    queries: &'a mut PreparedWindow8Queries,
    beam: View<'_>,
    proposal: View<'_>,
) -> Result<&'a [u8], RankRefusal> {
    queries
        .record_fields(
            "language-window8-rank-insert",
            &[("beam", beam), ("proposal", proposal)],
        )
        .map_err(|_| RankRefusal)
}

/// Epoch accumulator contains only exact copies of an original Source-produced
/// inactive seed child. This introduces no active hypothesis or graph.
pub(crate) fn empty_accumulator<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut crate::parser_session_window8_atoms::PreparedWindow8Atoms,
    seed: usize,
    epoch: u64,
    model_calls: u64,
) -> Result<usize, RankRefusal> {
    use crate::parser_session_window8_atoms::{Window8Atom as A, Window8AtomField as F};
    use crate::parser_session_window8_values::{boolean, field};
    let original = stage.book().source.get(seed).ok_or(RankRefusal)?;
    let selected = crate::parser_session_window8_ports::PORTS
        .iter()
        .find(|p| p.name == "language-window8-session-seed")
        .ok_or(RankRefusal)?;
    if !original.matches_fixed(
        selected.original_programs,
        selected.original_custody,
        selected.input,
        selected.output,
    ) {
        return Err(RankRefusal);
    }
    let beam = view(original.output_bytes()).map_err(|_| RankRefusal)?;
    let inactive = field(beam, "candidate1").map_err(|_| RankRefusal)?;
    if boolean(field(inactive, "active").map_err(|_| RankRefusal)?).map_err(|_| RankRefusal)? {
        return Err(RankRefusal);
    }
    atoms
        .record_fields(
            A::Beam,
            &[
                (
                    "basis",
                    F::Observed(field(beam, "basis").map_err(|_| RankRefusal)?),
                ),
                ("candidate0", F::Observed(inactive)),
                ("candidate1", F::Observed(inactive)),
                ("candidate2", F::Observed(inactive)),
                ("candidate3", F::Observed(inactive)),
            ],
        )
        .map_err(|_| RankRefusal)?;
    let query = queries
        .copy_record(
            RANK[0],
            view(atoms.encoded(A::Beam).map_err(|_| RankRefusal)?).map_err(|_| RankRefusal)?,
        )
        .map_err(|_| RankRefusal)?;
    let mut result = stage
        .source_named(RANK[0], query, epoch, model_calls)
        .map_err(|_| RankRefusal)?;
    for name in &RANK[1..] {
        let query = queries
            .copy_record(
                name,
                view(stage.book().source[result].output_bytes()).map_err(|_| RankRefusal)?,
            )
            .map_err(|_| RankRefusal)?;
        result = stage
            .source_named(name, query, epoch, model_calls)
            .map_err(|_| RankRefusal)?;
    }
    Ok(result)
}
