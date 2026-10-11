//! Pure bounded frame adapter checks, independent of physical USB/network admission.
use conduitos::{
    ethernet_frame::contract::FRAME_TYPES,
    protocol_source::{PreparedProtocolSource, ProtocolSourcePackage},
};

#[test]
fn ecm_frame_request_expands_and_unproven_receive_narrowing_refuses() {
    // Root catalog integration is deliberately separate. Supply the same checked
    // network definitions directly here; retain ordinary USB contract imports.
    let adapter = include_str!("../plots/usb/cdc-ecm-frames.conduit");
    let imports = adapter
        .lines()
        .filter(|line| line.starts_with("with machine/usb/"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = adapter
        .lines()
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("{imports}\n{FRAME_TYPES}\n{body}\n");
    let package = ProtocolSourcePackage::compile(source, &[]).unwrap();
    let prepared = PreparedProtocolSource::prepare(package).unwrap();
    prepared.expand("cdc-ecm-frame-request").unwrap();
    let error = prepared.expand("cdc-ecm-completed-frame").unwrap_err();
    assert!(format!("{error:?}").contains("CND-FRM-046"));
}
