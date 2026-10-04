use super::*;
use conduit_core::{BOOL_INFO_ID, kind_id};

fn package() -> ProtocolSourcePackage {
    ProtocolSourcePackage {
        schema: PACKAGE_SCHEMA.into(),
        source: "plot identity (\n >> input: Boolean...|\n output: Boolean...| >>\n) = (.)".into(),
        specializations: vec![ProtocolSpecialization::SeededFlow {
            value: ProtocolValue {
                schema: StructuredInfoType::leaf(kind_id(BOOL_INFO_ID)).unwrap(),
                contract: CheckedValueContract::new(kind_id(BOOL_INFO_ID), 1, vec![]).unwrap(),
            },
        }],
    }
}

#[test]
fn canonical_package_prepares_only_pure_offers_and_preserves_the_source() {
    let package = package();
    let encoded = serde_json::to_vec(&package).unwrap();
    let decoded = ProtocolSourcePackage::decode(&encoded).unwrap();
    assert_eq!(decoded.source, package.source);
    let prepared = PreparedProtocolSource::prepare(decoded).unwrap();
    assert_eq!(prepared.checked.plots[0].name, "identity");
    assert_eq!(prepared.capabilities.len(), 1);
    let offer = &prepared.capabilities[0];
    assert!(offer.resource_requirements.is_empty());
    assert!(offer.authority_requirements.is_empty());
    assert!(offer.host_calls.is_empty());
}

#[test]
fn malformed_encoding_and_injected_resource_fields_are_distinct_from_source_errors() {
    let mut encoded = serde_json::to_value(package()).unwrap();
    encoded["authority"] = serde_json::json!({"resource": "ambient/controller"});
    assert!(matches!(
        ProtocolSourcePackage::decode(&serde_json::to_vec(&encoded).unwrap()),
        Err(ProtocolSourceRefusal::Encoding)
    ));
    let mut encoded = serde_json::to_value(package()).unwrap();
    encoded["specializations"][0]["value"]["schema"] = serde_json::json!([255]);
    assert!(matches!(
        ProtocolSourcePackage::decode(&serde_json::to_vec(&encoded).unwrap()),
        Err(ProtocolSourceRefusal::Encoding)
    ));
    let mut unsupported = package();
    unsupported.schema = "conduit.conduitos/protocol-source@99".into();
    assert!(matches!(
        ProtocolSourcePackage::decode(&serde_json::to_vec(&unsupported).unwrap()),
        Err(ProtocolSourceRefusal::UnsupportedSchema)
    ));
    let mut invalid_source = package();
    invalid_source.source = "plot !".into();
    assert!(matches!(
        PreparedProtocolSource::prepare(invalid_source),
        Err(ProtocolSourceRefusal::Source(_))
    ));
}

#[test]
fn package_bounds_and_unavailable_specializations_refuse_before_any_play() {
    assert!(matches!(
        ProtocolSourcePackage::decode(&vec![0; MAXIMUM_PACKAGE_BYTES + 1]),
        Err(ProtocolSourceRefusal::Bounds)
    ));
    let mut oversized = package();
    oversized.source = "x".repeat(MAXIMUM_SOURCE_BYTES + 1);
    assert!(matches!(
        PreparedProtocolSource::prepare(oversized),
        Err(ProtocolSourceRefusal::Bounds)
    ));
    let mut too_many = package();
    too_many.specializations =
        vec![too_many.specializations[0].clone(); MAXIMUM_SPECIALIZATIONS + 1];
    assert!(matches!(
        PreparedProtocolSource::prepare(too_many),
        Err(ProtocolSourceRefusal::Bounds)
    ));
    let mut unavailable = package();
    let ProtocolSpecialization::SeededFlow { value } = &mut unavailable.specializations[0] else {
        unreachable!()
    };
    value.contract.maximum_bytes = 8192;
    assert!(matches!(
        PreparedProtocolSource::prepare(unavailable),
        Err(ProtocolSourceRefusal::Specialization(_))
    ));
}
