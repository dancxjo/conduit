//! Original finite state derivation over retained Source parents. Every walk and
//! root-count execution remains in the revision book; the full assembled proof
//! receives fresh Native admission as the original move-context input.
use crate::{
    LanguageParserWindow8RawBeam, LanguageParserWindow8RawHypothesis,
    LanguageParserWindow8RawState,
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_ancestry::PreparedWindow8Ancestry,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A, Window8AtomField as F},
    parser_session_window8_book::Window8Book,
    parser_session_window8_classes::Window8ClassDerivation,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_values::{field, path, unsigned, view, View},
};
use conduit_plot::rust_binding::PreparedNativeRustBinding;
const CANDIDATES: [&str; 4] = ["candidate0", "candidate1", "candidate2", "candidate3"];
#[derive(Clone, Copy, Debug)]
pub(crate) enum StateParent {
    State(usize),
    Hypothesis(usize),
    BeamCandidate { execution: usize, candidate: u8 },
}
#[derive(Debug)]
pub(crate) struct StateDerivationRefusal;
impl StateParent {
    pub(crate) fn state(self, book: &Window8Book) -> Result<View<'_>, StateDerivationRefusal> {
        let (execution, descriptor) = match self {
            Self::State(e) => (e, LanguageParserWindow8RawState::PREPARED_DESCRIPTOR),
            Self::Hypothesis(e) => (e, LanguageParserWindow8RawHypothesis::PREPARED_DESCRIPTOR),
            Self::BeamCandidate { execution, .. } => (execution, LanguageParserWindow8RawBeam::PREPARED_DESCRIPTOR),
        };
        let history = book.source.get(execution).ok_or(StateDerivationRefusal)?;
        if !core::ptr::eq(history.output_descriptor(), descriptor) {
            return Err(StateDerivationRefusal);
        }
        let value = view(history.output_bytes()).map_err(|_| StateDerivationRefusal)?;
        match self {
            Self::State(_) => Ok(value),
            Self::Hypothesis(_) => field(value, "state").map_err(|_| StateDerivationRefusal),
            Self::BeamCandidate { candidate, .. } => path(value, &[
                CANDIDATES.get(usize::from(candidate)).ok_or(StateDerivationRefusal)?, "state",
            ]).map_err(|_| StateDerivationRefusal),
        }
    }
}
pub(crate) struct Window8StateDerivation {
    pub(crate) parent: StateParent,
    pub(crate) walks: [usize; 8],
    pub(crate) roots: usize,
    pub(crate) context: usize,
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut PreparedWindow8Atoms,
    ancestry: &mut PreparedWindow8Ancestry,
    parent: StateParent,
    right_arc_class: &Window8ClassDerivation,
    epoch: u64,
    model_calls: u64,
) -> Result<Window8StateDerivation, StateDerivationRefusal> {
    let mut walks = [0usize; 8];
    for (start, destination) in walks.iter_mut().enumerate() {
        atoms.unsigned(start as u64).map_err(|_| StateDerivationRefusal)?;
        let query = queries.record_fields("language-window8-walk-initialize", &[
            ("heads", field(parent.state(stage.book())?, "heads").map_err(|_| StateDerivationRefusal)?),
            ("start", view(atoms.encoded(A::Unsigned).map_err(|_| StateDerivationRefusal)?)
                .map_err(|_| StateDerivationRefusal)?),
        ]).map_err(|_| StateDerivationRefusal)?;
        let mut walk = stage.source_named("language-window8-walk-initialize", query, epoch, model_calls)
            .map_err(|_| StateDerivationRefusal)?;
        for _ in 0..8 {
            let query = queries.copy_record("language-window8-walk-follow",
                view(stage.book().source[walk].output_bytes()).map_err(|_| StateDerivationRefusal)?)
                .map_err(|_| StateDerivationRefusal)?;
            walk = stage.source_named("language-window8-walk-follow", query, epoch, model_calls)
                .map_err(|_| StateDerivationRefusal)?;
        }
        *destination = walk;
    }
    let query = queries.copy_record("language-window8-root-count", parent.state(stage.book())?)
        .map_err(|_| StateDerivationRefusal)?;
    let roots = stage.source_named("language-window8-root-count", query, epoch, model_calls)
        .map_err(|_| StateDerivationRefusal)?;
    let first = view(stage.book().source[walks[0]].output_bytes()).map_err(|_| StateDerivationRefusal)?;
    let mut walk_values = [first; 8];
    for (value, execution) in walk_values.iter_mut().zip(walks) {
        *value = view(stage.book().source[execution].output_bytes()).map_err(|_| StateDerivationRefusal)?;
    }
    let ancestry_frame = ancestry.compose(walk_values).map_err(|_| StateDerivationRefusal)?;
    atoms.record_fields(A::StateProof, &[
        ("state", F::Observed(parent.state(stage.book())?)),
        ("ancestry", F::Observed(view(ancestry_frame).map_err(|_| StateDerivationRefusal)?)),
        ("roots", F::Observed(field(view(stage.book().source[roots].output_bytes())
            .map_err(|_| StateDerivationRefusal)?, "count").map_err(|_| StateDerivationRefusal)?)),
    ]).map_err(|_| StateDerivationRefusal)?;
    let state = parent.state(stage.book())?;
    let depth = unsigned(field(state, "depth").map_err(|_| StateDerivationRefusal)?)
        .map_err(|_| StateDerivationRefusal)?;
    let top = unsigned(field(state, "stack").map_err(|_| StateDerivationRefusal)?
        .collection_index(u16::try_from(depth.checked_sub(1).ok_or(StateDerivationRefusal)?)
            .map_err(|_| StateDerivationRefusal)?)
        .map_err(|_| StateDerivationRefusal)?.ok_or(StateDerivationRefusal)?)
        .map_err(|_| StateDerivationRefusal)?;
    let witness = view(ancestry_frame).map_err(|_| StateDerivationRefusal)?
        .collection_index(if top < 8 { top as u16 } else { 0 })
        .map_err(|_| StateDerivationRefusal)?.ok_or(StateDerivationRefusal)?;
    if right_arc_class.code() != 39 { return Err(StateDerivationRefusal); }
    let class = right_arc_class.value(stage.book()).map_err(|_| StateDerivationRefusal)?;
    // Action is borrowed from the retained original class-39 Source result;
    // the driver never constructs a substitute action or legality predicate.
    let action = field(class, "action")
        .map_err(|_| StateDerivationRefusal)?;
    let query = queries.record_fields("language-window8-move-context", &[
        ("basis", field(state, "basis").map_err(|_| StateDerivationRefusal)?),
        ("state", view(atoms.encoded(A::StateProof).map_err(|_| StateDerivationRefusal)?)
            .map_err(|_| StateDerivationRefusal)?),
        ("action", action),
        ("relation", field(state, "relation0").map_err(|_| StateDerivationRefusal)?),
        ("witness", witness),
    ]).map_err(|_| StateDerivationRefusal)?;
    let context = stage.source_named("language-window8-move-context", query, epoch, model_calls)
        .map_err(|_| StateDerivationRefusal)?;
    Ok(Window8StateDerivation { parent, walks, roots, context })
}
