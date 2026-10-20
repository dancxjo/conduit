//! Exact25-index parity and explicit Native/Source negative scope.
use super::{causal_prefix_inputs::*, causal_prefix_runtime::*, parser_selection_values::*};
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
use serde_json::{json, Value};
pub fn exact(actual: &[u64; 25], expected: &[u64; 25]) -> Result<(), usize> {
    actual
        .iter()
        .zip(expected)
        .position(|(a, b)| a != b)
        .map_or(Ok(()), Err)
}
pub fn query(
    graphs: &mut Graphs,
    store: &mut Store,
    arm: &Arrival,
    choices: [u64; 4],
    expected: [u64; 25],
    invocation: &mut u64,
) -> Value {
    *invocation += 1;
    let lexical = available(&arm.lexical, *arm.state.token_count() as usize);
    let query = LanguageParserScorerChoiceQuery::new(
        choices,
        *lexical.tape().source().sequence(),
        *invocation,
        lexical,
        arm.state.clone(),
    )
    .unwrap();
    let (result, evidence): (LanguageParserScorerFeatures, _) =
        graphs.invoke(store, "language-parser-scorer-model-features", query);
    assert_eq!(result.basis(), arm.state.basis());
    assert_eq!(*result.epoch(), *arm.lexical.tape().source().sequence());
    assert_eq!(*result.invocation(), *invocation);
    if let Err(index) = exact(result.indices(), &expected) {
        store.write("unexpected-source-feature-failure.json", &json!({"status":"UNEXPECTED_SOURCE_PARITY_FAILURE_NOT_FIT_ELIGIBLE","fit_eligible":false,"index":index,"actual":result.indices(),"expected":expected,"evidence":evidence,"policy":"fatal generator/fixture defect; never quarantine as representation mismatch"}));
        panic!("actual Source feature mismatch at {index}");
    }
    let mut forged = *result.indices();
    forged[8] ^= 1;
    assert_eq!(exact(result.indices(), &forged), Err(8));
    json!({"choice_vector":choices,"indices":result.indices(),"evidence":evidence,"actual_source_output_membership_mutation_refused_at":8})
}
pub fn arm(
    graphs: &mut Graphs,
    store: &mut Store,
    state: &Arrival,
    request: &Value,
    invocation: &mut u64,
) -> Value {
    assert_eq!(u(&request["available_count"]), *state.state.token_count());
    let choices = request["choice_vectors"].as_array().unwrap();
    let expected = request["expected25"].as_array().unwrap();
    assert_eq!(choices.len(), expected.len());
    assert!(!choices.is_empty() && choices.len() <= 256);
    let alternatives: Vec<_> = choices
        .iter()
        .zip(expected)
        .map(|(c, f)| query(graphs, store, state, array(c), array(f), invocation))
        .collect();
    let gold = query(
        graphs,
        store,
        state,
        array(&request["gold_choice_vector"]),
        array(&request["gold_expected25"]),
        invocation,
    );
    json!({"alternatives":alternatives,"gold":gold})
}
pub fn negative_queries(causal: &Arrival, full: &Arrival, p: usize, n: usize) -> Value {
    let c = available(&causal.lexical, p);
    let f = available(&full.lexical, n);
    let mut counts = serde_json::Map::new();
    let out = c.tape().tokens().iter().next().unwrap().candidates().len() as u64;
    let mut choice = [0; 4];
    choice[0] = out;
    assert!(LanguageParserScorerChoiceQuery::new(
        choice,
        *c.tape().source().sequence(),
        1,
        c.clone(),
        causal.state.clone()
    )
    .is_err());
    counts.insert("out_of_support_choice".into(), json!(1));
    if p < n {
        assert!(LanguageParserScorerChoiceQuery::new(
            [0; 4],
            *c.tape().source().sequence(),
            1,
            c.clone(),
            full.state.clone()
        )
        .is_err());
        counts.insert("full_state_with_causal_lexical".into(), json!(1));
        let mut heads = *causal.state.heads();
        heads[0] = p as u64;
        assert!(LanguageParserState::new(
            causal.state.basis().clone(),
            *causal.state.committed(),
            *causal.state.depth(),
            heads,
            causal.state.relation0().clone(),
            causal.state.relation1().clone(),
            causal.state.relation2().clone(),
            causal.state.relation3().clone(),
            *causal.state.stack(),
            *causal.state.token_count(),
            *causal.state.unread()
        )
        .is_err());
        counts.insert("future_head_native".into(), json!(1));
    } else {
        assert!(LanguageParserScorerChoiceQuery::new(
            [0; 4],
            *f.tape().source().sequence(),
            1,
            f,
            causal.state.clone()
        )
        .is_err());
        counts.insert("foreign_source_revision_occurrences".into(), json!(1));
    }
    // Actual output membership mutation is tested against each observed Source
    // feature result in query(); it is not Native shape/invariant authority.
    Value::Object(counts)
}
pub fn no_available_occurrence(profile: &LanguageLexicalProfile, word: &str) -> Value {
    let source = revision(
        word,
        "causal/incomplete-negative",
        0,
        None,
        None,
        LanguageTextFinality::Partial,
    );
    let lexical = conduit_language::lexical::prepare_lexical_tape(&source, profile, None).unwrap();
    assert!(LanguageParserAvailableLexical::new(lexical.tape().clone(), 1).is_err());
    let empty = LanguageParserAvailableLexical::new(lexical.tape().clone(), 0).unwrap();
    json!({"reason":"no_available_occurrence","source":digest(&source.clone().encode().unwrap()),"available":empty.token_count(),"emitted_action_samples":0})
}
