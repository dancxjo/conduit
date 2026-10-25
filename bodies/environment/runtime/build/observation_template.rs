//! Compile ordinary canonical composition into immutable framing and locations.
//! The runtime still decodes the authored Forms and emits the exact semantic Type.
use conduit_core::{
    self as c, StructuredFieldValue as Field, StructuredInfoType as Type,
    StructuredInfoValue as Value,
};
use std::{fmt::Write, path::Path};
fn ty<'a>(checked: &'a conduit_plot::CheckedSyntaxDocument, name: &str) -> &'a Type {
    &checked
        .native_types
        .iter()
        .find(|ty| ty.name == name)
        .unwrap()
        .value_type
}
fn quantity_range(bytes: &[u8], result: bool, name: &str) -> (usize, usize) {
    let value = c::validate_canonical_structured_value(bytes).unwrap();
    let observation = if result {
        value.variant_payload("observation").unwrap().unwrap()
    } else {
        value
    };
    let leaf = observation
        .record_field(name)
        .unwrap()
        .unwrap()
        .value_node();
    assert_eq!(leaf[0], 0);
    assert_eq!(leaf.len(), 5 + c::QUANTITY_ENCODED_LEN);
    let start = leaf.as_ptr() as usize - bytes.as_ptr() as usize + 5;
    (start, start + c::QUANTITY_ENCODED_LEN)
}
pub fn emit(checked: &conduit_plot::CheckedSyntaxDocument, out: &Path) {
    let observation = ty(checked, "Dht11Observation");
    let c::StructuredInfoTypeShape::Record { fields, .. } = observation.shape() else {
        panic!()
    };
    let field = |name: &str| {
        fields
            .iter()
            .find(|f| f.name() == name)
            .unwrap()
            .value_type()
    };
    let value = Value::record(
        observation.clone(),
        vec![
            Field::new(
                "temperature",
                Value::leaf(
                    field("temperature").clone(),
                    c::Quantity::new(0, c::Unit::Celsius).encode().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
            Field::new(
                "humidity",
                Value::leaf(
                    field("humidity").clone(),
                    c::Quantity::new(0, c::Unit::Percent).encode().to_vec(),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let result_type = ty(checked, "Dht11Result");
    let result = Value::variant(result_type.clone(), "observation", value.clone())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let observation = value.canonical_bytes().unwrap();
    let failure_type = ty(checked, "Dht11Failure");
    let failure = Value::variant(
        failure_type.clone(),
        "short-frame",
        Value::leaf(Type::leaf(c::kind_id("value/u8")).unwrap(), vec![0]).unwrap(),
    )
    .unwrap();
    let unavailable = Value::variant(result_type.clone(), "unavailable", failure)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let parsed = c::validate_canonical_structured_value(&unavailable).unwrap();
    let failure = parsed
        .variant_payload("unavailable")
        .unwrap()
        .unwrap()
        .value_node();
    let prefix = unavailable.len() - failure.len();
    assert_eq!(
        failure.as_ptr() as usize,
        unavailable.as_ptr() as usize + prefix
    );
    let mut source = String::new();
    for (name, bytes, ty, is_result) in [
        (
            "OBSERVATION",
            &observation,
            ty(checked, "Dht11Observation"),
            false,
        ),
        ("RESULT", &result, result_type, true),
    ] {
        let maximum = conduit_plot::maximum_prepared_canonical_value_bytes(ty).unwrap() as usize;
        assert!(bytes.len() <= maximum && maximum <= 4096);
        let t = quantity_range(bytes, is_result, "temperature");
        let h = quantity_range(bytes, is_result, "humidity");
        assert!(t.1 <= h.0 || h.1 <= t.0);
        writeln!(source, "pub const {name}: &[u8] = &{bytes:?};").unwrap();
        writeln!(source, "pub const {name}_MAXIMUM: usize = {maximum};").unwrap();
        writeln!(
            source,
            "pub const {name}_TEMPERATURE: core::ops::Range<usize> = {}..{};",
            t.0, t.1
        )
        .unwrap();
        writeln!(
            source,
            "pub const {name}_HUMIDITY: core::ops::Range<usize> = {}..{};",
            h.0, h.1
        )
        .unwrap();
    }
    writeln!(
        source,
        "pub const UNAVAILABLE_PREFIX: &[u8] = &{:?};",
        &unavailable[..prefix]
    )
    .unwrap();
    writeln!(
        source,
        "pub const FAILURE_TYPE: &[u8] = &{:?};",
        failure_type.canonical_bytes().unwrap()
    )
    .unwrap();
    std::fs::write(out.join("observation-material.rs"), source).unwrap();
}
