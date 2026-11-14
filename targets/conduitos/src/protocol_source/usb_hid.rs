//! Inert shared HID Source packaging for native preparation and conformance.
use super::{
    ProtocolSourcePackage, ProtocolSourceRefusal, ProtocolSpecializationRequest,
    ProtocolValueReference,
};
use alloc::format;

/// Compact motion observations retain a two-report ordering window without
/// multiplying the raw endpoint payload bound. No native effects or grants.
pub fn usb_hid_mouse_order_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    let lifecycle = include_str!("../../plots/usb/hid-mouse-order-lifecycle.conduit");
    let (header, body) =
        lifecycle
            .split_once("\n\n")
            .ok_or(ProtocolSourceRefusal::Specialization(
                "HID mouse ordering imports",
            ))?;
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        format!(
            "{header}\n{}\n{}\n{}\n{}\n{body}",
            include_str!("../../plots/usb/hid-reports.conduit"),
            include_str!("../../plots/usb/hid-mouse-order.conduit"),
            include_str!("../../plots/usb/hid-mouse-pointer.conduit"),
            include_str!("../../plots/usb/hid-absolute-pointer.conduit"),
        ),
        &[
            ProtocolSpecializationRequest::SeededUntil {
                value: value("UsbMouseOrderSession"),
            },
            ProtocolSpecializationRequest::FeedbackZip {
                left: value("UsbMouseOrderSession"),
                right: value("UsbMouseOrderCommand"),
            },
            ProtocolSpecializationRequest::Concat {
                value: value("UsbMouseOrderCommand"),
            },
            ProtocolSpecializationRequest::Merge {
                value: value("UsbMouseOrderMessage"),
            },
        ],
    )
}

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

/// Absolute report interpretation feeds the existing mouse pointer contract.
/// Layouts are checked descriptor facts; endpoint possession is separate.
pub fn usb_hid_absolute_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    let endpoint = include_str!("../../plots/usb/hid-absolute-endpoint.conduit");
    let (header, body) =
        endpoint
            .split_once("\n\n")
            .ok_or(ProtocolSourceRefusal::Specialization(
                "absolute HID imports",
            ))?;
    let value = |name: &str| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    };
    ProtocolSourcePackage::compile(
        format!(
            "{header}\n{}\n{}\n{}\n{}\n{}\n{}\n{body}\n{}",
            include_str!("../../plots/usb/hid-reports.conduit"),
            include_str!("../../plots/usb/hid-mouse-order.conduit"),
            include_str!("../../plots/usb/hid-mouse-pointer.conduit"),
            include_str!("../../plots/usb/hid-absolute-pointer.conduit"),
            include_str!("../../plots/usb/hid-absolute-layout-types.conduit"),
            include_str!("../../plots/usb/hid-absolute-report.conduit"),
            include_str!("../../plots/usb/hid-absolute-device.conduit")
        ),
        &[
            ProtocolSpecializationRequest::Zip {
                left: value("UsbAbsoluteReadContext"),
                right: value("UsbEndpointReadResult"),
            },
            ProtocolSpecializationRequest::FeedbackZipInstance {
                alias: "UsbAbsoluteInitialJoin".into(),
                paired_alias: "UsbAbsoluteInitialPair".into(),
                left: value("UsbAbsoluteDeviceParameters"),
                right: value("UsbMousePointerState"),
            },
        ],
    )
}

/// Descriptor parsing reuses one finite state cell and the checked pure stages.
pub fn usb_hid_absolute_descriptor_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal>
{
    ProtocolSourcePackage::compile(
        alloc::format!(
            "{}{}",
            include_str!("../../plots/usb/hid-absolute-layout-types.conduit"),
            include_str!("../../plots/usb/hid-absolute-layout.conduit")
        ),
        &[ProtocolSpecializationRequest::SeededUntil {
            value: ProtocolValueReference {
                type_name: "UsbAbsoluteHidLayoutState".into(),
                maximum_bytes: 4096,
            },
        }],
    )
}

/// Configuration walking and HID endpoint selection retain wire facts only.
pub fn usb_hid_configuration_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    ProtocolSourcePackage::compile(
        alloc::format!(
            "{}\n{}",
            include_str!("../../plots/usb/configuration-walk.conduit"),
            include_str!("../../plots/usb/hid-input-profile.conduit")
        ),
        &[],
    )
}

/// Complete configuration geometry through one bounded forward-only stage.
pub fn usb_configuration_decode_package() -> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    ProtocolSourcePackage::compile(
        alloc::format!(
            "{}\n{}",
            include_str!("../../plots/usb/configuration-walk.conduit"),
            include_str!("../../plots/usb/configuration-decode.conduit")
        ),
        &[],
    )
}
/// The same unsigned-contact policy with a reusable pass and Source iteration.
pub fn usb_hid_absolute_descriptor_stages_package()
-> Result<ProtocolSourcePackage, ProtocolSourceRefusal> {
    ProtocolSourcePackage::compile(
        alloc::format!(
            "{}\n{}\n{}",
            include_str!("../../plots/usb/hid-absolute-layout-types.conduit"),
            include_str!("../../plots/usb/hid-absolute-layout.conduit"),
            include_str!("../../plots/usb/hid-absolute-layout-stages.conduit")
        ),
        &[
            ProtocolSpecializationRequest::MergeInstance {
                alias: "UsbHidLayoutWorkMerge".into(),
                value: ProtocolValueReference {
                    type_name: "UsbAbsoluteHidLayoutState".into(),
                    maximum_bytes: 4096,
                },
            },
            ProtocolSpecializationRequest::SeededUntil {
                value: ProtocolValueReference {
                    type_name: "UsbAbsoluteHidLayoutState".into(),
                    maximum_bytes: 4096,
                },
            },
        ],
    )
}
