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
        "{endpoint_header}\n{lifecycle_header}\n{}\n{}\n{lifecycle_body}\n{endpoint_body}",
        include_str!("../../plots/usb/hid-reports.conduit"),
        include_str!("../../plots/usb/hid-keyboard-state.conduit")
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
