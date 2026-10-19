use super::*;

#[test]
fn admitted_limits_do_not_follow_hardware_maxima() {
    assert_eq!(ADMITTED_DEVICE_SLOTS, 3);
    assert_eq!(MAX_PENDING_COMMANDS, 1);
}

#[test]
fn dma_shape_is_fixed_and_aligned() {
    assert_eq!(core::mem::align_of::<DmaStorage>(), 64);
    assert_eq!(core::mem::size_of::<DmaStorage>(), 640);
    assert_eq!(core::mem::offset_of!(DmaStorage, dcbaa) % 64, 0);
    assert_eq!(core::mem::size_of::<[u64; DCBAA_ENTRIES]>(), 32);
    assert_eq!(core::mem::offset_of!(DmaStorage, command_ring) % 64, 0);
    assert_eq!(core::mem::offset_of!(DmaStorage, event_ring) % 64, 0);
    assert_eq!(core::mem::offset_of!(DmaStorage, erst) % 64, 0);
}

#[test]
fn pci_and_bar_failures_are_distinct() {
    assert_ne!(XhciError::Absent, XhciError::WrongClass);
    assert_ne!(XhciError::WrongClass, XhciError::InvalidBar);
    assert_ne!(XhciError::InvalidBar, XhciError::InvalidLayout);
}

#[test]
fn bounded_progress_failures_are_distinct() {
    assert_ne!(XhciError::ResetTimeout, XhciError::StartTimeout);
    assert_ne!(XhciError::CommandRingFull, XhciError::CommandTimeout);
    assert_ne!(XhciError::CommandTimeout, XhciError::UnexpectedCompletion);
}

#[test]
fn unsupported_storage_and_page_shapes_fail_separately() {
    assert_ne!(
        XhciError::UnsupportedPageSize,
        XhciError::ScratchpadsUnsupported
    );
    assert_ne!(
        XhciError::ScratchpadsUnsupported,
        XhciError::DmaAddressInvalid
    );
}

#[test]
fn stale_base_identity_cannot_equal_a_fresh_boot_base() {
    let old = crate::identity::derive_base(&[1; 32], "conduitos/xhci/0000:00:01.0/1b36:000d");
    let fresh = crate::identity::derive_base(&[2; 32], "conduitos/xhci/0000:00:01.0/1b36:000d");
    assert_ne!(old, fresh);
}

#[test]
fn all_refusals_remain_machine_readable() {
    for error in [
        XhciError::Absent,
        XhciError::WrongClass,
        XhciError::InvalidBar,
        XhciError::InvalidLayout,
        XhciError::InsufficientDeviceSlots,
        XhciError::UnsupportedPageSize,
        XhciError::ScratchpadsUnsupported,
        XhciError::ResetTimeout,
        XhciError::StartTimeout,
        XhciError::CommandRingFull,
        XhciError::EventRetentionPressure,
        XhciError::UnexpectedCompletion,
        XhciError::CommandTimeout,
        XhciError::DmaAddressInvalid,
    ] {
        assert!(error.as_str().starts_with("xhci-"));
    }
}

fn endpoint_stop_case(
    transfer: Option<(u8, u8, u64)>,
    ack_slot: u8,
    ack_code: u8,
    pointer_delta: u64,
    accepted: bool,
) {
    with_synthetic_stop_controller(ack_slot, ack_code, pointer_delta, |mut controller| {
        if let Some((slot, endpoint, td)) = transfer {
            // SAFETY: the shared fixture serializes global DMA/event ownership.
            unsafe {
                DMA.event_ring[1] = DMA.event_ring[0];
                DMA.event_ring[0] = [
                    td as u32,
                    (td >> 32) as u32,
                    26 << 24,
                    (32 << 10) | (u32::from(slot) << 24) | (u32::from(endpoint) << 16) | 1,
                ];
            }
        }
        // SAFETY: fixture exclusively owns slot 1, DCI 3, and retained TD 0x4000.
        let result = unsafe { controller.stop_original_endpoint(1, 3, Some(0x4000)) };
        assert_eq!(result.is_ok(), accepted);
        if accepted {
            assert_eq!(result.unwrap(), u8::from(transfer.is_some()));
        }
        // Endpoint stop never clears the device context or disables EP0/slot.
        unsafe {
            assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
            let command = core::ptr::read_volatile(core::ptr::addr_of!(DMA.command_ring[0]));
            assert_eq!((command[3] >> 10) & 63, 15);
            assert_eq!((command[3] >> 16) & 31, 3);
            assert_eq!(command[3] >> 24, 1);
        }
    });
}

#[test]
fn endpoint_stop_ack_preserves_slot_and_drains_only_original_td() {
    endpoint_stop_case(Some((1, 3, 0x4000)), 1, 1, 0, true);
    endpoint_stop_case(None, 1, 1, 0, true);
}

