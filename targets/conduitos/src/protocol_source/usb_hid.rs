//! Inert shared HID Source packaging for native preparation and conformance.
use super::{
    ProtocolSourcePackage, ProtocolSourceRefusal, ProtocolSpecializationRequest,
    ProtocolValueReference,
};
use alloc::format;

/// Assemble protocol meaning and generic typed operations only. Native callers
/// must independently select a ready endpoint Back and possess its resources.
pub fn usb_hid_endpoint_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    let lifecycle = include_str!("../../plots/usb/hid-keyboard-lifecycle.conduit");
    let (lifecycle_header, lifecycle_body) =
        lifecycle
            .split_once("\n\n")
            .ok_or(ProtocolSourceRefusal::Specialization(
                "HID lifecycle imports",
            ))?;
    let endpoint = include_str!("../../plots/usb/hid-endpoint.conduit");
    let (endpoint_header, endpoint_body) =
        endpoint
            .split_once("\n\n")
            .ok_or(ProtocolSourceRefusal::Specialization(
                "HID endpoint imports",
            ))?;
    let source = format!(
        "{endpoint_header}\n{lifecycle_header}\n{}\n{}\n{lifecycle_body}\n{endpoint_body}\n{}\n{}",
        include_str!("../../plots/usb/hid-reports.conduit"),
        include_str!("../../plots/usb/hid-keyboard-state.conduit"),
        include_str!("../../plots/usb/hid-keyboard-batches.conduit"),
        include_str!("../../plots/usb/hid-keyboard-order.conduit")
    );
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        source,
        &[
            ProtocolSpecializationRequest::SeededUntil {
                value: value("UsbKeyboardState"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("UsbKeyboardState"),
                right: value("UsbKeyboardCommand"),
            },
            ProtocolSpecializationRequest::Zip {
                left: value("UsbKeyboardDeltaInput"),
                right: value("UsbKeyboardTransitionBatch"),
            },
            ProtocolSpecializationRequest::Concat {
                value: value("UsbKeyboardCommand"),
            },
            ProtocolSpecializationRequest::Merge {
                value: value("UsbKeyboardState"),
            },
        ],
    )
}

/// Assemble wire-order policy with the existing bounded state, feedback and
/// merge operations. This package grants no endpoint or input authority.
pub fn usb_hid_keyboard_order_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    let lifecycle = include_str!("../../plots/usb/hid-keyboard-order-lifecycle.conduit");
    let (header, body) =
        lifecycle
            .split_once("\n\n")
            .ok_or(ProtocolSourceRefusal::Specialization(
                "HID ordering imports",
            ))?;
    let endpoint = include_str!("../../plots/usb/hid-keyboard-ordered-endpoint.conduit");
    let (endpoint_header, endpoint_body) =
        endpoint
            .split_once("\n\n")
            .ok_or(ProtocolSourceRefusal::Specialization(
                "HID ordered endpoint imports",
            ))?;
    let capture = include_str!("../../plots/usb/hid-keyboard-capture-window.conduit");
    let (capture_header, capture_body) = capture
        .split_once("\n\n")
        .ok_or(ProtocolSourceRefusal::Specialization("HID capture imports"))?;
    let source = format!(
        "{header}\n{endpoint_header}\n{capture_header}\n{}\n{}\n{}\n{body}\n{endpoint_body}\n{capture_body}",
        include_str!("../../plots/usb/hid-reports.conduit"),
        include_str!("../../plots/usb/hid-keyboard-order.conduit"),
        include_str!("../../plots/usb/hid-keyboard-state.conduit"),
    );
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        source,
        &[
            ProtocolSpecializationRequest::SeededUntil {
                value: value("UsbKeyboardOrderSession"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("UsbKeyboardOrderSession"),
                right: value("UsbKeyboardOrderCommand"),
            },
            ProtocolSpecializationRequest::Concat {
                value: value("UsbKeyboardOrderCommand"),
            },
            ProtocolSpecializationRequest::Merge {
                value: value("UsbKeyboardOrderMessage"),
            },
        ],
    )
}
