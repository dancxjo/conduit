//! Aggregate Host tests against synthetic MMIO and actual native event-ring code.
//! No QEMU/device execution or installed capability-table coverage is claimed.
use super::*;
use crate::arch::xhci_mechanism::{
    synthetic_late_original_stop_ack, synthetic_stop_dcbaa, with_synthetic_stop_controller,
};

fn identity(address: u64) -> Option<u64> {
    Some(address)
}

fn check_stop(slot: u8, code: u8, pointer_delta: u64, accepted: bool) {
    with_synthetic_stop_controller(slot, code, pointer_delta, |controller| {
        let device =
            super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        let dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        // SAFETY: serialized fixture owns this original attachment and its DMA.
        unsafe {
            (*dma).owner_slot = 1;
            (*dma).owner_root_port = 1;
            (*dma).owner_epoch = 1;
            (*dma).device_context[0] = 0x5a;
        }
        // SAFETY: no other controller/event consumer or attachment exists here.
        let mut host = unsafe { UsbNativeHost::new(controller, device, identity) }.unwrap();
        // Keep aggregate uncertain state visible until actual stop acknowledgement.
        host.uncertain_control = true;
        let result = host.cancel();
        assert_eq!(result.is_ok(), accepted);
        assert!(host.cancellation_requested);
        assert!(host.possessions_revoked);
        assert_eq!(host.stop_acknowledged, accepted);
        assert_eq!(host.stopped, accepted);
        assert_eq!(host.uncertain_control, !accepted);
        assert!(matches!(
            host.idle_transport(),
            Err(UsbNativeHostRefusal::NotReady)
        ));
        assert!(matches!(
            host.preparing(NodeId(0)),
            Err(UsbNativeHostRefusal::NotReady)
        ));
        // SAFETY: fixture owns this Host. Cancellation must reject before
        // attempting to resolve any endpoint, authority or DMA binding.
        let iso_begin = unsafe {
            host.begin_isochronous(
                1,
                crate::usb_base::isochronous_schedule::IsochronousFrame::new(1).unwrap(),
                &[0],
            )
        };
        assert!(matches!(iso_begin, Err(UsbNativeHostRefusal::NotReady)));
        // SAFETY: the fixture still owns both global DMA regions.
        unsafe {
            assert_eq!((*dma).owner_slot, if accepted { 0 } else { 1 });
            assert_eq!((*dma).device_context[0], 0x5a);
            assert_eq!(
                synthetic_stop_dcbaa(),
                if accepted { 0 } else { 0xfeed0000 }
            );
        }
        // Repeated successful cancellation consumes no duplicate completion.
        if accepted {
            assert!(host.cancel().is_ok());
        }
        if !accepted {
            // A delayed acknowledgement for the original command must never be
            // accepted as acknowledgement of a different retry command.
            unsafe { synthetic_late_original_stop_ack(&host.controller) };
            assert!(host.cancel().is_err());
            assert!(!host.stop_acknowledged);
            assert!(!host.stopped);
            unsafe {
                assert_eq!((*dma).owner_slot, 1);
                assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
            }
        }
        // Explicitly tear down test memory, without claiming failed hardware stop.
        unsafe { (*dma).owner_slot = 0 };
    });
}

#[test]
fn original_successful_stop_retires_aggregate_marker_once() {
    check_stop(1, 1, 0, true);
}

#[test]
fn foreign_slot_success_cannot_retire_original_dma() {
    check_stop(2, 1, 0, false);
}

#[test]
fn failed_original_stop_keeps_aggregate_quarantined() {
    check_stop(1, 6, 0, false);
}

#[test]
fn foreign_command_pointer_cannot_retire_original_dma() {
    check_stop(1, 1, 16, false);
}

