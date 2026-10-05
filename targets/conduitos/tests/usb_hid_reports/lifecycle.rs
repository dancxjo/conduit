use conduitos::protocol_source::{
    PreparedProtocolSource, ProtocolSourcePackage, ProtocolSpecializationRequest,
    ProtocolValueReference,
};

fn value(name: &str) -> ProtocolValueReference {
    ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes: 4096,
    }
}

#[test]
fn keyboard_lifecycle_checks_with_exact_retained_state_zip_and_merge_types() {
    let lifecycle = include_str!("../../plots/usb/hid-keyboard-lifecycle.conduit");
    let (header, body) = lifecycle.split_once("\n\n").unwrap();
    let source = format!(
        "{header}\n{}\n{}\n{body}",
        super::common::SOURCE,
        include_str!("../../plots/usb/hid-keyboard-state.conduit")
    );
    let package = ProtocolSourcePackage::compile(
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
            ProtocolSpecializationRequest::Merge {
                value: value("UsbKeyboardState"),
            },
        ],
    )
    .unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    let expanded = prepared.expand("usb-hid-keyboard-lifecycle").unwrap();
    assert!(!expanded.expanded.gears.is_empty());
}
