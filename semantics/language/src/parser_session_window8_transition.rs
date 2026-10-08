//! Class-specific original Source legality/apply and score advancement. Raw
//! hypotheses are retained proposals, never independent commitment authority.
use crate::{
    parser_session_canonical_ingress::ParserCanonicalSourceExecutor,
    parser_session_numeric_custody::ParserNumericExecutor,
    parser_session_window8_atoms::{PreparedWindow8Atoms, Window8Atom as A},
    parser_session_window8_classes::Window8ClassDerivation,
    parser_session_window8_queries::PreparedWindow8Queries,
    parser_session_window8_stage::Window8RevisionStage,
    parser_session_window8_state::Window8StateDerivation,
    parser_session_window8_values::{boolean, field, path, view},
};
#[derive(Debug)]
pub(crate) struct TransitionRefusal;
#[allow(clippy::too_many_arguments)]
pub(crate) fn propose<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    derivation: &Window8StateDerivation,
    class: &Window8ClassDerivation,
    epoch: u64,
    model_calls: u64,
) -> Result<Option<usize>, TransitionRefusal> {
    let result = (|| {
        let seed = view(
            stage
                .book()
                .source
                .get(derivation.context)
                .ok_or(TransitionRefusal)?
                .output_bytes(),
        )
        .map_err(|_| TransitionRefusal)?;
        let class = class.value(stage.book()).map_err(|_| TransitionRefusal)?;
        let query = queries
            .record_fields(
                "language-window8-class-context",
                &[("seed", seed), ("class", class)],
            )
            .map_err(|_| TransitionRefusal)?;
        let mut history = stage
            .source_named("language-window8-class-context", query, epoch, model_calls)
            .map_err(|_| TransitionRefusal)?;
        for name in [
            "language-window8-move-legal-shift",
            "language-window8-move-legal-reduce",
            "language-window8-move-legal-left",
            "language-window8-move-legal-right-root",
            "language-window8-move-legal-right-nonroot",
            "language-window8-move-apply",
        ] {
            let query = queries
                .copy_record(
                    name,
                    view(stage.book().source[history].output_bytes())
                        .map_err(|_| TransitionRefusal)?,
                )
                .map_err(|_| TransitionRefusal)?;
            history = stage
                .source_named(name, query, epoch, model_calls)
                .map_err(|_| TransitionRefusal)?;
        }
        let output =
            view(stage.book().source[history].output_bytes()).map_err(|_| TransitionRefusal)?;
        Ok(
            if boolean(field(output, "accepted").map_err(|_| TransitionRefusal)?)
                .map_err(|_| TransitionRefusal)?
            {
                Some(history)
            } else {
                None
            },
        )
    })();
    if result.is_err() {
        stage.abort();
    }
    result
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn advance<S: ParserCanonicalSourceExecutor, N: ParserNumericExecutor>(
    stage: &mut Window8RevisionStage<'_, S, N>,
    queries: &mut PreparedWindow8Queries,
    atoms: &mut PreparedWindow8Atoms,
    derivation: &Window8StateDerivation,
    class: &Window8ClassDerivation,
    context: usize,
    frontier: usize,
    model: usize,
    outcome: usize,
    identity: &mut u64,
    epoch: u64,
    model_calls: u64,
) -> Result<usize, TransitionRefusal> {
    let result = (|| {
        let next = identity.checked_add(1).ok_or(TransitionRefusal)?;
        let prior = derivation
            .parent
            .hypothesis(stage.book())
            .map_err(|_| TransitionRefusal)?;
        let context = view(
            stage
                .book()
                .source
                .get(context)
                .ok_or(TransitionRefusal)?
                .output_bytes(),
        )
        .map_err(|_| TransitionRefusal)?;
        let raw = path(context, &["query", "raw"]).map_err(|_| TransitionRefusal)?;
        let frontier_history = stage.book().source.get(frontier).ok_or(TransitionRefusal)?;
        let frontier_query = view(frontier_history.input_bytes()).map_err(|_| TransitionRefusal)?;
        let equal = |a: crate::parser_session_window8_values::View<'_>,
                     b: crate::parser_session_window8_values::View<'_>| {
            a.type_bytes() == b.type_bytes() && a.value_node() == b.value_node()
        };
        if !equal(
            field(frontier_query, "features").map_err(|_| TransitionRefusal)?,
            raw,
        ) || !equal(
            field(frontier_query, "prior_choices").map_err(|_| TransitionRefusal)?,
            field(prior, "choices").map_err(|_| TransitionRefusal)?,
        ) || !equal(
            field(frontier_query, "selected").map_err(|_| TransitionRefusal)?,
            field(prior, "selected").map_err(|_| TransitionRefusal)?,
        ) || !equal(
            field(raw, "state").map_err(|_| TransitionRefusal)?,
            field(prior, "state").map_err(|_| TransitionRefusal)?,
        ) {
            return Err(TransitionRefusal);
        }
        let model = stage.book().model.get(model).ok_or(TransitionRefusal)?;
        if !equal(
            view(model.numeric.features.input_bytes()).map_err(|_| TransitionRefusal)?,
            context,
        ) {
            return Err(TransitionRefusal);
        }
        let scores = field(
            view(&model.numeric.output).map_err(|_| TransitionRefusal)?,
            "scores",
        )
        .map_err(|_| TransitionRefusal)?;
        let score = scores
            .collection_index(u16::from(class.code()))
            .map_err(|_| TransitionRefusal)?
            .ok_or(TransitionRefusal)?;
        let selected = field(
            view(frontier_history.output_bytes()).map_err(|_| TransitionRefusal)?,
            "count",
        )
        .map_err(|_| TransitionRefusal)?;
        // Indices locate retained parents only; complete fixed programs and
        // every original whole-frame edge are checked before score advancement.
        let chain = [
            outcome.checked_sub(6).ok_or(TransitionRefusal)?,
            outcome.checked_sub(5).ok_or(TransitionRefusal)?,
            outcome.checked_sub(4).ok_or(TransitionRefusal)?,
            outcome.checked_sub(3).ok_or(TransitionRefusal)?,
            outcome.checked_sub(2).ok_or(TransitionRefusal)?,
            outcome.checked_sub(1).ok_or(TransitionRefusal)?,
            outcome,
        ];
        let names = [
            "language-window8-class-context",
            "language-window8-move-legal-shift",
            "language-window8-move-legal-reduce",
            "language-window8-move-legal-left",
            "language-window8-move-legal-right-root",
            "language-window8-move-legal-right-nonroot",
            "language-window8-move-apply",
        ];
        for (index, name) in chain.into_iter().zip(names) {
            let history = stage.book().source.get(index).ok_or(TransitionRefusal)?;
            let port = crate::parser_session_window8_ports::PORTS
                .iter()
                .find(|port| port.name == name)
                .ok_or(TransitionRefusal)?;
            if !history.matches_fixed(
                port.original_programs,
                port.original_custody,
                port.input,
                port.output,
            ) {
                return Err(TransitionRefusal);
            }
        }
        for pair in chain.windows(2) {
            if stage.book().source[pair[0]].output_bytes()
                != stage.book().source[pair[1]].input_bytes()
            {
                return Err(TransitionRefusal);
            }
        }
        let class_query =
            view(stage.book().source[chain[0]].input_bytes()).map_err(|_| TransitionRefusal)?;
        if !equal(
            field(class_query, "class").map_err(|_| TransitionRefusal)?,
            class.value(stage.book()).map_err(|_| TransitionRefusal)?,
        ) || !equal(
            field(class_query, "seed").map_err(|_| TransitionRefusal)?,
            view(
                stage
                    .book()
                    .source
                    .get(derivation.context)
                    .ok_or(TransitionRefusal)?
                    .output_bytes(),
            )
            .map_err(|_| TransitionRefusal)?,
        ) {
            return Err(TransitionRefusal);
        }
        let outcome = view(
            stage
                .book()
                .source
                .get(outcome)
                .ok_or(TransitionRefusal)?
                .output_bytes(),
        )
        .map_err(|_| TransitionRefusal)?;
        if !boolean(field(outcome, "accepted").map_err(|_| TransitionRefusal)?)
            .map_err(|_| TransitionRefusal)?
        {
            return Err(TransitionRefusal);
        }
        atoms.unsigned(*identity).map_err(|_| TransitionRefusal)?;
        let query = queries
            .record_fields(
                "language-window8-score-advance",
                &[
                    (
                        "identity",
                        view(atoms.encoded(A::Unsigned).map_err(|_| TransitionRefusal)?)
                            .map_err(|_| TransitionRefusal)?,
                    ),
                    (
                        "prior_score",
                        field(prior, "score").map_err(|_| TransitionRefusal)?,
                    ),
                    ("step_score", score),
                    ("outcome", outcome),
                    (
                        "choices",
                        field(raw, "choices").map_err(|_| TransitionRefusal)?,
                    ),
                    ("selected", selected),
                ],
            )
            .map_err(|_| TransitionRefusal)?;
        let history = stage
            .source_named("language-window8-score-advance", query, epoch, model_calls)
            .map_err(|_| TransitionRefusal)?;
        *identity = next;
        Ok(history)
    })();
    if result.is_err() {
        stage.abort();
    }
    result
}
