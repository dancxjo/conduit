//! Actual partial Source and lexical owners, with the retained Source rebase law.
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
use fixture::{dep, profile, revision, source, state};
#[test]
fn mechanical_admission_preserves_committed_source_and_refuses_forged_revisions() {
    let old = revision("answer record ", None, 7);
    let tape = prepare_lexical_tape(&old, &profile(), None).unwrap();
    let state = state(1);
    let analysis = || LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap();
    let changed_suffix = revision("answer answer ", Some(&old), 7);
    let admitted =
        PreparedParserRevisionRebase::prepare(&state, &tape, &changed_suffix, analysis(), dep())
            .unwrap();
    assert_eq!(admitted.request().previous(), &state);
    assert_eq!(admitted.request().lineage().previous(), &old);
    assert_eq!(*admitted.request().next().token_count(), 2);
    assert_eq!(admitted.lexical().tape().source(), &changed_suffix);
    for candidate in [
        revision("record answer ", Some(&old), 7),
        revision("answer answer ", None, 7),
    ] {
        assert!(PreparedParserRevisionRebase::prepare(
            &state,
            &tape,
            &candidate,
            analysis(),
            dep()
        )
        .is_err());
    }
    assert!(PreparedParserRevisionRebase::prepare(
        &state,
        &tape,
        &changed_suffix,
        LanguageAnalysisRevisionId::new("analysis/1".into()).unwrap(),
        dep()
    )
    .is_err());
    assert_eq!(tape.tape().source(), &old);
}

#[test]
fn checked_rebase_plays_and_preserves_only_the_committed_graph() {
    use conduit_core::{StructuredInfoValue, ValuePayload};
    use conduit_kernel::scheduler::RemoteIngressOutcome;
    let old = revision("answer record ", None, 7);
    let tape = prepare_lexical_tape(&old, &profile(), None).unwrap();
    let previous = state(1);
    let revised = revision("answer answer ", Some(&old), 7);
    let admitted = PreparedParserRevisionRebase::prepare(
        &previous,
        &tape,
        &revised,
        LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap(),
        dep(),
    )
    .unwrap();
    let original = admitted.request().clone().encode().unwrap();
    let mut run = parser_kernel::Execution::prepare(source(), "language-parser-revision-rebase");
    let input_port = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output_port = run.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    run.kernel.start().unwrap();
    let input = ValuePayload {
        value_kind: input_port.value_kind,
        encoded: original.clone(),
    };
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 0, &input)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    ));
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 1, &input)
            .unwrap(),
        RemoteIngressOutcome::Full { sequence: 1 }
    ));
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(PARSER_REVISION_MAXIMUM_INPUT_BYTES),
    };
    let received = (0..4000)
        .find_map(|_| {
            run.step();
            run.kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
        })
        .expect("finite actual rebase output");
    assert_eq!(received, 0);
    let value = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    let rebased = LanguageParserNumericState::from_structured(value).unwrap();
    assert_eq!(rebased.basis(), admitted.request().basis());
    assert_eq!(*rebased.committed(), 1);
    assert_eq!(*rebased.unread(), 1);
    assert_eq!(rebased.heads()[0], previous.heads()[0]);
    assert_eq!(rebased.relation0(), previous.relation0());
    assert_eq!(rebased.heads()[1], 5);
    assert_eq!(
        *rebased.relation1().base(),
        LanguageUniversalDependencyRelation::Dep
    );
    LanguageParserState::new(
        rebased.basis().clone(),
        *rebased.committed(),
        *rebased.depth(),
        *rebased.heads(),
        rebased.relation0().clone(),
        rebased.relation1().clone(),
        rebased.relation2().clone(),
        rebased.relation3().clone(),
        *rebased.stack(),
        *rebased.token_count(),
        *rebased.unread(),
    )
    .unwrap();
    run.kernel
        .complete_output(&output_port.port_id, received)
        .unwrap();
    run.kernel.close_input(&input_port.port_id).unwrap();
    assert!((0..4000).any(|_| run.step() == conduit_composite::KernelCompositeStatus::Complete));
    assert_eq!(
        run.kernel
            .output_terminal_into(&output_port.port_id, &mut output)
            .unwrap(),
        Some(conduit_composite::KernelCompositeTerminal::Normal)
    );
    assert_eq!(admitted.request().clone().encode().unwrap(), original);
    assert_eq!(tape.tape().source(), &old);
}
