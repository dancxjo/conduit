//! Original finite commitment Source extraction, before Session/runtime wiring.
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
#[path = "common/commit_fixture.rs"]
mod commit_fixture;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;

fn original_commit_source() -> String {
    [
        include_str!("../types.conduit"),
        include_str!("../identity.conduit"),
        include_str!("../coverage.conduit"),
        include_str!("../syntax.conduit"),
        include_str!("../text_revision.conduit"),
        include_str!("../revision_lineage.conduit"),
        include_str!("../lexical.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_beam.conduit"),
        include_str!("../parser_available.conduit"),
        include_str!("../discourse.conduit"),
        include_str!("../prosody.conduit"),
        include_str!("common/commit_source/parser_joint.conduit"),
        include_str!("common/commit_source/parser_joint_decode.conduit"),
        include_str!("common/commit_source/parser_session.conduit"),
        include_str!("common/commit_source/parser_session_commit.conduit"),
        include_str!("common/commit_source/parser_session_committed_dependency.conduit"),
        include_str!("common/commit_source/parser_session_dependency.conduit"),
        include_str!("common/commit_source/parser_session_facts.conduit"),
    ]
    .join("\n")
}

#[test]
fn original_commit_source_checks_and_expands_on_current_language_contracts() {
    let source = original_commit_source();
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &checked,
        "language-parser-joint-commit",
        &ProfileCatalog::new(),
    )
    .unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    assert_eq!(expanded.expanded.gears[0].configuration.len(), 1);
}

#[test]
fn original_commit_flow_advances_only_active_frontiers_under_the_existing_kernel() {
    use conduit_core::{StructuredInfoValue, ValuePayload, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
    use conduit_kernel::scheduler::RemoteIngressOutcome;
    use conduit_plot::rust_binding::NativeRustBinding;
    use fixture::{count, field};
    let source = original_commit_source();
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let query = commit_fixture::query(&checked);
    let original_bytes = query.canonical_bytes().unwrap();
    let original_fact = field(&query, "fact");
    let original_consensus = field(original_fact, "query");
    let original_beam = field(original_consensus, "beam");
    let candidate = field(original_beam, "candidate1");
    let parser = field(candidate, "parser");
    let state = field(parser, "state");
    let advanced = fixture::replace(
        state,
        "committed",
        fixture::number(fixture::field_type(state.value_type(), "committed"), 1),
    );
    conduit_language::LanguageParserState::from_structured(advanced.clone()).unwrap();
    let mismatch = fixture::replace(
        original_beam,
        "candidate1",
        fixture::replace(
            candidate,
            "parser",
            fixture::replace(parser, "state", advanced),
        ),
    );
    let mismatch = fixture::replace(
        &query,
        "fact",
        fixture::replace(
            original_fact,
            "query",
            fixture::replace(original_consensus, "beam", mismatch),
        ),
    );
    let commit_owner = checked
        .native_types
        .iter()
        .find(|owner| owner.name == "LanguageParserJointCommitQuery")
        .unwrap();
    assert!(conduit_plot::rust_binding::validate_native_invariants(
        &mismatch,
        &commit_owner.invariants
    )
    .is_err());
    let basis = field(original_beam, "basis");
    let foreign_basis = fixture::replace(
        basis,
        "analysis_revision",
        fixture::text(
            fixture::field_type(basis.value_type(), "analysis_revision"),
            "analysis/foreign",
        ),
    );
    let foreign = fixture::replace(original_beam, "basis", foreign_basis);
    let beam_owner = checked
        .native_types
        .iter()
        .find(|owner| owner.name == "LanguageParserJointBeam")
        .unwrap();
    assert!(conduit_plot::rust_binding::validate_native_invariants(
        &foreign,
        &beam_owner.invariants
    )
    .is_err());

    let mut run = parser_kernel::Execution::prepare(source, "language-parser-joint-commit");
    let input_port = run.kernel.definition().boundary.input_fronts[0]
        .external_port
        .clone();
    let output_port = run.kernel.definition().boundary.output_fronts[0]
        .external_port
        .clone();
    run.kernel.start().unwrap();
    let input = ValuePayload {
        value_kind: input_port.value_kind,
        encoded: original_bytes.clone(),
    };
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 0, &input)
            .unwrap(),
        RemoteIngressOutcome::Accepted { .. }
    ));
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 1, &input)
            .unwrap(),
        RemoteIngressOutcome::Full { sequence: 1 }
    ));
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
    };
    assert!(run
        .kernel
        .output_into(&output_port.port_id, &mut output)
        .unwrap()
        .is_none());
    let received = (0..4000)
        .find_map(|_| {
            run.step();
            run.kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
        })
        .expect("bounded actual commitment output");
    assert_eq!(received, 0);
    let proposal = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    let beam = field(field(field(&query, "fact"), "query"), "beam");
    for (name, active) in [
        ("candidate0", true),
        ("candidate1", true),
        ("candidate2", false),
        ("candidate3", false),
    ] {
        let before = field(beam, name);
        let after = field(&proposal, name);
        assert_eq!(field(before, "choices"), field(after, "choices"));
        let before = field(before, "parser");
        let after = field(after, "parser");
        for retained in ["identity", "score", "active"] {
            assert_eq!(field(before, retained), field(after, retained));
        }
        let before = field(before, "state");
        let after = field(after, "state");
        assert_eq!(
            count(field(after, "committed")),
            count(field(before, "committed")) + u64::from(active)
        );
        for retained in [
            "basis",
            "token_count",
            "unread",
            "depth",
            "stack",
            "heads",
            "relation0",
            "relation1",
            "relation2",
            "relation3",
        ] {
            assert_eq!(field(before, retained), field(after, retained));
        }
    }
    assert_eq!(query.canonical_bytes().unwrap(), original_bytes);
    run.kernel.complete_output(&output_port.port_id, 0).unwrap();
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 1, &input)
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 1 }
    ));
    run.kernel.cancel().unwrap();
    assert_eq!(
        run.kernel.step().unwrap(),
        conduit_composite::KernelCompositeStatus::Cancelled
    );
    assert!(run.kernel.next_host_request().is_none());
    assert!(run
        .kernel
        .output_into(&output_port.port_id, &mut output)
        .is_err());
    assert!(run
        .kernel
        .admit_input(&input_port.port_id, 2, &input)
        .is_err());
    assert_eq!(query.canonical_bytes().unwrap(), original_bytes);
}
