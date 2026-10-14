//! Value Fore cancellation and adapter pressure through the ordinary host owner.
use super::*;
#[path = "../installed_std/pure_expression_back/scalar_tape_fixture.rs"]
mod fixture;

#[test]
fn baseline_and_tape_cancel_before_input_with_identical_terminal_and_sign_kinds() {
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    let mut results = Vec::new();
    for tape in [false, true] {
        let (mut host, plan, _) = fixture::prepare(source, "signed", tape);
        let control = RunControl::default();
        control
            .request_stop(RunControlRequestId::new("before-input").unwrap())
            .unwrap();
        let mut outputs = fixture::Outputs(Vec::with_capacity(1));
        let report = host
            .run_external_plot_with_mode_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: conduit_core::ConnectionTrack::Payload,
                    bytes: (-42i64).to_le_bytes().to_vec(),
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut fixture::NoTimer,
                &control,
                false,
            )
            .unwrap();
        assert!(outputs.0.is_empty());
        assert_eq!(
            report.control_receipts[0].request_id.as_str(),
            "before-input"
        );
        assert!(matches!(
            report.observations.last().unwrap().kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Cancelled { .. }
            }
        ));
        results.push((
            report.observations.last().unwrap().kind.clone(),
            report
                .kernel
                .unwrap()
                .kernel_sign
                .iter()
                .map(|e| e.kind)
                .collect::<Vec<_>>(),
        ));
    }
    assert_eq!(results[0], results[1]);
}

#[test]
fn output_adapter_refusal_is_preserved_without_hidden_retry_or_completion() {
    struct Refuse {
        calls: usize,
    }
    impl ExternalForeOutputAdapter for Refuse {
        fn deliver(&mut self, _: ExternalForeDelivery) -> Result<(), String> {
            self.calls += 1;
            Err("fixture/output-pressure@1".into())
        }
    }
    let source = "plot signed (\n >> value: I64\n result: I64 >>\n) = (. + .)\n";
    for tape in [false, true] {
        let (mut host, plan, _) = fixture::prepare(source, "signed", tape);
        let mut output = Refuse { calls: 0 };
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: conduit_core::ConnectionTrack::Payload,
                bytes: (-42i64).to_le_bytes().to_vec(),
            }],
            &mut output,
            &mut Vec::new(),
            &mut fixture::NoTimer,
        );
        assert_eq!(result.unwrap_err(), "fixture/output-pressure@1");
        assert_eq!(output.calls, 1);
    }
}
