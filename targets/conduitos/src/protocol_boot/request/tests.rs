use super::*;
fn request() -> serde_json::Value {
    serde_json::json!({"schema": REQUEST_SCHEMA, "entry": "sensor", "package_sha256": "0".repeat(64),
        "administrator_approval": true, "firmware_handoff": true, "electrical_attachment_approved": true,
        "pci_bus": 0, "pci_device": 31, "pci_function": 3, "minimum_address": 118, "maximum_address": 118,
        "maximum_polls": 4096, "maximum_bus_operations": 256, "maximum_clock_operations": 256,
        "clock_lifetime_millis": 10000, "maximum_steps": 10000, "inputs": []})
}
#[test]
fn exact_source_bytes_are_bound_separately_from_administrative_approval() {
    let mut value = request();
    value["package_sha256"] = alloc::format!("{:x}", Sha256::digest(b"source")).into();
    let decoded = ProtocolBootRequest::decode(&serde_json::to_vec(&value).unwrap()).unwrap();
    decoded.bind_package(b"source").unwrap();
    assert_eq!(
        decoded.bind_package(b"source\n"),
        Err(ProtocolBootRequestRefusal::SourceBinding)
    );
    value["administrator_approval"] = false.into();
    assert!(matches!(
        ProtocolBootRequest::decode(&serde_json::to_vec(&value).unwrap()),
        Err(ProtocolBootRequestRefusal::Approval)
    ));
}
#[test]
fn invalid_geometry_work_and_unknown_fields_refuse_without_effects() {
    for (field, invalid) in [
        ("pci_device", 32),
        ("pci_function", 8),
        ("minimum_address", 0),
        ("maximum_polls", 0),
        ("maximum_bus_operations", 0),
        ("maximum_clock_operations", 0),
        ("clock_lifetime_millis", 60001),
        ("maximum_steps", 1000001),
    ] {
        let mut value = request();
        value[field] = invalid.into();
        assert!(
            ProtocolBootRequest::decode(&serde_json::to_vec(&value).unwrap()).is_err(),
            "{field}"
        );
    }
    let mut value = request();
    value["invented_grant"] = true.into();
    assert!(matches!(
        ProtocolBootRequest::decode(&serde_json::to_vec(&value).unwrap()),
        Err(ProtocolBootRequestRefusal::Encoding)
    ));
}
#[test]
fn duplicate_or_oversized_fore_payloads_cannot_widen_prepared_input() {
    let mut value = request();
    value["inputs"] = serde_json::json!([{"port":"begin","canonical_bytes":[1]}, {"port":"begin","canonical_bytes":[2]}]);
    assert!(ProtocolBootRequest::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    value["inputs"] = serde_json::json!([{"port":"begin","canonical_bytes":alloc::vec![0;4097]}]);
    assert!(ProtocolBootRequest::decode(&serde_json::to_vec(&value).unwrap()).is_err());
}
