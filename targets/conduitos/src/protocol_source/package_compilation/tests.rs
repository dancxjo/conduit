use super::*;

fn source() -> String {
    "type Packet = {\n code: U8\n}\nplot identity (\n >> input: Packet...|\n output: Packet...| >>\n) = (.)".into()
}
fn request(name: &str, maximum_bytes: u32) -> ProtocolSpecializationRequest {
    ProtocolSpecializationRequest::SeededFlow {
        value: ProtocolValueReference {
            type_name: name.into(),
            maximum_bytes,
        },
    }
}
#[test]
fn named_type_preparation_preserves_exact_source_and_checks_the_final_plot_separately() {
    let original = source();
    let package =
        ProtocolSourcePackage::compile(original.clone(), &[request("Packet", 128)]).unwrap();
    assert_eq!(package.source, original);
    let encoded = serde_json::to_vec(&package).unwrap();
    let entry = PreparedProtocolEntry::prepare(&encoded, "identity").unwrap();
    assert_eq!(entry.expanded().expanded.gears.len(), 1);
    let ProtocolSpecialization::SeededFlow { value } = &package.specializations[0] else {
        panic!()
    };
    assert_eq!(value.contract.maximum_bytes, 128);
    assert_eq!(
        value.schema.profile().unwrap().value_kind(),
        &value.contract.value_kind
    );
    let invalid = original.replace("= (.)", "= (missing-kind())");
    let package = ProtocolSourcePackage::compile(invalid, &[request("Packet", 128)]).unwrap();
    assert!(
        PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), "identity").is_err()
    );
}
#[test]
fn unknown_types_invalid_envelopes_and_invalid_declarations_refuse() {
    for request in [
        request("absent", 128),
        request("Packet", 0),
        request("Packet", 4097),
    ] {
        assert!(ProtocolSourcePackage::compile(source(), &[request]).is_err());
    }
    assert!(
        ProtocolSourcePackage::compile("type Packet = Unknown".into(), &[request("Packet", 128)])
            .is_err()
    );
    assert!(
        ProtocolSourcePackage::compile(
            source(),
            &vec![request("Packet", 128); MAXIMUM_SPECIALIZATIONS + 1]
        )
        .is_err()
    );
}
