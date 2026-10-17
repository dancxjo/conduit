//! Sustained checked control execution with explicitly admitted Sign history.
use super::*;

#[test]
fn admitted_sign_storage_runs_sixty_four_control_calls_and_normal_closure() {
    let contract = ControlContract::prepare().unwrap();
    let (mut kernel, input_port, output_port) =
        prepared_with_sign_storage(KernelCompositeSignStorage {
            additional_local_items: 2048,
            additional_remote_items: 256,
        })
        .unwrap();
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: input(&contract),
    };
    let raw = crate::usb_base::control_request::ControlTransferRequest::new(
        [128, 6, 0, 1, 0, 0, 8, 0],
        &[],
        256,
    )
    .unwrap();
    let mut encoder =
        crate::usb_base::control_result::PreparedControlResultEncoder::new(&contract).unwrap();
    let result = encoder.completed(&raw, 0, &[]).unwrap().to_vec();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(CONTROL_MAXIMUM_BYTES as usize),
    };
    let capacity = output.encoded.capacity();
    kernel.start().unwrap();
    for sequence in 0..64 {
        assert!(
            matches!(kernel.admit_input(&input_port.port_id, sequence, &input).unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: actual }
                if actual == sequence)
        );
        if sequence == 63 {
            kernel.close_input(&input_port.port_id).unwrap();
        }
        let mut dispatched = false;
        let mut delivered = false;
        for _ in 0..16 {
            kernel.step().unwrap();
            if let Some(request) = kernel.next_host_request() {
                assert!(!dispatched, "duplicate effect for {sequence}");
                let obligation = kernel.host_request_obligation(&request).unwrap();
                let admitted = kernel
                    .admit_host_request(
                        &request,
                        &obligation.host,
                        &obligation.resources,
                        &obligation.authorities,
                    )
                    .unwrap();
                kernel.complete_host_call_bytes(&admitted, &result).unwrap();
                dispatched = true;
            }
            if let Some(actual) = kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
            {
                assert_eq!(actual, sequence);
                assert_eq!(output.encoded, result);
                assert_eq!(output.encoded.capacity(), capacity);
                kernel
                    .complete_output(&output_port.port_id, actual)
                    .unwrap();
                delivered = true;
                break;
            }
        }
        assert!(
            dispatched && delivered,
            "unfinished control call {sequence}"
        );
    }
    let mut complete = false;
    for _ in 0..32 {
        if kernel.step().unwrap() == KernelCompositeStatus::Complete {
            complete = true;
            break;
        }
        assert!(kernel.next_host_request().is_none());
    }
    assert!(complete);
    assert_eq!(
        kernel
            .output_terminal_into(&output_port.port_id, &mut output)
            .unwrap(),
        Some(KernelCompositeTerminal::Normal)
    );
}

#[test]
fn overflowing_sign_storage_is_refused_before_play() {
    for sign_storage in [
        KernelCompositeSignStorage {
            additional_local_items: u16::MAX,
            additional_remote_items: 0,
        },
        KernelCompositeSignStorage {
            additional_local_items: 0,
            additional_remote_items: u16::MAX,
        },
    ] {
        assert!(matches!(
            prepared_with_sign_storage(sign_storage),
            Err(KernelCompositeError::ChildRefused { .. })
        ));
    }
}
