use super::*;
fn request() -> serde_json::Value {
    serde_json::json!({"schema": REQUEST_SCHEMA, "entry": "usb-control", "package_sha256": "0".repeat(64),
        "administrator_approval": true, "firmware_handoff": true, "electrical_attachment_approved": true,
        "root_port": 1, "attachment_epoch": 7, "maximum_control_operations": 64,
        "maximum_steps": 10000, "inputs": [{"port":"request","canonical_bytes":[1]}]})
}
fn decode(value: &serde_json::Value) -> Result<UsbProtocolBootRequest, ProtocolBootRequestRefusal> {
    UsbProtocolBootRequest::decode(&serde_json::to_vec(value).unwrap())
}
#[test]
fn accepted_request_binds_exact_package_separately_from_flags() {
    let mut value = request();
    value["package_sha256"] = alloc::format!("{:X}", Sha256::digest(b"checked-source")).into();
    let decoded = decode(&value).unwrap();
    assert_eq!(decoded.root_port, 1);
    assert_eq!(decoded.attachment_epoch, 7);
    assert_eq!(decoded.maximum_control_operations, 64);
    decoded.bind_package(b"checked-source").unwrap();
    assert_eq!(
        decoded.bind_package(b"checked-source\n"),
        Err(ProtocolBootRequestRefusal::SourceBinding)
    );
    for flag in [
        "administrator_approval",
        "firmware_handoff",
        "electrical_attachment_approved",
    ] {
        let mut invalid = value.clone();
        invalid[flag] = false.into();
        assert!(matches!(
            decode(&invalid),
            Err(ProtocolBootRequestRefusal::Approval)
        ));
    }
}
#[test]
fn zero_and_excessive_work_or_attachment_bounds_refuse() {
    for (field, invalid) in [
        ("root_port", 0),
        ("attachment_epoch", 0),
        ("maximum_control_operations", 0),
        ("maximum_control_operations", 4097),
        ("maximum_steps", 0),
        ("maximum_steps", 1000001),
    ] {
        let mut value = request();
        value[field] = invalid.into();
        assert!(
            matches!(decode(&value), Err(ProtocolBootRequestRefusal::Bounds)),
            "{field}"
        );
    }
}
#[test]
fn malformed_schema_hash_and_unknown_fields_refuse() {
    assert!(matches!(
        UsbProtocolBootRequest::decode(b"{"),
        Err(ProtocolBootRequestRefusal::Encoding)
    ));
    assert!(matches!(
        UsbProtocolBootRequest::decode(&[]),
        Err(ProtocolBootRequestRefusal::Bounds)
    ));
    assert!(matches!(
        UsbProtocolBootRequest::decode(&alloc::vec![b' '; MAXIMUM_REQUEST_BYTES + 1]),
        Err(ProtocolBootRequestRefusal::Bounds)
    ));
    let mut value = request();
    value["schema"] = "conduit.conduitos/protocol-root-request@1".into();
    assert!(matches!(
        decode(&value),
        Err(ProtocolBootRequestRefusal::Schema)
    ));
    for hash in ["0".repeat(63), "g".repeat(64)] {
        let mut value = request();
        value["package_sha256"] = hash.into();
        assert!(matches!(
            decode(&value),
            Err(ProtocolBootRequestRefusal::Bounds)
        ));
    }
    let mut value = request();
    value["invented_grant"] = true.into();
    assert!(matches!(
        decode(&value),
        Err(ProtocolBootRequestRefusal::Encoding)
    ));
}
#[test]
fn duplicate_empty_oversized_and_excess_inputs_refuse() {
    let cases = [
        serde_json::json!([{"port":"request","canonical_bytes":[1]}, {"port":"request","canonical_bytes":[2]}]),
        serde_json::json!([{"port":"","canonical_bytes":[1]}]),
        serde_json::json!([{"port":"request","canonical_bytes":[]}]),
        serde_json::json!([{"port":"request","canonical_bytes":alloc::vec![0;crate::protocol_source::MAXIMUM_PROTOCOL_FORE_BYTES as usize + 1]}]),
        serde_json::Value::Array((0..17).map(|n| serde_json::json!({"port":alloc::format!("input-{n}"),"canonical_bytes":[1]})).collect()),
    ];
    for inputs in cases {
        let mut value = request();
        value["inputs"] = inputs;
        assert!(matches!(
            decode(&value),
            Err(ProtocolBootRequestRefusal::Bounds)
        ));
    }
    for entry in ["".into(), "e".repeat(129)] {
        let mut value = request();
        value["entry"] = entry.into();
        assert!(matches!(
            decode(&value),
            Err(ProtocolBootRequestRefusal::Bounds)
        ));
    }
}

