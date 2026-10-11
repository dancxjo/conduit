//! Actual endpoint owners, capability leases, DMA publication and aggregate
//! retirement below physics. Configure Endpoint receipts and transfer events are
//! simulated; the original controller executes its real acknowledged-stop path.
use super::event_routing::PendingEventKey;
use super::*;
#[path = "paired_kernel.rs"]
mod paired_kernel;
use crate::usb_base::{
    endpoint_read_owner::tests::fixture as read, endpoint_ring::EndpointRingCursor,
    endpoint_write_owner::tests::fixture as write,
};
use conduit_core::{PlotIdentity, bind_active_play, seal_plan};
use conduit_plan_lowering::lowering::lower_plan_fragment;

fn issued(
    read_direction: bool,
    active: &conduit_core::ActivePlayIdentity,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    let mut request = if read_direction {
        read::request()
    } else {
        write::request()
    };
    let (implementation, authority) = if read_direction {
        (
            crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
            crate::usb_base::endpoint_read_factory::ENDPOINT_READ_AUTHORITY,
        )
    } else {
        (
            crate::usb_base::endpoint_write_factory::ENDPOINT_WRITE_IMPLEMENTATION,
            crate::usb_base::endpoint_write_factory::ENDPOINT_WRITE_AUTHORITY,
        )
    };
    request.scope.implementation_id = implementation.into();
    request.scope.authority_contract_id = authority.into();
    request.authority.grant.contract_id = authority.into();
    request.scope.plan_id = active.plan_id.clone();
    request.scope.active_play_id = active.active_play_id.clone();
    let scope = &request.scope;
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [if read_direction { 73 } else { 74 }; 32],
        1,
    )
    .unwrap();
    let handle = table.issue(request).unwrap();
    let mut claim = if read_direction {
        read::claim()
    } else {
        write::claim()
    };
    claim.implementation_id = implementation.into();
    claim.plan_id = active.plan_id.clone();
    claim.active_play_id = active.active_play_id.clone();
    (table, handle, claim)
}
fn event(key: PendingEventKey, code: u8) -> crate::arch::xhci_mechanism::Event {
    crate::arch::xhci_mechanism::Event {
        event_type: 32,
        completion_code: code,
        slot: key.slot,
        endpoint: key.endpoint,
        residual: 0,
        pointer: key.pointer,
    }
}