// Reuse the existing proof root's explicit issuance; no fabricated table owner.
use crate::usb_base::{
    control_contract::{CONTROL_CALL, CONTROL_MAXIMUM_BYTES, ControlContract},
    control_factory::CONTROL_IMPLEMENTATION,
    control_proof_plan::{self as planning, ControlProofSubject},
};
use conduit_core::*;
#[cfg(not(all(target_arch = "x86_64", feature = "scripted-keyboard-proof")))]
#[path = "../usb_control_kernel_proof/possession.rs"]
pub(in crate::arch::usb_xhci) mod possession;
#[cfg(all(target_arch = "x86_64", feature = "scripted-keyboard-proof"))]
pub(in crate::arch::usb_xhci) use super::super::kernel_proof::possession;

fn control_input(contract: &ControlContract) -> alloc::vec::Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        panic!("control request record")
    };
    let ty = |name| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value_type()
            .clone()
    };
    StructuredInfoValue::record(
        contract.request_type().clone(),
        alloc::vec![
            StructuredFieldValue::new(
                "setup",
                StructuredInfoValue::leaf(ty("setup"), alloc::vec![128, 6, 0, 1, 0, 0, 64, 0],)
                    .unwrap()
            )
            .unwrap(),
            StructuredFieldValue::new(
                "output",
                StructuredInfoValue::sequence(ty("output"), alloc::vec![],).unwrap()
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn check_bound_control_stop(slot: u8, code: u8, accepted: bool) {
    with_synthetic_stop_controller(1, 1, 0, |controller| {
        let contract = ControlContract::prepare().unwrap();
        let plan = planning::plan(
            &contract,
            &ControlProofSubject {
                host_id: "host/stop-fixture",
                boot_id: "boot/stop-fixture",
                controller_base_id: "controller/stop-fixture",
                device_instance_id: "device/stop-fixture",
                root_port: 1,
                slot: 1,
                attachment_epoch: 1,
            },
        )
        .unwrap();
        let (table, handle, claim) = possession::issue(&plan).unwrap();
        let fragment = &plan.fragments[0];
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let device =
            super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        let dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            (*dma).owner_slot = 1;
            (*dma).owner_root_port = 1;
            (*dma).owner_epoch = 1;
            (*dma).control_cursor = crate::usb_base::control_ring::ControlRingCursor::new();
        }
        let mut host = unsafe { UsbNativeHost::new(controller, device, identity) }.unwrap();
        let index = unsafe {
            host.add_control(
                table,
                handle,
                claim,
                UsbControlSelection {
                    contract: &contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &fragment.placements[0].placement_id,
                },
            )
        }
        .unwrap();
        // The synthetic command completion is foreign to this real control TD.
        // Its refused invocation keeps the admitted opaque lease and DMA live.
        assert!(
            host.control_invoke(index, RequestId(0), &control_input(&contract))
                .is_err()
        );
        let Transfer::Control(transfer) = &host.calls[index.0].transfer else {
            panic!("control")
        };
        assert!(transfer.retains_native_submission());
        unsafe {
            crate::arch::xhci_mechanism::synthetic_stop_ack_event(&host.controller, slot, code)
        };
        assert_eq!(host.cancel().is_ok(), accepted);
        let Transfer::Control(transfer) = &host.calls[index.0].transfer else {
            panic!("control")
        };
        assert_eq!(transfer.retains_native_submission(), !accepted);
        assert_eq!(host.stopped, accepted);
        assert!(host.possessions_revoked);
        unsafe {
            assert_eq!((*dma).owner_slot, if accepted { 0 } else { 1 });
            assert_eq!(
                synthetic_stop_dcbaa(),
                if accepted { 0 } else { 0xfeed0000 }
            );
            // Test-only teardown, never a claim of acknowledged failed stop.
            (*dma).owner_slot = 0;
        }
    });
}

#[test]
fn bound_control_lease_retires_only_after_original_stop_ack() {
    check_bound_control_stop(1, 1, true);
}
#[test]
fn bound_control_foreign_stop_keeps_revoked_lease_and_dma() {
    check_bound_control_stop(2, 1, false);
}
#[test]
fn bound_control_failed_stop_keeps_revoked_lease_and_dma() {
    check_bound_control_stop(1, 6, false);
}
