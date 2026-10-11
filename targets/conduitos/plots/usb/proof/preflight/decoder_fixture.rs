//! Validator-only synthetic carrier; never a native acceptance observation.
use conduit_core::*;
fn leaf(kind: &str, bytes: Vec<u8>) -> StructuredInfoValue {
    StructuredInfoValue::leaf(StructuredInfoType::leaf(kind_id(kind)).unwrap(), bytes).unwrap()
}
fn u(value: u64) -> StructuredInfoValue {
    leaf("value/u64", value.to_le_bytes().to_vec())
}
fn record(name: &str, fields: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    let ty = StructuredInfoType::record(
        kind_id(name),
        fields
            .iter()
            .map(|(n, v)| StructuredFieldType::new(*n, v.value_type().clone()).unwrap())
            .collect(),
    )
    .unwrap();
    StructuredInfoValue::record(
        ty,
        fields
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn variant(name: &str, tag: &str, value: StructuredInfoValue) -> StructuredInfoValue {
    let ty = StructuredInfoType::variant(
        kind_id(name),
        vec![StructuredVariantCase::new(tag, value.value_type().clone()).unwrap()],
    )
    .unwrap();
    StructuredInfoValue::variant(ty, tag, value).unwrap()
}
fn main() {
    let input = StructuredInfoValue::sequence(
        StructuredInfoType::sequence(StructuredInfoType::leaf(kind_id("value/u8")).unwrap(), 256)
            .unwrap(),
        vec![leaf("value/u8", vec![9]), leaf("value/u8", vec![2])],
    )
    .unwrap();
    let completed = record(
        "fixture/completion",
        vec![
            ("input", input),
            ("short", leaf("value/bool", vec![1])),
            (
                "transferred",
                leaf("value/u16", 2u16.to_le_bytes().to_vec()),
            ),
        ],
    );
    let geometry = record(
        "fixture/geometry",
        vec![
            ("address", u(2)),
            ("epoch", u(1)),
            ("interval", u(0)),
            ("packet", u(64)),
            ("transfer", u(2)),
        ],
    );
    let carrier = record(
        "fixture/discovery",
        vec![
            ("attachment", u(1)),
            (
                "configuration",
                variant("fixture/control", "completed", completed),
            ),
            (
                "read",
                variant("fixture/read", "absent", leaf("value/empty", vec![])),
            ),
            ("write", variant("fixture/write", "selected", geometry)),
        ],
    );
    let bytes = carrier.canonical_bytes().unwrap();
    println!(
        "CONDUIT_USB_PREFLIGHT_OBSERVATION kind=fixture/discovery bytes={}",
        bytes.len()
    );
    println!(
        "{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
}