#[test]
fn paired_original_endpoint_leases_route_out_before_in_without_double_completion() {
    paired(1, false, false);
}
#[test]
fn paired_stall_retires_both_original_leases_only_after_acknowledged_stop() {
    paired(1, true, false);
}
#[test]
fn paired_stall_failed_stop_retains_both_original_dma_reservations() {
    paired(6, true, false);
}
#[test]
fn paired_acknowledged_cancel_rejects_late_original_events_and_both_kernel_tokens() {
    paired(1, false, true);
}
fn paired(stop_code: u8, stalled: bool, cancelled: bool) {
    crate::arch::xhci_mechanism::with_synthetic_stop_controller(1, stop_code, 0, |controller| {
        let (mut fragment, _, _, read_placement) = read::selected();
        let (write_fragment, _, _, write_placement) = write::selected();
        fragment.placements.extend(write_fragment.placements);
        fragment.startup_order.push(write_placement.clone());
        for gear in &mut fragment.placements {
            let (implementation, profile, base, attachment, authority) =
                if gear.placement_id == read_placement {
                    use crate::usb_base::endpoint_read_factory::*;
                    (
                        ENDPOINT_READ_IMPLEMENTATION,
                        ENDPOINT_READ_PROFILE,
                        ENDPOINT_READ_BASE,
                        ENDPOINT_READ_ATTACHMENT,
                        ENDPOINT_READ_AUTHORITY,
                    )
                } else {
                    use crate::usb_base::endpoint_write_factory::*;
                    (
                        ENDPOINT_WRITE_IMPLEMENTATION,
                        ENDPOINT_WRITE_PROFILE,
                        ENDPOINT_WRITE_BASE,
                        ENDPOINT_WRITE_ATTACHMENT,
                        ENDPOINT_WRITE_AUTHORITY,
                    )
                };
            gear.implementation_id = implementation.into();
            gear.execution_profile_id = profile.into();
            gear.base.as_mut().unwrap().implementation_id = base.into();
            gear.base.as_mut().unwrap().mechanism_family = attachment.into();
            gear.resources[0].class_id = attachment.into();
            gear.authority[0].contract_id = authority.into();
        }
        fragment
            .expected_sign
            .extend(write_fragment.expected_sign.into_iter().filter(|sign| {
                matches!(
                    sign,
                    conduit_core::ExpectedSign::PlacementPrepared(_)
                        | conduit_core::ExpectedSign::PlacementTerminal(_)
                )
            }));
        fragment.sign_storage_budget =
            conduit_core::mandatory_sign_storage_requirement(&fragment.expected_sign).unwrap();
        let identity = PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        };
        let plan = seal_plan(identity, alloc::vec![fragment]);
        let fragment = plan.fragments[0].clone();
        let mut kernel = paired_kernel::prepare(plan);
        let lowered = lower_plan_fragment(&fragment).unwrap();
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let mut read_ring = [[0; 4]; 64];
        let mut write_ring = [[0; 4]; 64];
        let mut read_buffer = [0; 2048];
        let mut write_buffer = [0; 2048];
        let mut read_cursor = EndpointRingCursor::new(64).unwrap();
        let mut write_cursor = EndpointRingCursor::new(64).unwrap();
        let device =
            super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        let dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            (*dma).owner_slot = 1;
            (*dma).owner_root_port = 1;
            (*dma).owner_epoch = 1;
        }
        let mut host = unsafe { UsbNativeHost::new(controller, device, Some) }.unwrap();
        let (table, handle, claim) = issued(true, &active);
        let read_index = unsafe {
            host.add_endpoint(
                super::super::endpoint_setup::ConfiguredInboundEndpoint {
                    slot: 1,
                    device_epoch: 1,
                    endpoint_epoch: 9,
                    dci: 3,
                    ring_physical: 0x40000,
                },
                EndpointReceiveDma {
                    ring: &mut read_ring,
                    buffer: &mut read_buffer,
                    cursor: &mut read_cursor,
                    ring_physical: 0x40000,
                    buffer_physical: 0x50000,
                },
                EndpointReadAttachment {
                    slot: 1,
                    generation: 1,
                    endpoint_generation: 9,
                    endpoint_dci: 3,
                    maximum_data_bytes: 512,
                    resource_bytes: 4096,
                },
                table,
                handle,
                claim,
                &EndpointReadContract::prepare().unwrap(),
                SelectedOperationPlan {
                    fragment: &fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &read_placement,
                },
            )
        }
        .unwrap();
        let (table, handle, claim) = issued(false, &active);
        let write_index = unsafe {
            host.add_write(
                super::super::endpoint_setup::ConfiguredOutboundEndpoint {
                    slot: 1,
                    device_epoch: 1,
                    endpoint_epoch: 9,
                    dci: 2,
                    ring_physical: 0x60000,
                },
                EndpointTransmitDma {
                    ring: &mut write_ring,
                    buffer: &mut write_buffer,
                    cursor: &mut write_cursor,
                    ring_physical: 0x60000,
                    buffer_physical: 0x70000,
                },
                EndpointWriteAttachment {
                    slot: 1,
                    generation: 1,
                    endpoint_generation: 9,
                    endpoint_dci: 2,
                    maximum_data_bytes: 512,
                    resource_bytes: 4096,
                },
                table,
                handle,
                claim,
                &EndpointWriteContract::prepare().unwrap(),
                SelectedOperationPlan {
                    fragment: &fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &write_placement,
                },
            )
        }
        .unwrap();
        host.prepare_independent().unwrap();
        let tokens = paired_kernel::admit_pair(
            &mut kernel,
            [
                conduit_core::ValuePayload {
                    value_kind: fragment.placements[0].inputs[0].value_kind.clone(),
                    encoded: read::input_length(8),
                },
                conduit_core::ValuePayload {
                    value_kind: fragment.placements[1].inputs[0].value_kind.clone(),
                    encoded: write::input_wire(&[1, 2, 3, 4]),
                },
            ],
        );
        for (direction, index) in [read_index, write_index].into_iter().enumerate() {
            let call = *kernel
                .admitted_host_request_view(&tokens[direction])
                .unwrap()
                .request;
            assert_eq!(call.node, host.calls[index.0].node);
            let input = kernel.host_request_input(&tokens[direction]).unwrap();
            if direction == 0 {
                host.begin_independent_read(index, call.request, input)
                    .unwrap();
            } else {
                host.begin_independent_write(index, call.request, input)
                    .unwrap();
            }
        }
        let keys = host.independent_keys().unwrap();
        assert_eq!(keys[0].unwrap().index, read_index);
        assert_eq!(keys[1].unwrap().index, write_index);
        if cancelled {
            host.cancel().unwrap();
            kernel.cancel().unwrap();
            assert!(host.stopped && host.stop_acknowledged);
            assert_eq!(host.independent_keys().unwrap(), [None, None]);
            for direction in 0..2 {
                assert!(matches!(
                    host.complete_independent_event(event(keys[direction].unwrap(), 1), keys),
                    Err(UsbNativeHostRefusal::NotReady)
                ));
                assert!(kernel.host_request_input(&tokens[direction]).is_err());
                assert!(
                    kernel
                        .complete_host_call_bytes(&tokens[direction], &[])
                        .is_err()
                );
            }
            assert!(host.poll_independent().is_err());
            unsafe {
                assert_eq!((*dma).owner_slot, 0);
            }
            return;
        }

        assert!(
            host.begin_independent_read(read_index, RequestId(13), &read::input_length(8))
                .is_err()
        );
        let completion = host
            .complete_independent_event(event(keys[1].unwrap(), if stalled { 6 } else { 1 }), keys);
        if stalled && stop_code != 1 {
            assert!(completion.is_err());
            assert!(kernel.host_request_input(&tokens[0]).is_ok());
            assert!(kernel.host_request_input(&tokens[1]).is_ok());
            assert_eq!(host.pending_endpoint, Some(read_index.0));
            assert_eq!(host.pending_write, Some(write_index.0));
            assert_eq!(host.independent_keys().unwrap(), keys);
            assert!(!host.stop_acknowledged);
            assert!(host.poll_independent().is_err());
        } else if stalled {
            let (first, bytes) = completion.unwrap().unwrap();
            assert_eq!(first, read_index);
            kernel.complete_host_call_bytes(&tokens[0], bytes).unwrap();
            assert!(kernel.complete_host_call_bytes(&tokens[0], bytes).is_err());
            assert!(kernel.host_request_input(&tokens[1]).is_ok());
            assert!(host.stopped && host.stop_acknowledged);
            let (second, bytes) = host.poll_independent().unwrap().unwrap();
            assert_eq!(second, write_index);
            kernel.complete_host_call_bytes(&tokens[1], bytes).unwrap();
            assert!(kernel.host_request_input(&tokens[1]).is_err());
            assert_eq!(
                conduit_core::validate_canonical_structured_value(bytes)
                    .unwrap()
                    .variant_tag()
                    .unwrap(),
                "stalled"
            );
            assert!(host.poll_independent().is_err());
        } else {
            let (completed, bytes) = completion.unwrap().unwrap();
            assert_eq!(completed, write_index);
            kernel.complete_host_call_bytes(&tokens[1], bytes).unwrap();
            assert!(kernel.complete_host_call_bytes(&tokens[1], bytes).is_err());
            assert!(kernel.host_request_input(&tokens[0]).is_ok());
            assert_eq!(host.pending_endpoint, Some(read_index.0));
            assert!(host.pending_write.is_none());
            let remaining = host.independent_keys().unwrap();
            let (completed, bytes) = host
                .complete_independent_event(event(remaining[0].unwrap(), 1), remaining)
                .unwrap()
                .unwrap();
            assert_eq!(completed, read_index);
            kernel.complete_host_call_bytes(&tokens[0], bytes).unwrap();
            assert!(kernel.host_request_input(&tokens[0]).is_err());
            assert_eq!(host.independent_keys().unwrap(), [None, None]);
            assert!(
                host.complete_independent_event(event(keys[1].unwrap(), 1), [None, None])
                    .is_err()
            );
        }
        // This fixture owns stack-backed DMA. Explicitly retire/destroy transfer
        // borrows before their storage leaves this scope; failed-stop teardown is
        // fixture disposal, never evidence of hardware quiescence.
        host.calls.clear();
        drop(host);
        unsafe {
            (*dma).owner_slot = 0;
        }
    });
}
