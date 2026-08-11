use super::*;

#[test]
fn pci_coordinates_are_exact() {
    assert_eq!(pci::address(2, 3, 1, 0x14), 0x8002_1914);
}

#[test]
fn admitted_limits_do_not_follow_hardware_maxima() {
    assert_eq!(ADMITTED_DEVICE_SLOTS, 1);
    assert_eq!(MAX_PENDING_COMMANDS, 1);
}

#[test]
fn dma_shape_is_fixed_and_aligned() {
    assert_eq!(core::mem::align_of::<DmaStorage>(), 64);
    assert_eq!(core::mem::size_of::<DmaStorage>(), 640);
    assert_eq!(core::mem::offset_of!(DmaStorage, dcbaa) % 64, 0);
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
        XhciError::UnsupportedPageSize,
        XhciError::ScratchpadsUnsupported,
        XhciError::ResetTimeout,
        XhciError::StartTimeout,
        XhciError::CommandRingFull,
        XhciError::UnexpectedCompletion,
        XhciError::CommandTimeout,
        XhciError::DmaAddressInvalid,
    ] {
        assert!(error.as_str().starts_with("xhci-"));
    }
}
