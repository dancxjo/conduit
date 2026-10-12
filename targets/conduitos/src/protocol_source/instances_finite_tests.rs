//! Distinct exact finite tuple families remain independently selected.
use crate::protocol_source::{
    PreparedProtocolEntry, ProtocolSourcePackage, ProtocolSpecializationRequest,
    ProtocolValueReference,
};

#[test]
fn two_exact_finite_pairs_prepare_without_overwriting_the_family() {
    let source = "type FirstValue = {\n ready: Boolean\n}\ntype SecondValue = {\n count: U64\n}\nplot first (\n >> left: FirstValue...| <= 4096B\n >> right: FirstValue...| <= 4096B\n paired: FirstPair...| >>\n) {\n join: FirstJoin\n left >> join.left\n right >> join.right\n join.paired >> paired\n}\nplot second (\n >> left: FirstValue...| <= 4096B\n >> right: SecondValue...| <= 4096B\n paired: SecondPair...| >>\n) {\n join: SecondJoin\n left >> join.left\n right >> join.right\n join.paired >> paired\n}";
    let reference = |name: &str, maximum_bytes| ProtocolValueReference {
        type_name: name.into(),
        maximum_bytes,
    };
    let requests = [
        ProtocolSpecializationRequest::FiniteZipInstance {
            alias: "FirstJoin".into(),
            paired_alias: "FirstPair".into(),
            left: reference("FirstValue", 4096),
            right: reference("FirstValue", 4096),
        },
        ProtocolSpecializationRequest::FiniteZipInstance {
            alias: "SecondJoin".into(),
            paired_alias: "SecondPair".into(),
            left: reference("FirstValue", 4096),
            right: reference("SecondValue", 4096),
        },
    ];
    let package = ProtocolSourcePackage::compile(source.into(), &requests).unwrap();
    let bytes = serde_json::to_vec(&package).unwrap();
    let first = PreparedProtocolEntry::prepare(&bytes, "first").unwrap();
    let second = PreparedProtocolEntry::prepare(&bytes, "second").unwrap();
    let kind = |entry: &PreparedProtocolEntry| {
        entry
            .expanded()
            .expanded
            .gears
            .iter()
            .find(|gear| gear.kind_id.as_str().starts_with("flow/zip/finite/typed-"))
            .unwrap()
            .kind_id
            .clone()
    };
    assert_ne!(kind(&first), kind(&second));
    assert!(!kind(&first).as_str().starts_with("flow/zip/feedback/"));
    let duplicate = [
        requests[0].clone(),
        requests[1].clone(),
        ProtocolSpecializationRequest::FiniteZipInstance {
            alias: "DuplicateJoin".into(),
            paired_alias: "DuplicatePair".into(),
            left: reference("FirstValue", 4096),
            right: reference("FirstValue", 4096),
        },
    ];
    let duplicate_package = ProtocolSourcePackage::compile(source.into(), &duplicate).unwrap();
    let duplicate_bytes = serde_json::to_vec(&duplicate_package).unwrap();
    assert!(PreparedProtocolEntry::prepare(&duplicate_bytes, "first").is_err());
}
