#[path = "common/committed_prosody.rs"]
mod fixture;
use conduit_language::{
    dependency_commitment::{
        DEPENDENCY_CONTEXT_MAXIMUM_BYTES, DEPENDENCY_PAIR_MAXIMUM_BYTES,
        DEPENDENCY_QUERY_MAXIMUM_BYTES, DEPENDENCY_RESULT_MAXIMUM_BYTES,
    },
    prosody_commitment::*,
    *,
};
use conduit_plot::rust_binding::NativeRustBinding;
#[test]
fn supplied_graph_commitments_join_three_placements_and_execute_source_plan_play() {
    let execution = fixture::Prepared::new();
    for (text, dep, head) in [
        ("Travis hello friend", 0, 1),
        ("hello Travis friend", 1, 0),
        ("hello friend Travis", 2, 0),
    ] {
        let f = fixture::prosody::fixture(text, dep, head);
        let request = fixture::request(&f, 3, false, false);
        let result = fixture::result(&execution, &request);
        let profile = fixture::symbolic(&f.profile);
        let prepared = PreparedCommittedProsody::prepare(
            &f.lexical,
            dep as usize,
            f.discourse.fact(),
            &profile,
            &request,
            &result,
        )
        .unwrap();
        assert!(
            prepared
                .dependency()
                .admission()
                .query()
                .clone()
                .encode()
                .unwrap()
                .len()
                <= DEPENDENCY_QUERY_MAXIMUM_BYTES as usize
        );
        assert!(
            prepared
                .commitment()
                .result()
                .clone()
                .encode()
                .unwrap()
                .len()
                <= DEPENDENCY_RESULT_MAXIMUM_BYTES as usize
        );
        let installed = LanguageDependencyExecutionResult::decode(
            &execution.play(
                "language/commit-dependency",
                prepared
                    .dependency()
                    .admission()
                    .query()
                    .clone()
                    .encode()
                    .unwrap(),
            ),
        )
        .unwrap();
        assert_eq!(installed.actual(), prepared.commitment().result());
        assert_eq!(installed.context().arc(), prepared.dependency().arc());
        assert_eq!(
            installed.context(),
            &prepared.dependency().execution_context().unwrap()
        );
        assert_eq!(prepared.commitment().executions().len(), 8);
        assert_eq!(prepared.intent().choice(), f.profile.vocative());
        let played = LanguageProsodySymbolicIntent::decode(&execution.play(
            "language/committed-vocative-prosody",
            prepared.admission().clone().encode().unwrap(),
        ))
        .unwrap();
        assert_eq!(&played, prepared.intent());
    }
}
#[test]
fn punctuation_does_not_replace_structural_cause_and_uncommitted_vocative_refuses() {
    let execution = fixture::Prepared::new();
    // Four-token finite punctuation fixture: Hello , Travis .
    let f = fixture::prosody::fixture("Hello, Travis.", 2, 0);
    let profile = fixture::symbolic(&f.profile);
    let request = fixture::request(&f, 4, false, false);
    let result = fixture::result(&execution, &request);
    assert!(PreparedCommittedProsody::prepare(
        &f.lexical,
        2,
        f.discourse.fact(),
        &profile,
        &request,
        &result
    )
    .is_ok());
    // Supplied four-token reproduction of the retained learned demonstration's
    // precise safety obstacle: Travis' vocative is agreed, but the earlier
    // comma can attach to Hello or Travis. No pruning or forced commit occurs.
    let beam = request.beam();
    let previous = beam.candidate1().state();
    let mut heads = *previous.heads();
    heads[1] = 2;
    let competing = LanguageParserState::new(
        previous.basis().clone(),
        *previous.committed(),
        *previous.depth(),
        heads,
        previous.relation0().clone(),
        previous.relation1().clone(),
        previous.relation2().clone(),
        previous.relation3().clone(),
        *previous.stack(),
        *previous.token_count(),
        *previous.unread(),
    )
    .unwrap();
    let disagreeing_beam = LanguageParserAdmittedBeam::new(
        beam.basis().clone(),
        beam.candidate0().clone(),
        LanguageParserHypothesis::new(
            true,
            *beam.candidate1().identity(),
            *beam.candidate1().score(),
            competing,
        )
        .unwrap(),
        beam.candidate2().clone(),
        beam.candidate3().clone(),
    )
    .unwrap();
    let disagreeing = LanguageParserBeamCommitRequest::new(
        disagreeing_beam,
        false,
        request.frontier().clone(),
        false,
        4,
    )
    .unwrap();
    let refused = fixture::result(&execution, &disagreeing);
    assert!(!*refused.accepted());
    assert_eq!(refused.frontier(), disagreeing.frontier());
    assert_eq!(*refused.candidate0().state().committed(), 0);
    assert_eq!(*refused.candidate1().state().committed(), 0);
    assert!(PreparedCommittedProsody::prepare(
        &f.lexical,
        2,
        f.discourse.fact(),
        &profile,
        &disagreeing,
        &refused
    )
    .is_err());
    // Same source and supplied graph, but only Hello committed: the learned
    // comma-disagreement demonstration cannot cross this actual frontier.
    let prefix = fixture::request(&f, 1, false, false);
    let result = fixture::result(&execution, &prefix);
    assert!(PreparedCommittedProsody::prepare(
        &f.lexical,
        2,
        f.discourse.fact(),
        &profile,
        &prefix,
        &result
    )
    .is_err());
}
#[test]
fn refused_or_forged_commitment_foreign_occurrence_and_wrong_arc_do_not_publish() {
    let execution = fixture::Prepared::new();
    let f = fixture::prosody::fixture("Hello Travis", 1, 0);
    let profile = fixture::symbolic(&f.profile);
    let request = fixture::request(&f, 2, false, false);
    let accepted = fixture::result(&execution, &request);
    for (cancelled, pressure) in [(true, false), (false, true)] {
        let blocked = fixture::request(&f, 2, cancelled, pressure);
        assert!(PreparedCommittedProsody::prepare(
            &f.lexical,
            1,
            f.discourse.fact(),
            &profile,
            &blocked,
            &accepted
        )
        .is_err());
    }
    let foreign = fixture::prosody::fixture("Other Travis", 1, 0);
    assert!(PreparedCommittedProsody::prepare(
        &foreign.lexical,
        1,
        f.discourse.fact(),
        &profile,
        &request,
        &accepted
    )
    .is_err());
    let wrong = fixture::prosody::fixture("Hello Travis", 0, 1);
    assert!(PreparedCommittedProsody::prepare(
        &f.lexical,
        0,
        wrong.discourse.fact(),
        &profile,
        &request,
        &accepted
    )
    .is_err());
}
#[test]
fn checked_frontier_policy_executes_under_plan_play_without_advancing_on_pressure() {
    let execution = fixture::Prepared::new();
    for (cancelled, pressure, stable, through, expected) in [
        (false, false, 2, 2, true),
        (true, false, 2, 2, false),
        (false, true, 2, 2, false),
        (false, false, 1, 2, false),
    ] {
        let query =
            LanguageProsodyFrontierRequest::new(cancelled, 0, pressure, stable, through).unwrap();
        let result = LanguageProsodyFrontierResult::decode(
            &execution.play("language/prosody-frontier", query.encode().unwrap()),
        )
        .unwrap();
        assert_eq!(*result.accepted(), expected);
    }
}

