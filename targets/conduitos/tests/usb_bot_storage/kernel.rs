//! Actual prepared graph closure; no physical USB effects are permitted here.
use super::*;
use conduit_composite::KernelCompositeStatus;
use conduit_kernel::scheduler::RemoteIngressOutcome;

#[test]
fn whole_storage_back_drains_one_and_sixteen_unsupported_requests_without_usb() {
    for count in [1usize, 16] {
        let mut run = execution::Execution::prepare(package());
        let input = run.kernel.definition().external_capability.inputs[0].clone();
        let output = run.kernel.definition().external_capability.outputs[0].clone();
        let contract = conduitos::storage_base::contract::BlockReadContract::prepare().unwrap();
        let value = record(contract.request_type(), |field| {
            scalar(
                field.value_type(),
                match field.name() {
                    "block_size" => 512,
                    "count" => 1,
                    "lba" => 1,
                    _ => unreachable!(),
                },
            )
        });
        let input_value = ValuePayload {
            value_kind: input.value_kind,
            encoded: value.canonical_bytes().unwrap(),
        };
        let mut output_value = ValuePayload {
            value_kind: output.value_kind,
            encoded: Vec::with_capacity(4096),
        };
        let mut sent = 0;
        let mut closed = false;
        let mut seen = 0;
        let mut retired = false;
        run.kernel.start().unwrap();
        for _ in 0..30000 {
            if sent < count
                && matches!(
                    run.kernel
                        .admit_input(&input.port_id, sent as u64, &input_value)
                        .unwrap(),
                    RemoteIngressOutcome::Accepted { .. }
                )
            {
                sent += 1;
            }
            if sent == count && !closed {
                run.kernel.close_input(&input.port_id).unwrap();
                closed = true;
            }
            let status = run.kernel.step().unwrap();
            if let Some(request) = run.kernel.next_host_request() {
                assert!(
                    run.service_pure(&request),
                    "unsupported first request must not cross any USB Host Call"
                );
            }
            if let Some(sequence) = run
                .kernel
                .output_into(&output.port_id, &mut output_value)
                .unwrap()
            {
                seen += 1;
                assert_eq!(seen, 1, "drain must suppress duplicate failure output");
                let result = validate_canonical_structured_value(&output_value.encoded).unwrap();
                assert_eq!(result.variant_tag().unwrap(), "unsupported");
                let identity = result.variant_payload("unsupported").unwrap().unwrap();
                assert_eq!(
                    identity
                        .record_field("lba")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    &1u64.to_le_bytes()
                );
                assert_eq!(
                    identity
                        .record_field("ordinal")
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    &0u64.to_le_bytes()
                );
                run.kernel
                    .complete_output(&output.port_id, sequence)
                    .unwrap();
            }
            if status == KernelCompositeStatus::Complete {
                retired = true;
                break;
            }
        }
        assert_eq!(sent, count);
        assert!(
            closed && retired,
            "whole prepared graph must retire after explicit closure"
        );
        assert_eq!(seen, 1);
        run.kernel.cancel().unwrap();
    }
}
