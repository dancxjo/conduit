//! Checked commitment updates the actual hypothesis states, not only a counter.
#[path = "common/parser_revision_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn query(
    through: u64,
    stable: u64,
    cancelled: bool,
    pressure: bool,
) -> LanguageParserBeamCommitRequest {
    let first = fixture::state_with_relation(0, "obj");
    let second = fixture::state_with_relation(0, "nmod");
    let basis = first.basis().clone();
    let candidate =
        |active, id, score, state| LanguageParserHypothesis::new(active, id, score, state).unwrap();
    let beam = LanguageParserAdmittedBeam::new(
        basis.clone(),
        candidate(true, 1, 300, first.clone()),
        candidate(true, 2, 800, second),
        candidate(false, 3, 0, first.clone()),
        candidate(false, 4, 0, first),
    )
    .unwrap();
    LanguageParserBeamCommitRequest::new(
        beam,
        cancelled,
        LanguageParserRawFrontier::new(basis, 0, stable).unwrap(),
        pressure,
        through,
    )
    .unwrap()
}
fn play(entry: &str, packets: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    use conduit_core::ValuePayload;
    use conduit_kernel::scheduler::RemoteIngressOutcome;
    let mut run = parser_kernel::Execution::prepare(fixture::source(), entry);
    let input_port = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output_port = run.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    let mut results = Vec::with_capacity(packets.len());
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(65536),
    };
    run.kernel.start().unwrap();
    for (sequence, encoded) in packets.into_iter().enumerate() {
        let sequence = sequence as u64;
        let input = ValuePayload {
            value_kind: input_port.value_kind.clone(),
            encoded,
        };
        assert!(
            matches!(run.kernel.admit_input(&input_port.port_id, sequence, &input).unwrap(), RemoteIngressOutcome::Accepted { sequence: actual } if actual == sequence)
        );
        let received = (0..4000)
            .find_map(|_| {
                run.step();
                run.kernel
                    .output_into(&output_port.port_id, &mut output)
                    .unwrap()
            })
            .expect("bounded result from the actual checked Source");
        assert_eq!(received, sequence);
        results.push(output.encoded.clone());
        run.kernel
            .complete_output(&output_port.port_id, received)
            .unwrap();
    }
    run.kernel.close_input(&input_port.port_id).unwrap();
    assert!((0..4000).any(|_| run.step() == conduit_composite::KernelCompositeStatus::Complete));
    assert_eq!(
        run.kernel
            .output_terminal_into(&output_port.port_id, &mut output)
            .unwrap(),
        Some(conduit_composite::KernelCompositeTerminal::Normal)
    );
    results
}
fn run(request: LanguageParserBeamCommitRequest) -> LanguageParserBeamCommitResult {
    LanguageParserBeamCommitResult::decode(
        &play(
            "language-parser-beam-commit",
            vec![request.encode().unwrap()],
        )[0],
    )
    .unwrap()
}
#[test]
fn actual_source_commits_the_agreed_prefix_and_preserves_competing_suffixes() {
    let request = query(1, 1, false, false);
    let prior = request.clone();
    let result = run(request);
    assert!(*result.accepted());
    assert_eq!(*result.frontier().committed(), 1);
    for (before, after) in [
        (prior.beam().candidate0(), result.candidate0()),
        (prior.beam().candidate1(), result.candidate1()),
    ] {
        assert_eq!(*after.state().committed(), 1);
        assert_eq!(after.state().heads(), before.state().heads());
        assert_eq!(after.state().relation1(), before.state().relation1());
        assert_eq!(after.identity(), before.identity());
        assert_eq!(after.score(), before.score());
    }
    assert_ne!(
        result.candidate0().state().relation1(),
        result.candidate1().state().relation1()
    );
    assert_eq!(*result.candidate2().state().committed(), 0);
}
#[test]
fn disagreement_pressure_and_cancellation_leave_every_hypothesis_uncommitted() {
    let cases = [
        (2, 2, false, false, LanguageParserFrontierRefusal::NotAgreed),
        (1, 1, true, false, LanguageParserFrontierRefusal::Cancelled),
        (1, 1, false, true, LanguageParserFrontierRefusal::Pressure),
        (
            1,
            0,
            false,
            false,
            LanguageParserFrontierRefusal::BeyondStable,
        ),
    ];
    let template = query(1, 1, false, false);
    let packets = cases
        .iter()
        .map(|(through, stable, cancelled, pressure, _)| {
            LanguageParserBeamCommitRequest::new(
                template.beam().clone(),
                *cancelled,
                LanguageParserRawFrontier::new(template.frontier().basis().clone(), 0, *stable)
                    .unwrap(),
                *pressure,
                *through,
            )
            .unwrap()
            .encode()
            .unwrap()
        })
        .collect();
    for (bytes, (_, _, _, _, refusal)) in play("language-parser-beam-commit", packets)
        .into_iter()
        .zip(cases)
    {
        let result = LanguageParserBeamCommitResult::decode(&bytes).unwrap();
        assert!(!*result.accepted());
        assert_eq!(result.refusal(), &refusal);
        assert_eq!(*result.frontier().committed(), 0);
        for candidate in [
            result.candidate0(),
            result.candidate1(),
            result.candidate2(),
            result.candidate3(),
        ] {
            assert_eq!(*candidate.state().committed(), 0);
        }
    }
}

