//! Below-physics tests of finite terminal publication and native stop quarantine.
//! These stage software terminals explicitly; no endpoint lease/device execution
//! or kernel-token admission is claimed by this fixture.
use super::*;
use crate::arch::xhci_mechanism::{synthetic_stop_dcbaa, with_synthetic_stop_controller};

fn with_host(code: u8, test: impl FnOnce(&mut UsbNativeHost<'_>)) {
    with_synthetic_stop_controller(1, code, 0, |controller| {
        let device =
            super::super::descriptor::device_from_descriptor(1, 1, 1, 64, &[0; 18]).unwrap();
        let dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            (*dma).owner_slot = 1;
            (*dma).owner_root_port = 1;
            (*dma).owner_epoch = 1;
        }
        let mut host = unsafe { UsbNativeHost::new(controller, device, Some) }.unwrap();
        test(&mut host);
        // Explicit teardown of fixture memory, never failed-stop retirement.
        unsafe {
            (*dma).owner_slot = 0;
        }
    });
}

#[test]
fn terminal_storage_is_reserved_only_for_duplex_before_play() {
    with_host(1, |host| {
        assert!(host.bulk_terminals.is_none());
        assert!(matches!(
            host.begin_independent_read(UsbNativeCallIndex(0), RequestId(0), &[]),
            Err(UsbNativeHostRefusal::NotReady)
        ));
        host.prepare_independent().unwrap();
        let address = host.bulk_terminals.as_ref().unwrap().as_ptr();
        assert!(host.prepare_independent().is_err());
        assert!(host.poll_independent().unwrap().is_none());
        assert_eq!(address, host.bulk_terminals.as_ref().unwrap().as_ptr());
        host.playing = true;
        assert!(host.prepare_independent().is_err());
    });
}

#[test]
fn staged_pair_cannot_publish_before_shared_retirement_and_publishes_each_once() {
    with_host(1, |host| {
        host.prepare_independent().unwrap();
        for (direction, slot) in host.bulk_terminals.as_mut().unwrap().iter_mut().enumerate() {
            slot.index = Some(UsbNativeCallIndex(direction + 1));
            slot.bytes[0] = direction as u8;
            slot.length = 1;
        }
        assert!(matches!(
            host.poll_independent(),
            Err(UsbNativeHostRefusal::NotReady)
        ));
        assert!(
            host.bulk_terminals
                .as_ref()
                .unwrap()
                .iter()
                .all(|slot| slot.index.is_some())
        );
        host.bulk_terminals_ready = true; // explicit simulated software retirement
        let (index, bytes) = host.poll_independent().unwrap().unwrap();
        assert_eq!(index, UsbNativeCallIndex(1));
        assert_eq!(bytes, &[0]);
        let (index, bytes) = host.poll_independent().unwrap().unwrap();
        assert_eq!(index, UsbNativeCallIndex(2));
        assert_eq!(bytes, &[1]);
        assert!(host.poll_independent().unwrap().is_none());
    });
}

#[test]
fn refused_original_stop_keeps_staged_pair_and_root_marker_quarantined() {
    with_host(6, |host| {
        host.prepare_independent().unwrap();
        for (direction, slot) in host.bulk_terminals.as_mut().unwrap().iter_mut().enumerate() {
            slot.index = Some(UsbNativeCallIndex(direction + 1));
            slot.length = 1;
        }
        assert!(host.cancel().is_err());
        assert!(!host.stop_acknowledged && !host.stopped);
        assert!(host.cancellation_requested);
        assert!(matches!(
            host.poll_independent(),
            Err(UsbNativeHostRefusal::NotReady)
        ));
        assert!(
            host.bulk_terminals
                .as_ref()
                .unwrap()
                .iter()
                .all(|slot| slot.index.is_some())
        );
        let dma = super::super::dma::dma_pointer(super::super::dma::UsbDmaSlot::PRIMARY);
        unsafe {
            assert_eq!((*dma).owner_slot, 1);
            assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
        }
    });
}
