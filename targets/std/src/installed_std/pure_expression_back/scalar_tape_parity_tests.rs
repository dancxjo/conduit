//! Baseline/tape differential conformance through the actual installed host.
use super::scalar_tape_fixture::{prepare, NoTimer, Outputs};
use crate::ExternalForeInput;
use conduit_core::ConnectionTrack;

#[test]
fn checked_signed_values_and_overflow_have_identical_evaluator_outcomes() {
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    let (_, _, program) = prepare(source, "signed", true);
    let mut baseline = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let mut tape =
        conduit_plot::PreparedPortableExpressionEvaluator::new_scalar_tape(&program).unwrap();
    for n in [i64::MIN, -100, -1, 0, 1, 100, i64::MAX] {
        let bytes = n.to_le_bytes();
        let a = baseline.evaluate(&bytes).map(|x| x.to_vec());
        let b = tape.evaluate(&bytes).map(|x| x.to_vec());
        assert_eq!(a, b);
    }
}
#[test]
fn source_identity_and_actual_kernel_outputs_match_for_negative_input() {
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    let mut results = Vec::new();
    for tape in [false, true] {
        let (mut host, plan, program) = prepare(source, "signed", tape);
        let input = (-42i64).to_le_bytes();
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
        let kernel = report.kernel.as_ref().unwrap();
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(output.0, [expected]);
        results.push((
            plan.source_document_id,
            plan.checked_plot_id,
            plan.expanded_plot_id,
            output.0,
            report.observations.last().unwrap().kind.clone(),
            kernel
                .kernel_sign
                .iter()
                .map(|s| s.kind)
                .collect::<Vec<_>>(),
        ));
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn invalid_input_and_program_refusals_preserve_logical_attribution() {
    use conduit_plot::{
        PortableExpressionEvaluationRefusal as Refusal,
        PreparedPortableExpressionEvaluator as Evaluator,
    };
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    let (_, _, program) = prepare(source, "signed", true);
    let mut baseline = Evaluator::new(&program).unwrap();
    let mut tape = Evaluator::new_scalar_tape(&program).unwrap();
    for bytes in [vec![], vec![0; 7], vec![0; 9]] {
        let a = baseline.evaluate(&bytes).unwrap_err();
        let b = tape.evaluate(&bytes).unwrap_err();
        assert_eq!(a, Refusal::InvalidInput);
        assert_eq!(a, b);
        assert_eq!(super::refusal_detail(&a), super::refusal_detail(&b));
    }
    let mut malformed = program;
    malformed.input_maximum_bytes = Some(u32::MAX);
    assert!(matches!(
        Evaluator::new(&malformed),
        Err(Refusal::InvalidProgram)
    ));
    assert!(matches!(
        Evaluator::new_scalar_tape(&malformed),
        Err(Refusal::InvalidProgram)
    ));
}

#[test]
fn installed_back_refuses_foreign_provenance_and_malformed_configuration_before_play() {
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    for tape in [false, true] {
        let (_, plan, _) = prepare(source, "signed", tape);
        let placement = plan.fragments[0].placements[0].clone();
        let mut foreign = placement.clone();
        foreign.artifact_id = conduit_core::ArtifactId::from("foreign/scalar@1");
        assert!(super::PureExpressionHost::from_placement(&foreign).is_err());
        let mut malformed = placement;
        malformed.configuration[0].value = conduit_core::ConfigurationValue::Text("not-hex".into());
        assert!(super::PureExpressionHost::from_placement(&malformed).is_err());
    }
}

#[test]
fn actual_kernel_arithmetic_refusal_matches_baseline_sign_and_terminal() {
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    let mut results = Vec::new();
    for tape in [false, true] {
        let (mut host, plan, _) = prepare(source, "signed", tape);
        let mut output = Outputs(Vec::with_capacity(1));
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: i64::MAX.to_le_bytes().to_vec(),
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        );
        assert!(output.0.is_empty());
        results.push(match result {
            Ok(report) => format!(
                "{:?}/{:?}",
                report.observations.last().unwrap().kind,
                report
                    .kernel
                    .unwrap()
                    .kernel_sign
                    .iter()
                    .map(|e| e.kind)
                    .collect::<Vec<_>>()
            ),
            Err(error) => error,
        });
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn sealed_value_input_storage_and_one_value_scope_refuse_before_delivery() {
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    for bytes in [vec![0; 7], vec![0; 9], vec![0; 4097]] {
        let mut errors = Vec::new();
        for tape in [false, true] {
            let (mut host, plan, _) = prepare(source, "signed", tape);
            let mut output = Outputs(Vec::with_capacity(1));
            errors.push(
                host.run_external_plot_to(
                    plan.fragments[0].clone(),
                    &[ExternalForeInput {
                        front_port_id: "value".into(),
                        track: ConnectionTrack::Payload,
                        bytes: bytes.clone(),
                    }],
                    &mut output,
                    &mut Vec::new(),
                    &mut NoTimer,
                )
                .unwrap_err(),
            );
            assert!(output.0.is_empty());
        }
        assert_eq!(errors[0], errors[1]);
        assert!(errors[0].contains("sealed value contract"));
    }
    for tape in [false, true] {
        let (mut host, plan, _) = prepare(source, "signed", tape);
        let inputs = (0..2)
            .map(|_| ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: 1i64.to_le_bytes().to_vec(),
            })
            .collect::<Vec<_>>();
        let mut output = Outputs(Vec::with_capacity(1));
        let error = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &inputs,
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap_err();
        assert_eq!(
            error,
            "external Fore input set does not match the sealed Plan"
        );
        assert!(output.0.is_empty());
    }
}