#[test]
fn finite_public_dependency_pair_bound_fits_existing_zip_profile() {
    let pair = conduit_language::dependency_commitment::committed_dependency_execution_pair();
    eprintln!(
        "actual dependency execution pair envelope bytes: {}",
        pair.maximum_bytes()
    );
    assert!(pair.maximum_bytes() <= DEPENDENCY_PAIR_MAXIMUM_BYTES);
}

#[test]
fn punctuation_native_query_and_context_fit_declared_envelopes_without_execution() {
    // Layout/admission only: this constructs neither an accepted result nor an
    // execution/currentness receipt and does not prepare ProtocolSource.
    let f = fixture::prosody::fixture("Hello, Travis.", 2, 0);
    let request = fixture::request(&f, 4, false, false);
    let query = LanguageCommittedDependencyQuery::new(
        f.discourse.fact().basis().clone(),
        request.clone(),
        0,
        f.lexical.tape().source().clone(),
        f.lexical.tape().tokens().iter().nth(2).unwrap().clone(),
    )
    .unwrap();
    let context = LanguageDependencyExecutionContext::new(
        f.discourse.fact().basis().clone(),
        request.beam().basis().clone(),
        0,
        [
            *request.beam().candidate0().identity(),
            *request.beam().candidate1().identity(),
            *request.beam().candidate2().identity(),
            *request.beam().candidate3().identity(),
        ],
        4,
    )
    .unwrap();
    let query_bytes = query.encode().unwrap().len();
    let context_bytes = context.encode().unwrap().len();
    eprintln!("punctuation Native envelope bytes: query={query_bytes}/{DEPENDENCY_QUERY_MAXIMUM_BYTES} context={context_bytes}/{DEPENDENCY_CONTEXT_MAXIMUM_BYTES}; layout admission only");
    let query_schema = LanguageCommittedDependencyQuery::semantic_type()
        .unwrap()
        .canonical_bytes()
        .unwrap()
        .len();
    let context_schema = LanguageDependencyExecutionContext::semantic_type()
        .unwrap()
        .canonical_bytes()
        .unwrap()
        .len();
    let result_schema = LanguageParserBeamCommitResult::semantic_type()
        .unwrap()
        .canonical_bytes()
        .unwrap()
        .len();
    let execution_schema = LanguageDependencyExecutionResult::semantic_type()
        .unwrap()
        .canonical_bytes()
        .unwrap()
        .len();
    eprintln!("bare semantic Type canonical bytes: Query={query_schema} Context={context_schema} CommitResult={result_schema} PublicExecution={execution_schema}; no accepted result constructed");
    assert!(query_bytes <= DEPENDENCY_QUERY_MAXIMUM_BYTES as usize);
    assert!(context_bytes <= DEPENDENCY_CONTEXT_MAXIMUM_BYTES as usize);
}
