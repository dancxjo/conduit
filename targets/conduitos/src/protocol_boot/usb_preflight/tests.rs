//! Exact pure Kernel admission; fixture observations are never native possession.
use super::*;
use crate::protocol_source::ProtocolSourcePackage;
use sha2::{Digest, Sha256};
const CONFIG: &[u8] = &[
    9, 2, 67, 0, 2, 1, 0, 128, 50, 9, 4, 0, 0, 1, 2, 2, 1, 0, 5, 36, 0, 16, 1, 5, 36, 1, 0, 1, 4,
    36, 2, 2, 5, 36, 6, 0, 1, 7, 5, 131, 3, 16, 0, 16, 9, 4, 1, 0, 2, 10, 0, 0, 0, 7, 5, 129, 2,
    64, 0, 0, 7, 5, 2, 2, 64, 0, 0,
];
fn package() -> Vec<u8> {
    let source = alloc::format!(
        "with machine/usb/control/result as UsbControlResult\n{}\nplot class (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)",
        include_str!("../../../plots/usb/cdc-serial-profile.conduit")
    );
    serde_json::to_vec(&ProtocolSourcePackage::compile(source, &[]).unwrap()).unwrap()
}
fn request(bytes: &[u8]) -> UsbProtocolBootRequest {
    UsbProtocolBootRequest::decode(&serde_json::to_vec(&serde_json::json!({
        "schema":super::super::usb_request::REQUEST_SCHEMA, "entry":"class",
        "package_sha256":alloc::format!("{:x}",Sha256::digest(bytes)),
        "administrator_approval":true,"firmware_handoff":true,"electrical_attachment_approved":true,
        "root_port":1,"attachment_epoch":7,"maximum_control_operations":5,"maximum_steps":1000,"inputs":[],
        "preflight":{"entry":"cdc-serial-profile-preflight","maximum_discovery_operations":2,"maximum_steps":4096}
    })).unwrap()).unwrap()
}
#[test]
fn exact_profile_pure_kernel_retires_and_class_package_substitution_refuses() {
    let bytes = package();
    let request = request(&bytes);
    let admitted = accept(
        &bytes,
        &request,
        request.preflight.as_ref().unwrap(),
        UsbConfigurationSnapshot::fixture(CONFIG, 67),
        &"fixture/host".into(),
        &"fixture/boot".into(),
    )
    .unwrap();
    let class = PreparedProtocolEntry::prepare(&bytes, "class").unwrap();
    admitted.validate_class(&class).unwrap();
    let mut substituted = bytes.clone();
    substituted.push(b' ');
    let changed = PreparedProtocolEntry::prepare(&substituted, "class").unwrap();
    assert_eq!(
        admitted.validate_class(&changed),
        Err("protocol-usb-preflight-class-substitution")
    );
    let wrong = PreparedProtocolEntry::prepare(&bytes, "cdc-serial-profile-preflight").unwrap();
    assert_eq!(
        admitted.validate_class(&wrong),
        Err("protocol-usb-preflight-class-substitution")
    );
}
#[test]
fn unsupported_observations_have_no_admitted_class_handoff() {
    let bytes = package();
    let request = request(&bytes);
    let mut wrong = CONFIG.to_vec();
    wrong[11] = 2;
    wrong[35] = 2;
    for observed in [wrong, CONFIG[..66].to_vec(), alloc::vec![0;67]] {
        let result = accept(
            &bytes,
            &request,
            request.preflight.as_ref().unwrap(),
            UsbConfigurationSnapshot::fixture(&observed, 67),
            &"fixture/host".into(),
            &"fixture/boot".into(),
        );
        assert!(matches!(result, Err("protocol-usb-profile-unsupported")));
    }
}
