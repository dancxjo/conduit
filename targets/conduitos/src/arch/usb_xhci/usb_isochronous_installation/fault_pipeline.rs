//! Composed native fault retirement and checked Source, below controller physics.
//! Raw controller events and Configure Endpoint are simulated. Disable Slot,
//! native event decoding, original lease retirement and result encoding are real.
//! This starts from the checked playback request; setup and clock-profile
//! acceptance remain covered by the separate ordinary guest proof.
use super::*;
use crate::arch::xhci_mechanism;
use crate::usb_base::isochronous::owner::tests::fixture;
use alloc::{boxed::Box, vec, vec::Vec};
use conduit_kernel::{HostCallId, RequestId};
#[path = "fault_pipeline/kernel.rs"]
mod kernel;
#[path = "fault_pipeline/source.rs"]
mod source;

fn plan() -> Plan {
    let (mut fragment, _, _, _) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.implementation_id = ISOCHRONOUS_OUT_IMPLEMENTATION.into();
    gear.execution_profile_id = ISOCHRONOUS_OUT_PROFILE.into();
    gear.base.as_mut().unwrap().implementation_id = ISOCHRONOUS_OUT_BASE.into();
    gear.base.as_mut().unwrap().mechanism_family = ISOCHRONOUS_OUT_ATTACHMENT.into();
    gear.resources[0].class_id = ISOCHRONOUS_OUT_ATTACHMENT.into();
    gear.authority[0].contract_id = ISOCHRONOUS_OUT_AUTHORITY.into();
    kernel::admit_fore(&mut fragment);
    seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
}
fn owner(plan: &Plan) -> IsochronousOutCallOwner {
    let fragment = &plan.fragments[0];
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let mut request = fixture::request();
    request.scope.plan_id = plan.plan_id.clone();
    request.scope.active_play_id = active.active_play_id.clone();
    request.scope.implementation_id = ISOCHRONOUS_OUT_IMPLEMENTATION.into();
    request.scope.authority_contract_id = ISOCHRONOUS_OUT_AUTHORITY.into();
    request.authority.grant.contract_id = ISOCHRONOUS_OUT_AUTHORITY.into();
    let scope = &request.scope;
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [91; 32],
        1,
    )
    .unwrap();
    let handle = table.issue(request).unwrap();
    let mut claim = fixture::claim();
    claim.plan_id = plan.plan_id.clone();
    claim.active_play_id = active.active_play_id.clone();
    claim.implementation_id = ISOCHRONOUS_OUT_IMPLEMENTATION.into();
    unsafe {
        IsochronousOutCallOwner::bind_admitted(
            table,
            handle,
            claim,
            IsochronousAttachment {
                slot: 1,
                generation: 7,
                endpoint_generation: 9,
                endpoint_dci: 2,
                maximum_packet_bytes: 192,
                resource_bytes: 65536,
            },
            &IsochronousOutContract::prepare().unwrap(),
            SelectedOperationPlan {
                fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &fragment.placements[0].placement_id,
            },
        )
    }
    .unwrap()
}

