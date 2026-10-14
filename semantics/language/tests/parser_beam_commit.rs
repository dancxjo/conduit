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
fn run(request: LanguageParserBeamCommitRequest) -> LanguageParserBeamCommitResult {
    use conduit_core::ValuePayload;
    use conduit_kernel::scheduler::RemoteIngressOutcome;
    let bytes = request.encode().unwrap();
    let mut run =
        parser_kernel::Execution::prepare(fixture::source(), "language-parser-beam-commit");
    let input_port = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output_port = run.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    run.kernel.start().unwrap();
    let input = ValuePayload {
        value_kind: input_port.value_kind,
        encoded: bytes,
    };
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 0, &input)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    ));
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(65536),
    };
    let received = (0..4000)
        .find_map(|_| {
            run.step();
            run.kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
        })
        .expect("bounded commitment result");
    assert_eq!(received, 0);
    let result = LanguageParserBeamCommitResult::decode(&output.encoded).unwrap();
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
    result
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
    for (through, stable, cancelled, pressure, refusal) in [
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
    ] {
        let result = run(query(through, stable, cancelled, pressure));
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
fn commitment_stage_expansion_identifies_the_exact_admission_boundary() {
    let source = fixture::source();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    for name in [
        "language-parser-beam-commit-proposal",
        "language-parser-beam-commit-start",
        "language-parser-beam-commit-check-0",
        "language-parser-beam-commit-check-1",
        "language-parser-beam-commit-check-2",
        "language-parser-beam-commit-check-3",
        "language-parser-beam-commit-decision",
        "language-parser-beam-commit-apply",
    ] {
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            name,
            &conduit_plot::ProfileCatalog::new(),
        );
        assert!(expanded.is_ok(), "{name}: {expanded:?}");
    }
}
