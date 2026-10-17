//! Genuine selected leases and original Kernel tokens under deterministic xHCI
//! observations. Configure Endpoint/transfer/command events are simulated below
//! physics; no checked class-control recovery or device execution is claimed.
use super::event_routing::PendingEventKey;
use super::*;
use crate::arch::xhci_mechanism::{Event, synthetic_next_stop_ack, synthetic_transfer_event};
use crate::usb_base::{
    endpoint_read_owner::tests::fixture as read, endpoint_write_owner::tests::fixture as write,
};
use conduit_composite::{AdmittedKernelCompositeHostRequest, KernelCompositeHost};
use conduit_core::{ValuePayload, port_id};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Success,
    FailedReset,
    FailedDequeue,
    LateOriginal,
}

fn event(key: PendingEventKey, code: u8) -> Event {
    Event {
        event_type: 32,
        completion_code: code,
        slot: key.slot,
        endpoint: key.endpoint,
        residual: 0,
        pointer: key.pointer,
    }
}
fn position(host: &UsbNativeHost<'_>, index: UsbNativeCallIndex) -> (usize, u32) {
    match &host.calls[index.0].transfer {
        Transfer::Endpoint(t) => t.ring_position(),
        Transfer::Write(t) => t.ring_position(),
        _ => panic!("bulk"),
    }
}
fn retains(host: &UsbNativeHost<'_>, index: UsbNativeCallIndex) -> bool {
    match &host.calls[index.0].transfer {
        Transfer::Endpoint(t) => t.retains_settlement_obligation(),
        Transfer::Write(t) => t.retains_settlement_obligation(),
        _ => panic!("bulk"),
    }
}
fn stage_command(host: &mut UsbNativeHost<'_>, code: u8) {
    // Stage an inert fixture event before the exact NEXT original command
    // acknowledgement; consume the inert event without touching a native TD.
    unsafe {
        synthetic_transfer_event(
            &host.controller,
            Event {
                event_type: 32,
                completion_code: 1,
                slot: 0,
                endpoint: 0,
                residual: 0,
                pointer: 0,
            },
            Some(code),
        );
    }
    assert_eq!(host.controller.poll_event().unwrap().slot, 0);
}
fn admit(
    kernel: &mut KernelCompositeHost,
    port: &str,
    sequence: u64,
    value: &ValuePayload,
) -> AdmittedKernelCompositeHostRequest {
    kernel.admit_input(&port_id(port), sequence, value).unwrap();
    for _ in 0..64 {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            let exact = kernel.host_request_obligation(&request).unwrap();
            return kernel
                .admit_host_request(&request, &exact.host, &exact.resources, &exact.authorities)
                .unwrap();
        }
    }
    panic!("original selected bulk request");
}
fn begin(
    host: &mut UsbNativeHost<'_>,
    index: UsbNativeCallIndex,
    read_direction: bool,
    kernel: &KernelCompositeHost,
    token: &AdmittedKernelCompositeHostRequest,
) {
    let call = *kernel.admitted_host_request_view(token).unwrap().request;
    assert_eq!(call.node, host.calls[index.0].node);
    let input = kernel.host_request_input(token).unwrap();
    if read_direction {
        host.begin_independent_read(index, call.request, input)
            .unwrap();
    } else {
        host.begin_independent_write(index, call.request, input)
            .unwrap();
    }
}
fn run(read_direction: bool, case: Case) {
    let plan = crate::protocol_host_calls::recovery_fixture_plan();
    crate::arch::NativeUsbDuplexHost::with_recovery_fixture(&plan, |native| {
        let (mut host, read_index, write_index) = native.into_recovery_fixture();
        // The fixture initial command is unrelated to either original TD.
        assert_eq!(host.controller.poll_event().unwrap().event_type, 33);
        let index = if read_direction {
            read_index
        } else {
            write_index
        };
        let direction = usize::from(!read_direction);
        let port = if read_direction { "read" } else { "write" };
        let output = if read_direction {
            "received"
        } else {
            "written"
        };
        let value = ValuePayload {
            value_kind: plan.fragments[0].placements[direction].inputs[0]
                .value_kind
                .clone(),
            encoded: if read_direction {
                read::input_length(8)
            } else {
                write::input_wire(&[1, 2, 3, 4])
            },
        };
        let nodes = [
            host.calls[read_index.0].node,
            host.calls[write_index.0].node,
        ];
        let original_host = plan.fragments[0].host_id.clone();
        let mut kernel = crate::protocol_host_calls::recovery_fixture_kernel(plan.clone());
        kernel.start().unwrap();
        kernel
            .close_input(&port_id(if read_direction { "write" } else { "read" }))
            .unwrap();
        kernel.close_input(&port_id("control")).unwrap();
        let token = admit(&mut kernel, port, 0, &value);
        let original_request = kernel
            .admitted_host_request_view(&token)
            .unwrap()
            .request
            .request;
        let before = position(&host, index);
        begin(&mut host, index, read_direction, &kernel, &token);
        let submitted = position(&host, index);
        assert_ne!(submitted, before);
        let old = host.independent_keys().unwrap()[direction].unwrap();
        unsafe {
            synthetic_transfer_event(
                &host.controller,
                event(old, 6),
                Some(if case == Case::FailedReset { 6 } else { 1 }),
            );
        }
        let result = host.poll_independent();
        if case == Case::FailedReset {
            assert!(matches!(result, Err(UsbNativeHostRefusal::Native(_))));
            assert!(retains(&host, index));
            assert_eq!(position(&host, index), submitted);
            assert_eq!(host.recovery_endpoint, Some(index.0));
            assert!(host.uncertain_bulk);
            assert!(kernel.admitted_host_request_view(&token).is_ok());
            assert!(
                host.bulk_terminals
                    .as_ref()
                    .unwrap()
                    .iter()
                    .all(|s| s.index.is_none())
            );
        } else {
            let (terminal_index, bytes) = result.unwrap().unwrap();
            assert_eq!(terminal_index, index);
            let bytes = bytes.to_vec();
            let expected = if read_direction {
                crate::usb_base::endpoint_read_result::PreparedEndpointReadResultEncoder::new(
                    &crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare()
                        .unwrap(),
                )
                .unwrap()
                .disposition(EndpointReadDisposition::Stalled)
                .unwrap()
                .to_vec()
            } else {
                crate::usb_base::endpoint_write_result::PreparedEndpointWriteResultEncoder::new(
                    &crate::usb_base::endpoint_write_contract::EndpointWriteContract::prepare()
                        .unwrap(),
                )
                .unwrap()
                .disposition(EndpointWriteDisposition::Stalled)
                .unwrap()
                .to_vec()
            };
            assert_eq!(bytes, expected);
            assert!(!retains(&host, index));
            assert_eq!(position(&host, index), submitted);
            assert_eq!(host.recovery_endpoint, Some(index.0));
            assert!(!host.stop_acknowledged && !host.stopped);
            assert!(
                host.begin_independent_read(read_index, RequestId(999), &read::input_length(8))
                    .is_err()
            );
            assert!(
                host.begin_independent_write(write_index, RequestId(999), &write::input_wire(&[1]))
                    .is_err()
            );
            assert!(
                kernel
                    .has_retained_host_call_for_nodes(&original_host, &nodes)
                    .unwrap()
            );
            kernel.complete_host_call_bytes(&token, &bytes).unwrap();
            assert!(
                kernel
                    .has_retained_host_call_for_nodes(&original_host, &nodes)
                    .unwrap()
            );
            // Actual original Kernel completion is consumed before the unsafe
            // native recovery precondition is asserted, not an empty-map proxy.
            for _ in 0..64 {
                kernel.step().unwrap();
                if !kernel
                    .has_retained_host_call_for_nodes(&original_host, &nodes)
                    .unwrap()
                {
                    break;
                }
            }
            assert!(
                !kernel
                    .has_retained_host_call_for_nodes(&original_host, &nodes)
                    .unwrap()
            );
            let mut observed_output = None;
            for _ in 0..64 {
                if let Some(value) = kernel.output(&port_id(output)).unwrap() {
                    observed_output = Some(value);
                    break;
                }
                kernel.step().unwrap();
            }
            let (sequence, observed) = observed_output.expect("original typed STALL output");
            assert_eq!(observed.encoded, expected);
            kernel.complete_output(&port_id(output), sequence).unwrap();
            stage_command(&mut host, if case == Case::FailedDequeue { 6 } else { 1 });
            let prepared = unsafe { host.prepare_endpoint_recovery() };
            assert_eq!(position(&host, index), submitted);
            if case == Case::FailedDequeue {
                assert!(matches!(prepared, Err(UsbNativeHostRefusal::Native(_))));
                assert!(host.uncertain_bulk && host.recovery_endpoint == Some(index.0));
                assert!(!retains(&host, index)); // Original lease already settled by Reset ack.
                assert!(
                    host.begin_independent_read(read_index, RequestId(999), &read::input_length(8))
                        .is_err()
                );
                assert!(
                    host.begin_independent_write(
                        write_index,
                        RequestId(999),
                        &write::input_wire(&[1])
                    )
                    .is_err()
                );
            } else {
                prepared.unwrap();
                assert!(!host.uncertain_bulk && host.recovery_endpoint.is_none());
                let fresh = admit(&mut kernel, port, 1, &value);
                assert_ne!(
                    kernel
                        .admitted_host_request_view(&fresh)
                        .unwrap()
                        .request
                        .request,
                    original_request
                );
                begin(&mut host, index, read_direction, &kernel, &fresh);
                let keys = host.independent_keys().unwrap();
                let new = keys[direction].unwrap();
                assert_ne!(old.pointer, new.pointer);
                if case == Case::LateOriginal {
                    assert!(matches!(
                        host.complete_independent_event(event(old, 1), keys),
                        Err(UsbNativeHostRefusal::Binding)
                    ));
                    assert!(host.uncertain_bulk && retains(&host, index));
                    assert!(kernel.admitted_host_request_view(&fresh).is_ok());
                    assert!(kernel.output(&port_id(output)).unwrap().is_none());
                } else {
                    let (completed, bytes) = host
                        .complete_independent_event(event(new, 1), keys)
                        .unwrap()
                        .unwrap();
                    assert_eq!(completed, index);
                    let bytes = bytes.to_vec();
                    let expected = if read_direction {
                        crate::usb_base::endpoint_read_result::PreparedEndpointReadResultEncoder::new(&crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare().unwrap()).unwrap().completed(1,8,8,&[0;8]).unwrap().to_vec()
                    } else {
                        crate::usb_base::endpoint_write_result::PreparedEndpointWriteResultEncoder::new(&crate::usb_base::endpoint_write_contract::EndpointWriteContract::prepare().unwrap()).unwrap().completed(4,4).unwrap().to_vec()
                    };
                    assert_eq!(bytes, expected);
                    kernel.complete_host_call_bytes(&fresh, &bytes).unwrap();
                    assert!(!retains(&host, index));
                    assert!(host.poll_independent().unwrap().is_none());
                }
            }
        }
        let dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            assert_eq!((*dma).owner_slot, 1);
            synthetic_next_stop_ack(&host.controller);
        }
        host.cancel().unwrap();
        kernel.cancel().unwrap();
        assert!(!retains(&host, index));
        unsafe {
            assert_eq!((*dma).owner_slot, 0);
        }
    });
}
#[test]
fn single_read_stall_exact_reset_and_dequeue_ack_resume_fresh_kernel_request() {
    run(true, Case::Success);
}
#[test]
fn single_write_stall_exact_reset_and_dequeue_ack_resume_fresh_kernel_request() {
    run(false, Case::Success);
}
#[test]
fn single_read_stall_failed_reset_retains_original_lease_and_kernel_token() {
    run(true, Case::FailedReset);
}
#[test]
fn single_write_stall_failed_reset_retains_original_lease_and_kernel_token() {
    run(false, Case::FailedReset);
}
#[test]
fn single_read_stall_failed_dequeue_keeps_retired_cursor_quarantined() {
    run(true, Case::FailedDequeue);
}
#[test]
fn single_write_stall_failed_dequeue_keeps_retired_cursor_quarantined() {
    run(false, Case::FailedDequeue);
}
#[test]
fn single_read_stall_old_td_cannot_complete_fresh_kernel_request() {
    run(true, Case::LateOriginal);
}
#[test]
fn single_write_stall_old_td_cannot_complete_fresh_kernel_request() {
    run(false, Case::LateOriginal);
}
