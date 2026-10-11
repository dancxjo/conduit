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
