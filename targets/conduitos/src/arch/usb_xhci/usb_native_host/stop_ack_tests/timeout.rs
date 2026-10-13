//! A real selected EP0 submission times out with its original lease and DMA live.
//! Controller observations are deterministic fixture input, not device/QEMU proof.
use super::*;
use crate::arch::xhci_mechanism::{Event, synthetic_next_stop_ack, synthetic_transfer_event};
use crate::usb_base::control_ring::{ControlRingCursor, RingRefusal};

#[test]
fn submitted_ep0_timeout_retains_dma_until_exact_stop_and_rejects_late_success() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        // Consume the fixture's initial unrelated command event BEFORE binding.
        // The submitted control TD therefore waits on an actually empty event
        // ring and reaches the ordinary bounded native timeout, not a foreign
        // completion error or an injected software owner timeout.
        assert_eq!(controller.poll_event().unwrap().event_type, 33);
        assert!(controller.poll_event().is_none());
        let contract = ControlContract::prepare().unwrap();
        let plan = planning::plan(
            &contract,
            &ControlProofSubject {
                host_id: "host/ep0-timeout",
                boot_id: "boot/ep0-timeout",
                controller_base_id: "controller/ep0-timeout",
                device_instance_id: "device/ep0-timeout",
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
            super::super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        let dma =
            super::super::super::dma::dma_pointer(super::super::super::dma::UsbDmaSlot::PRIMARY);
        // SAFETY: serialized fixture owns the original controller/global DMA.
        unsafe {
            (*dma).owner_slot = 1;
            (*dma).owner_root_port = 1;
            (*dma).owner_epoch = 1;
            (*dma).control_cursor = ControlRingCursor::new();
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
        let input = control_input(&contract);
        let position_before = unsafe { (*dma).control_cursor.position() };
        assert!(matches!(
            host.control_invoke(index, RequestId(0), &input),
            Err(UsbNativeHostRefusal::Control(
                UsbControlCallRefusal::Native(UsbError::ControlTimeout)
            ))
        ));
        let Transfer::Control(transfer) = &host.calls[index.0].transfer else {
            panic!("control")
        };
        assert!(transfer.retains_native_submission());
        assert!(transfer.retains_settlement_obligation());
        assert!(host.uncertain_control);
        assert_eq!(
            host.control_output_length, 0,
            "timeout must not publish completed bytes"
        );
        let published = unsafe { (*dma).control_cursor.position() };
        assert_ne!(
            published, position_before,
            "the actual selected TD must be submitted"
        );
        unsafe {
            assert_eq!(
                (*dma).control_cursor.ensure_idle(),
                Err(RingRefusal::Uncertain)
            );
            assert_eq!((*dma).owner_slot, 1);
            assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
        }
        assert!(matches!(
            host.control_invoke(index, RequestId(1), &input),
            Err(UsbNativeHostRefusal::Pending)
        ));
        assert_eq!(unsafe { (*dma).control_cursor.position() }, published);

        // An exact original-slot command with a failed completion grants no stop.
        // This is the first DisableSlot command, so the existing fixture helper
        // names its original command-ring pointer precisely.
        unsafe {
            crate::arch::xhci_mechanism::synthetic_stop_ack_event(&host.controller, 1, 6);
        }
        assert!(host.cancel().is_err());
        assert!(!host.stop_acknowledged && !host.stopped);
        let Transfer::Control(transfer) = &host.calls[index.0].transfer else {
            panic!("control")
        };
        assert!(transfer.retains_native_submission() && transfer.retains_settlement_obligation());
        unsafe {
            assert_eq!((*dma).owner_slot, 1);
            assert_eq!(
                (*dma).control_cursor.ensure_idle(),
                Err(RingRefusal::Uncertain)
            );
            assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
        }
        assert!(host.validate_cancelled_settlement().is_err());
        assert!(matches!(
            host.control_invoke(index, RequestId(1), &input),
            Err(UsbNativeHostRefusal::NotReady)
        ));

        // This second cancellation executes a new original-controller stop;
        // its acknowledgement names that exact next command, not a delayed
        // success for the earlier failed command.
        unsafe {
            synthetic_next_stop_ack(&host.controller);
        }
        host.cancel().unwrap();
        assert!(host.stop_acknowledged && host.stopped);
        let Transfer::Control(transfer) = &host.calls[index.0].transfer else {
            panic!("control")
        };
        assert!(!transfer.retains_native_submission() && !transfer.retains_settlement_obligation());
        host.validate_cancelled_settlement().unwrap();
        unsafe {
            assert_eq!((*dma).owner_slot, 0);
            assert_eq!((*dma).control_cursor.ensure_idle(), Ok(()));
            assert_eq!(synthetic_stop_dcbaa(), 0);
        }
        // The original TD's late successful status event cannot reopen a call
        // or publish any bytes after acknowledged retirement.
        let status_pointer = unsafe { &(*dma).transfer_ring[2] as *const _ as u64 };
        unsafe {
            synthetic_transfer_event(
                &host.controller,
                Event {
                    event_type: 32,
                    completion_code: 1,
                    slot: 1,
                    endpoint: 1,
                    residual: 0,
                    pointer: status_pointer,
                },
                None,
            );
        }
        assert!(matches!(
            host.control_invoke(index, RequestId(0), &input),
            Err(UsbNativeHostRefusal::NotReady)
        ));
        assert_eq!(host.control_output_length, 0);
        host.cancel().unwrap();
    });
}