#[test]
fn endpoint_stop_rejects_foreign_queue_or_td_events() {
    endpoint_stop_case(Some((2, 3, 0x4000)), 1, 1, 0, false);
    endpoint_stop_case(Some((1, 4, 0x4000)), 1, 1, 0, false);
    endpoint_stop_case(Some((1, 3, 0x4010)), 1, 1, 0, false);
}

#[test]
fn endpoint_stop_requires_exact_successful_command_ack() {
    endpoint_stop_case(None, 2, 1, 0, false);
    endpoint_stop_case(None, 1, 6, 0, false);
    endpoint_stop_case(None, 1, 1, 16, false);
}

#[test]
fn endpoint_stop_invalid_geometry_publishes_no_command() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        for (slot, dci, td) in [
            (0, 3, None),
            (4, 3, None),
            (1, 1, None),
            (1, 32, None),
            (1, 3, Some(0)),
            (1, 3, Some(0x4001)),
        ] {
            assert_eq!(
                unsafe { controller.stop_original_endpoint(slot, dci, td) },
                Err(XhciError::InvalidLayout)
            );
        }
        assert_eq!(controller.command_enqueue, 0);
    });
}

#[test]
fn endpoint_stop_duplicate_td_does_not_settle_queue() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        unsafe {
            DMA.event_ring[2] = DMA.event_ring[0];
            let td = [0x4000, 0, 26 << 24, (32 << 10) | (1 << 24) | (3 << 16) | 1];
            DMA.event_ring[0] = td;
            DMA.event_ring[1] = td;
        }
        assert_eq!(
            unsafe { controller.stop_original_endpoint(1, 3, Some(0x4000)) },
            Err(XhciError::UnexpectedCompletion)
        );
        // The original acknowledgement cannot acknowledge a later command.
        assert_eq!(
            unsafe { controller.stop_original_endpoint(1, 3, Some(0x4000)) },
            Err(XhciError::UnexpectedCompletion)
        );
        unsafe {
            assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
        }
    });
}

#[test]
fn endpoint_stop_event_budget_is_finite() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        unsafe {
            DMA.event_ring = [[0, 0, 0, (34 << 10) | 1]; EVENT_TRBS];
        }
        assert_eq!(
            unsafe { controller.stop_original_endpoint(1, 3, None) },
            Err(XhciError::UnexpectedCompletion)
        );
        assert_eq!(controller.event_dequeue, 0);
        assert_eq!(controller.event_cycle, 0);
        unsafe {
            assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
        }
    });
}

#[test]
fn endpoint_reset_and_dequeue_require_exact_slot_success() {
    for (slot, code, delta, accepted) in [
        (1, 1, 0, true),
        (2, 1, 0, false),
        (1, 6, 0, false),
        (1, 1, 16, false),
    ] {
        for reset in [true, false] {
            with_synthetic_stop_controller(slot, code, delta, |mut controller| {
                let result = unsafe {
                    if reset {
                        controller.reset_original_halted_endpoint(1, 3)
                    } else {
                        controller.set_original_stopped_endpoint_dequeue(1, 3, 0x4000, 1)
                    }
                };
                assert_eq!(result.is_ok(), accepted);
                unsafe {
                    let command =
                        core::ptr::read_volatile(core::ptr::addr_of!(DMA.command_ring[0]));
                    assert_eq!((command[3] >> 10) & 63, if reset { 14 } else { 16 });
                    assert_eq!((command[3] >> 16) & 31, 3);
                    assert_eq!(command[3] >> 24, 1);
                    assert_eq!(command[0], if reset { 0 } else { 0x4001 });
                    assert_eq!(synthetic_stop_dcbaa(), 0xfeed0000);
                }
            });
        }
    }
}

#[test]
fn endpoint_dequeue_rejects_invalid_position_before_publication() {
    with_synthetic_stop_controller(1, 1, 0, |mut controller| {
        for (address, cycle) in [(0, 1), (0x4001, 1), (0x4000, 2)] {
            assert_eq!(
                unsafe { controller.set_original_stopped_endpoint_dequeue(1, 3, address, cycle) },
                Err(XhciError::InvalidLayout)
            );
        }
        assert_eq!(controller.command_enqueue, 0);
    });
}

#[test]
fn synthetic_microframe_clock_uses_owned_runtime_register_memory() {
    with_synthetic_stop_controller(1, 1, 0, |controller| {
        assert_eq!(
            controller.runtime_interrupter - 0x20,
            controller.operational
        );
        assert_eq!(controller.microframe_index(), 0);
        // SAFETY: the fixture owns the register array for this closure; the
        // checked MFINDEX location is its first aligned u32, not stack padding.
        unsafe { core::ptr::write_volatile(controller.operational as *mut u32, 0x1234) };
        assert_eq!(controller.microframe_index(), 0x1234);
    });
}
