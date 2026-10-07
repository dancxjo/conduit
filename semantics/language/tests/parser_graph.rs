use conduit_core::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;
#[path = "common/parser_fixture.rs"]
mod fixture;
#[path = "common/parser_kernel.rs"]
mod parser_kernel;
use fixture::*;

#[test]
fn source_transition_graph_runs_under_ordinary_plan_and_play() {
    let mut f = Fixture::new();
    let source = format!(
        "{}\n{}\n{}",
        include_str!("../identity.conduit"),
        include_str!("../types.conduit"),
        include_str!("../parser.conduit")
    );
    let mut run = parser_kernel::Execution::prepare(source, "language-parser-transition");
    let boundary = &run.kernel.definition().boundary;
    let input_port = boundary.input_fronts[0].external_port.clone();
    let output_port = boundary.output_fronts[0].external_port.clone();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
    };
    run.kernel.start().unwrap();
    let mut state = f.initial(2);
    for (sequence, (action, relation, revision, expected)) in [
        ("right_arc", "root", "analysis/1", true),
        ("right_arc", "vocative", "analysis/1", true),
        ("reduce", "dep", "analysis/stale", false),
        ("reduce", "dep", "analysis/1", true),
    ]
    .into_iter()
    .enumerate()
    {
        let ty = f.ty("LanguageParserRequest");
        let request = record(
            ty,
            vec![
                ("basis", f.basis(revision)),
                ("state", state.clone()),
                ("action", f.action(action)),
                ("relation", f.relation(relation)),
            ],
        );
        let input = ValuePayload {
            value_kind: input_port.value_kind.clone(),
            encoded: request.canonical_bytes().unwrap(),
        };
        let admission = run
            .kernel
            .admit_input(&input_port.port_id, sequence as u64, &input)
            .unwrap();
        assert!(
            matches!(admission, RemoteIngressOutcome::Accepted { .. }),
            "sequence {sequence}: {admission:?}"
        );
        let result = (0..4000)
            .find_map(|_| {
                run.step();
                run.kernel
                    .output_into(&output_port.port_id, &mut output)
                    .unwrap()
                    .map(|received| {
                        assert_eq!(received, sequence as u64);
                        StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap()
                    })
            })
            .expect("bounded source graph output");
        run.kernel
            .complete_output(&output_port.port_id, sequence as u64)
            .unwrap();
        assert_eq!(accepted(&result), expected);
        if expected {
            state = field(&result, "state").clone();
            assert!(f.native_ok(&state));
        } else {
            assert_eq!(field(&result, "state"), &state);
        }
    }
    run.kernel.close_input(&input_port.port_id).unwrap();
}