fn exercise(code: u8, failed_stop: bool, cancel: bool) {
    let mut pipeline = source::Pipeline::prepare();
    let plan = plan();
    let owner = owner(&plan);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    let node = lowered
        .identity
        .node_for_placement(&plan.fragments[0].placements[0].placement_id)
        .unwrap();
    let mut kernel = kernel::prepare(plan);
    xhci_mechanism::with_synthetic_stop_controller(1, 1, 0, |controller| {
        let mut device =
            super::super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        device.attachment_epoch = 7;
        let global =
            super::super::super::dma::dma_pointer(super::super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            (*global).owner_slot = 1;
            (*global).owner_root_port = 1;
            (*global).owner_epoch = 7;
        }
        let mut storage = Box::new(FixtureDma {
            ring: [[0; 4]; 64],
            padding: [0; 1024],
            wire: [0; 2048],
            cursor: crate::usb_base::endpoint_ring::EndpointRingCursor::with_maximum_pending(64, 8)
                .unwrap(),
        });
        let pointer = &mut *storage as *mut FixtureDma;
        let ring = unsafe { (*pointer).ring.as_mut_ptr() } as u64;
        let buffer = unsafe { (*pointer).wire.as_mut_ptr() } as u64;
        let mut host = unsafe { UsbNativeHost::new(controller, device, Some) }.unwrap();
        // SAFETY: the original allocation outlives the private runtime and all
        // acknowledgements, including failed-stop quarantine; no borrow escapes.
        unsafe {
            host.add_isochronous(
                ConfiguredIsochronousOutboundEndpoint {
                    configured: super::super::super::endpoint_setup::ConfiguredOutboundEndpoint {
                        slot: 1,
                        device_epoch: 7,
                        endpoint_epoch: 9,
                        dci: 2,
                        ring_physical: ring,
                    },
                    geometry: crate::usb_base::isochronous_schedule::IsochronousOutGeometry::usb2(
                        1, 192, 1,
                    )
                    .unwrap(),
                },
                EndpointTransmitDma {
                    ring: &mut (*pointer).ring,
                    buffer: &mut (*pointer).wire,
                    cursor: &mut (*pointer).cursor,
                    ring_physical: ring,
                    buffer_physical: buffer,
                },
            )
        }
        .unwrap();
        let mut runtime = NativeUsbIsochronousHost {
            host,
            owner,
            control_index: UsbNativeCallIndex::synthetic_unused(),
            control_node: conduit_kernel::NodeId(u16::MAX),
            iso_node: node,
            endpoint_epoch: 9,
            pending: None,
        };
        kernel.start().unwrap();
        let input = ValuePayload {
            value_kind: IsochronousOutContract::prepare().unwrap().kind().inputs[0]
                .value_kind
                .clone(),
            encoded: pipeline.request.clone(),
        };
        kernel.admit_input(&port_id("request"), 0, &input).unwrap();
        let original = loop {
            kernel.step().unwrap();
            if let Some(request) = kernel.next_host_request() {
                let exact = kernel.host_request_obligation(&request).unwrap();
                break kernel
                    .admit_host_request(&request, &exact.host, &exact.resources, &exact.authorities)
                    .unwrap();
            }
        };
        let request_id = kernel
            .admitted_host_request_view(&original)
            .unwrap()
            .request
            .request;
        let result_port = port_id("result");
        let mut output = ValuePayload {
            value_kind: kernel.definition().external_capability.outputs[0]
                .value_kind
                .clone(),
            encoded: Vec::with_capacity(32768),
        };
        let allocations = crate::test_allocations::allocations(|| {
            runtime
                .isochronous_begin(
                    node,
                    HostCallId(0),
                    request_id,
                    kernel.host_request_input(&original).unwrap(),
                )
                .unwrap();
            assert_eq!(runtime.host.synthetic_iso_pending(), 8);
            assert!(runtime.host.drain_isochronous(9).is_err());
            assert!(
                runtime
                    .isochronous_begin(
                        node,
                        HostCallId(0),
                        RequestId(request_id.0 + 1),
                        &input.encoded
                    )
                    .is_err()
            );
            assert_eq!(runtime.host.synthetic_iso_pending(), 8);
            for index in 0..4 {
                assert_eq!(runtime.host.synthetic_iso_wire(), pipeline.wire(index));
                runtime.host.synthetic_iso_event(1, 0, None);
                assert!(runtime.isochronous_poll().unwrap().is_none());
                assert_eq!(runtime.host.synthetic_iso_pending(), 8);
            }
            assert_eq!(runtime.host.synthetic_iso_wire(), pipeline.wire(4));
            let late = runtime.host.synthetic_iso_event(
                code,
                if code == 13 { 4 } else { 0 },
                Some(if failed_stop { 2 } else { 1 }),
            );
            if cancel {
                runtime.host.synthetic_original_stop_ack();
                runtime.cancel().unwrap();
                kernel.cancel().unwrap();
                assert!(runtime.isochronous_poll().is_err());
            } else if failed_stop {
                assert!(runtime.isochronous_poll().is_err());
                assert!(runtime.pending.is_some());
                assert!(runtime.host.synthetic_iso_pending() > 0);
                assert!(
                    kernel
                        .has_retained_host_call_for_nodes(&kernel.definition().host_id, &[node])
                        .unwrap()
                );
                runtime.host.synthetic_original_stop_ack();
                assert!(runtime.cancel().is_err());
                assert!(runtime.pending.is_some());
                assert!(runtime.host.synthetic_iso_pending() > 0);
                // A late original acknowledgement cannot settle the newer command.
                // Cleanup explicitly acknowledges the next original-owner command.
                runtime.host.synthetic_next_stop_ack();
                runtime.cancel().unwrap();
                kernel.cancel().unwrap();
            } else {
                let bytes = runtime
                    .isochronous_poll()
                    .unwrap()
                    .expect("native terminal after stop");
                pipeline.check(
                    bytes,
                    match code {
                        13 => "short",
                        23 => "missed-service",
                        14 => "underrun",
                        _ => unreachable!(),
                    },
                    4,
                );
                kernel.complete_host_call_bytes(&original, bytes).unwrap();
                let sequence = loop {
                    kernel.step().unwrap();
                    if let Some(sequence) = kernel.output_into(&result_port, &mut output).unwrap() {
                        break sequence;
                    }
                };
                assert_eq!(output.encoded, bytes);
                kernel.complete_output(&result_port, sequence).unwrap();
                assert!(kernel.complete_host_call_bytes(&original, bytes).is_err());
                runtime.cancel().unwrap();
                kernel.cancel().unwrap();
            }
            assert_eq!(runtime.host.synthetic_iso_pending(), 0);
            // Late original TRB cannot publish another completion on the stopped host.
            runtime.host.synthetic_iso_late_event(late);
            assert!(runtime.isochronous_poll().is_err());
            assert!(kernel.complete_host_call_bytes(&original, b"late").is_err());
        });
        assert_eq!(
            allocations, 0,
            "native terminal, Source validation and Kernel delivery must not allocate"
        );
        drop(runtime);
        drop(storage);
    });
}
#[test]
fn actual_short_event_flows_through_native_encoder_and_checked_source() {
    exercise(13, false, false);
}
#[test]
fn actual_missed_event_flows_through_native_encoder_and_checked_source() {
    exercise(23, false, false);
}
#[test]
fn actual_underrun_event_flows_through_native_encoder_and_checked_source() {
    exercise(14, false, false);
}
#[test]
fn failed_stop_retains_original_dma_and_kernel_token_until_acknowledgement() {
    exercise(14, true, false);
}
#[test]
fn mid_request_cancel_rejects_original_token_and_stopped_poll() {
    exercise(14, false, true);
}