#[test]
fn endpoint_configuration_is_explicit_bounded_inbound_and_optional() {
    assert!(decode(&request()).unwrap().endpoint_read.is_none());
    let endpoint = serde_json::json!({
        "address":129, "transfer_type":3, "maximum_packet":8, "interval":10,
        "endpoint_epoch":2, "maximum_operations":32
    });
    let mut value = request();
    value["endpoint_read"] = endpoint.clone();
    let configured = decode(&value).unwrap().endpoint_read.unwrap();
    assert_eq!(configured.address, 129);
    assert_eq!(configured.maximum_operations, 32);
    for (field, invalid) in [
        ("address", 1),
        ("address", 128),
        ("address", 145),
        ("transfer_type", 0),
        ("transfer_type", 1),
        ("maximum_packet", 0),
        ("maximum_packet", 1025),
        ("interval", 0),
        ("endpoint_epoch", 0),
        ("maximum_operations", 0),
        ("maximum_operations", 4097),
    ] {
        let mut invalid_request = value.clone();
        invalid_request["endpoint_read"][field] = invalid.into();
        assert!(
            matches!(
                decode(&invalid_request),
                Err(ProtocolBootRequestRefusal::Bounds)
            ),
            "{field}={invalid}"
        );
    }
    value["endpoint_read"]["transfer_type"] = 2.into();
    value["endpoint_read"]["interval"] = 0.into();
    assert!(decode(&value).is_ok());
    value["endpoint_read"]["class_driver"] = "hidden".into();
    assert!(matches!(
        decode(&value),
        Err(ProtocolBootRequestRefusal::Encoding)
    ));
}

#[test]
fn duplex_endpoint_requests_validate_each_direction_and_budget_independently() {
    let mut value = request();
    value["endpoint_read"] = serde_json::json!({"address":129,"transfer_type":2,
        "maximum_packet":64,"interval":0,"endpoint_epoch":2,"maximum_operations":17});
    value["endpoint_write"] = serde_json::json!({"address":2,"transfer_type":2,
        "maximum_packet":512,"interval":0,"endpoint_epoch":9,"maximum_operations":23});
    let decoded = decode(&value).unwrap();
    assert_eq!(decoded.endpoint_read.unwrap().maximum_operations, 17);
    let write = decoded.endpoint_write.unwrap();
    assert_eq!(
        (
            write.address,
            write.endpoint_epoch,
            write.maximum_operations
        ),
        (2, 9, 23)
    );
    for (field, invalid) in [
        ("address", 0),
        ("address", 130),
        ("address", 18),
        ("transfer_type", 1),
        ("maximum_packet", 0),
        ("maximum_packet", 1025),
        ("endpoint_epoch", 0),
        ("maximum_operations", 0),
        ("maximum_operations", 4097),
    ] {
        let mut invalid_request = value.clone();
        invalid_request["endpoint_write"][field] = invalid.into();
        assert!(
            matches!(
                decode(&invalid_request),
                Err(ProtocolBootRequestRefusal::Bounds)
            ),
            "{field}"
        );
    }
    value["endpoint_write"]["transfer_type"] = 3.into();
    assert!(matches!(
        decode(&value),
        Err(ProtocolBootRequestRefusal::Bounds)
    ));
    value["endpoint_write"]["interval"] = 1.into();
    assert!(decode(&value).is_ok());
    value.as_object_mut().unwrap().remove("endpoint_read");
    assert!(matches!(
        decode(&value),
        Err(ProtocolBootRequestRefusal::Bounds)
    ));
}
