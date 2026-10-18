//! Reuses accepted Language Source and its existing kernel harness. No second
//! parser or commitment counter is introduced by the Speech proof.
#[path = "../../../../language/tests/common/parser_kernel.rs"]
mod kernel;
#[path = "../../../../language/tests/common/parser_fixture.rs"]
pub(super) mod parser;
use conduit_core::ValuePayload;
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub fn source() -> String {
    [
        include_str!("../../../../language/types.conduit"),
        include_str!("../../../../language/identity.conduit"),
        include_str!("../../../../language/coverage.conduit"),
        include_str!("../../../../language/syntax.conduit"),
        include_str!("../../../../language/text_revision.conduit"),
        include_str!("../../../../language/revision_lineage.conduit"),
        include_str!("../../../../language/lexical.conduit"),
        include_str!("../../../../language/parser.conduit"),
        include_str!("../../../../language/parser_beam.conduit"),
        PARSER_BEAM_COMMIT_SOURCE,
    ]
    .join("\n")
}

pub fn request() -> LanguageParserBeamCommitRequest {
    let mut parser = parser::Fixture::new();
    let mut state = parser.initial(3);
    // A deterministic supplied analysis: would -> you -> weigh, with one root.
    // These are actual legal Source transitions, not a forged committed state.
    for relation in ["root", "dep", "dep"] {
        let next = parser.step(&state, "right_arc", relation, "analysis/1");
        assert!(parser::accepted(&next));
        state = parser::field(&next, "state").clone();
    }
    let raw = LanguageParserNumericState::from_structured(state).unwrap();
    let state = LanguageParserState::new(
        raw.basis().clone(),
        *raw.committed(),
        *raw.depth(),
        *raw.heads(),
        raw.relation0().clone(),
        raw.relation1().clone(),
        raw.relation2().clone(),
        raw.relation3().clone(),
        *raw.stack(),
        *raw.token_count(),
        *raw.unread(),
    )
    .unwrap();
    let basis = state.basis().clone();
    let hypothesis = |active, identity| {
        LanguageParserHypothesis::new(active, identity, 0, state.clone()).unwrap()
    };
    LanguageParserBeamCommitRequest::new(
        LanguageParserAdmittedBeam::new(
            basis.clone(),
            hypothesis(true, 1),
            hypothesis(false, 2),
            hypothesis(false, 3),
            hypothesis(false, 4),
        )
        .unwrap(),
        false,
        LanguageParserRawFrontier::new(basis, 0, 3).unwrap(),
        false,
        3,
    )
    .unwrap()
}

pub fn execute(request: &LanguageParserBeamCommitRequest) -> LanguageParserBeamCommitResult {
    use conduit_kernel::scheduler::RemoteIngressOutcome;
    let mut execution = kernel::Execution::prepare(source(), "language-parser-beam-commit");
    let input = execution.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output = execution.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    let packet = ValuePayload {
        value_kind: input.value_kind,
        encoded: request.clone().encode().unwrap(),
    };
    let mut result = ValuePayload {
        value_kind: output.value_kind,
        encoded: Vec::with_capacity(65536),
    };
    execution.kernel.start().unwrap();
    assert!(matches!(
        execution
            .kernel
            .admit_input(&input.port_id, 0, &packet)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    ));
    assert!((0..4000).any(|_| {
        execution.step();
        execution
            .kernel
            .output_into(&output.port_id, &mut result)
            .unwrap()
            == Some(0)
    }));
    execution
        .kernel
        .complete_output(&output.port_id, 0)
        .unwrap();
    execution.kernel.close_input(&input.port_id).unwrap();
    assert!(
        (0..4000).any(|_| execution.step() == conduit_composite::KernelCompositeStatus::Complete)
    );
    LanguageParserBeamCommitResult::decode(&result.encoded).unwrap()
}
