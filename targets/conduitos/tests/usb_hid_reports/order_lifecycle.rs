//! Ordering lifecycle admission through the production pure-protocol kernel.
use conduitos::protocol_source::{
    PreparedProtocolSource, ProtocolSourcePackage, ProtocolSpecializationRequest,
    ProtocolValueReference,
};

pub(super) fn package() -> ProtocolSourcePackage {
    let lifecycle = include_str!("../../plots/usb/hid-keyboard-order-lifecycle.conduit");
    let (header, body) = lifecycle.split_once("\n\n").unwrap();
    let source = format!(
        "{header}\n{}\n{}\n{body}",
        super::common::SOURCE,
        include_str!("../../plots/usb/hid-keyboard-order.conduit"),
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
            ProtocolSpecializationRequest::Merge {
                value: value("UsbKeyboardOrderSession"),
            },
        ],
    )
    .unwrap()
}

#[test]
fn ordering_lifecycle_has_exact_admitted_state_and_feedback_in_the_existing_kernel() {
    let package = package();
    PreparedProtocolSource::prepare(package.clone())
        .unwrap()
        .expand("usb-hid-keyboard-order-lifecycle")
        .unwrap();
    let (_, run) =
        super::state_kernel::prepared_package(&package, "usb-hid-keyboard-order-lifecycle");
    let boundary = &run.kernel().definition().boundary;
    assert_eq!(boundary.input_fronts.len(), 2);
    assert_eq!(boundary.output_fronts.len(), 2);
}