#[test]
fn actual_commit_then_text_revision_rebases_and_revises_only_uncommitted_edges() {
    use conduit_language::lexical::prepare_lexical_tape;
    let old = fixture::revision("answer record ", None, 7);
    let lexical = prepare_lexical_tape(&old, &fixture::profile(), None).unwrap();
    let committed = run(query(1, 1, false, false));
    let previous = fixture::native_state(committed.candidate0().state());
    assert_eq!(*previous.committed(), 1);
    let next = fixture::revision("answer answer ", Some(&old), 7);
    let analysis = || LanguageAnalysisRevisionId::new("analysis/2".into()).unwrap();
    let prepared = PreparedParserRevisionRebase::prepare(
        &previous,
        &lexical,
        &next,
        analysis(),
        fixture::dep(),
    )
    .unwrap();
    let output = play(
        "language-parser-revision-rebase",
        vec![prepared.request().clone().encode().unwrap()],
    );
    let rebased = LanguageParserNumericState::decode(&output[0]).unwrap();
    fixture::native_state(&rebased);
    assert_eq!(rebased.heads()[0], previous.heads()[0]);
    assert_eq!(rebased.relation0(), previous.relation0());
    assert_eq!(rebased.heads()[1], 5);
    let relation = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Nmod,
        LanguageParserSubtype::new(String::new()).unwrap(),
    )
    .unwrap();
    let request = LanguageParserRequest::new(
        LanguageParserAction::RightArc,
        rebased.basis().clone(),
        relation,
        rebased,
    )
    .unwrap();
    let output = play(
        "language-parser-transition",
        vec![request.encode().unwrap()],
    );
    let revised = LanguageParserResult::decode(&output[0]).unwrap();
    assert!(*revised.accepted());
    let final_state = fixture::native_state(revised.state());
    assert_eq!(*final_state.committed(), 1);
    assert_eq!(final_state.heads()[0], previous.heads()[0]);
    assert_eq!(final_state.relation0(), previous.relation0());
    assert_eq!(
        *final_state.relation1().base(),
        LanguageUniversalDependencyRelation::Nmod
    );
    assert_ne!(final_state.relation1(), previous.relation1());
    assert_eq!(
        final_state.basis().source_revision(),
        next.material().revision()
    );
    let forged = fixture::revision("record answer ", Some(&old), 7);
    assert!(PreparedParserRevisionRebase::prepare(
        &previous,
        &lexical,
        &forged,
        analysis(),
        fixture::dep()
    )
    .is_err());
    assert_eq!(lexical.tape().source(), &old);
}
