use super::*;
fn schema() -> StructuredInfoType {
    let package = conduitos::protocol_source::ProtocolSourcePackage {
        schema: conduitos::protocol_source::PACKAGE_SCHEMA.into(),
        source: "type Packet = {\n code: U8\n}\nplot identity (\n >> input: Packet...|\n output: Packet...| >>\n) = (.)".into(),
        specializations: vec![],
    };
    PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "identity")
        .unwrap()
        .input_schema(&PortId::from("input"))
        .unwrap()
}
#[test]
fn encoded_input_round_trips_through_the_core_and_rejects_wrong_fields_or_scalar_widths() {
    let schema = schema();
    let value = encode(&schema, &serde_json::json!({"code":[7]}), 0, &mut 4096).unwrap();
    let bytes = value.canonical_bytes().unwrap();
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&bytes).unwrap(),
        value
    );
    assert_eq!(value.value_type(), &schema);
    for json in [
        serde_json::json!({}),
        serde_json::json!({"code":[7],"extra":[1]}),
        serde_json::json!({"code":[7,8]}),
        serde_json::json!({"code":[256]}),
    ] {
        assert!(encode(&schema, &json, 0, &mut 4096).is_err());
    }
    assert!(encode(&schema, &serde_json::json!({"code":[7]}), 32, &mut 4096).is_err());
    assert!(encode(&schema, &serde_json::json!({"code":[7]}), 0, &mut 0).is_err());
}
