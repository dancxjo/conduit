//! Exact scalar tape eligibility and installed execution.
use crate::host_execution::scalar_tape_fixture::{prepare, NoTimer, Outputs};
use crate::ExternalForeInput;
use conduit_core::ConfigurationValue;
use conduit_core::ConnectionTrack;
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};

#[test]
fn actual_std_plan_and_kernel_play_select_the_scalar_tape_back() {
    let source =
        include_str!("../../../../../architecture/plot/tests/fixtures/expression_baseline.conduit");
    for operations in [1u64, 3, 8, 32] {
        let (mut host, plan, program) = prepare(source, &format!("baseline-{operations}"), true);
        assert_eq!(
            plan.source_document_id,
            parse_syntax_document(source).source_document_id()
        );
        let placement = &plan.fragments[0].placements[0];
        assert_eq!(
            placement.implementation_id.as_str(),
            conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
        );
        assert_eq!(
            placement.artifact_id.as_str(),
            conduit_std_offers::PURE_SCALAR_TAPE_STD_ARTIFACT
        );
        assert!(conduit_core::verify_plan(&plan));
        let property = &placement.realization_properties[0];
        let bounds =
            conduit_core::StructuredInfoValue::from_canonical_bytes(property.canonical_value())
                .unwrap();
        let conduit_core::StructuredInfoValueShape::Record(fields) = bounds.shape() else {
            panic!("typed tape bounds")
        };
        let evaluator =
            conduit_plot::PreparedPortableExpressionEvaluator::new_scalar_tape(&program).unwrap();
        let (instructions, scratch) = evaluator.scalar_tape_limits().unwrap();
        for (name, expected) in [
            ("maximum-instructions", instructions),
            ("scratch-payload-bytes", scratch),
            (
                "retained-tape-payload-bytes",
                evaluator.scalar_tape_storage_bytes().unwrap(),
            ),
            ("output-capacity-bytes", evaluator.output_capacity()),
        ] {
            let field = fields.iter().find(|field| field.name() == name).unwrap();
            let conduit_core::StructuredInfoValueShape::Leaf(bytes) = field.value().shape() else {
                panic!("integer bound")
            };
            assert_eq!(bytes, (expected as u64).to_le_bytes());
        }
        let input = 7u64.to_le_bytes();
        let expected = program.evaluate(&input).unwrap();
        let mut output = Outputs(Vec::with_capacity(1));
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input.to_vec(),
                }],
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(output.0, [expected]);
        let kernel = report.kernel.as_ref().expect("actual kernel Play");
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|e| e.kind == conduit_kernel::KernelEventKind::HostCallRequested));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|e| e.kind == conduit_kernel::KernelEventKind::HostCallCompleted));

        println!(
            "operations={operations} plan={} report={report:?}",
            plan.plan_id.as_str()
        );
    }
}

#[test]
fn omitted_or_foreign_tape_bounds_refuse_before_output() {
    let source =
        include_str!("../../../../../architecture/plot/tests/fixtures/expression_baseline.conduit");
    let (_, foreign, _) = prepare(source, "baseline-32", true);
    for omit in [false, true] {
        let (mut host, mut plan, _) = prepare(source, "baseline-1", true);
        plan.fragments[0].placements[0].realization_properties = if omit {
            vec![]
        } else {
            foreign.fragments[0].placements[0]
                .realization_properties
                .clone()
        };
        assert!(!conduit_core::verify_plan(&plan));
        let mut outputs = Outputs(Vec::new());
        assert!(host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: 7u64.to_le_bytes().to_vec()
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .is_err());
        assert!(outputs.0.is_empty());
    }
}

#[test]
fn tape_offer_refuses_unsupported_temporal_and_semantic_profiles() {
    let source =
        include_str!("../../../../../architecture/plot/tests/fixtures/expression_baseline.conduit");
    let (_, _, program) = prepare(source, "baseline-1", true);
    assert!(conduit_std_offers::pure_scalar_tape_std_offer(
        &program,
        conduit_core::PortTemporal::Flow { closes: true }
    )
    .is_err());
    let checked = check_syntax_document(
        &parse_syntax_document(
            "plot text (\n >> value: Text <= 8B\n result: Text <= 8B >>\n) = (.)\n",
        ),
        &StartupCatalog::new(),
    )
    .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "text", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    assert!(conduit_std_offers::pure_scalar_tape_std_offer(
        &program,
        conduit_core::PortTemporal::Value
    )
    .is_err());
    assert!(conduit_std_offers::pure_expression_std_offer(
        &program,
        conduit_core::PortTemporal::Value
    )
    .is_ok());
}
