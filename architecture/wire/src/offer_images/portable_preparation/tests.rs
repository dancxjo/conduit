use super::*;
use alloc::{format, string::String, vec::Vec};
use serde_json::{json, Value};
fn offer() -> Value {
    json!({"capability_id":"opaque/cap","kind_id":"opaque/kind","kind_contract_revision":"opaque/revision","inputs":[],"outputs":[],"semantic_contract":{"configuration":[],"laws":[]},"execution_profile_id":"opaque/profile","implementation_id":"opaque/implementation","artifact_id":"opaque/artifact","host_calls":[],"resource_requirements":[],"authority_requirements":[],"limits":{"max_active_instances":1,"max_queue_items":1,"max_queue_bytes":1024}})
}
fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
#[test]
fn profile_is_bound_to_exact_bytes_and_ids_are_opaque() {
    let value = offer();
    let json = bytes(&value);
    let profile = prepare(&json).unwrap();
    assert!(profile.matches_json(&json));
    let other = format!("{} ", core::str::from_utf8(&json).unwrap());
    assert!(!profile.matches_json(other.as_bytes()));
    assert!(profile.retained_heap_bytes() > 0);
    assert!(profile.typed_decode_peak_heap_bytes() >= profile.retained_heap_bytes());
    let mut renamed = value;
    renamed["capability_id"] = json!("Quantity/Structured/arbitrary");
    assert!(prepare(&bytes(&renamed)).is_ok());
}
#[test]
fn optional_empty_and_null_publisher_fields_have_explicit_policy() {
    let mut value = offer();
    for (name, v) in [
        ("startup_parameters", json!([])),
        ("shorthand", Value::Null),
        ("state_retention", Value::Null),
        ("realization_properties", json!([])),
    ] {
        value[name] = v;
    }
    assert!(prepare(&bytes(&value)).is_ok());
    for name in ["startup_parameters", "realization_properties"] {
        let mut invalid = value.clone();
        invalid[name] = Value::Null;
        assert!(prepare(&bytes(&invalid)).is_err());
    }
    value["semantic_contract"]["configuration"] = json!([{"key":"program","default_value":{"Text":"{Quantity:opaque,Structured:opaque}"},"rule":{"TextBytes":{"maximum":134217728}}}]);
    let before = prepare(&bytes(&value)).unwrap();
    value["semantic_contract"]["configuration"][0]["rule"]["TextBytes"]["maximum"] = json!(1);
    assert_eq!(
        prepare(&bytes(&value)).unwrap().retained_heap_bytes(),
        before.retained_heap_bytes()
    );
}
#[test]
fn unmodelled_constructors_are_refused_before_owned_decode() {
    for variant in ["Quantity", "Structured", "Unit", "TemperatureDifference"] {
        let mut value = offer();
        let mut default = serde_json::Map::new();
        default.insert(
            variant.into(),
            json!({"canonical_value":[0,1],"source":"bad","profile":"bad"}),
        );
        value["semantic_contract"]["configuration"] =
            json!([{"key":"x","default_value":default,"rule":{"TextBytes":{"maximum":1}}}]);
        assert!(prepare(&bytes(&value)).is_err(), "{variant}");
    }
    for (field, v) in [
        (
            "realization_properties",
            json!([{"profile":"bad","canonical_value":[0]}]),
        ),
        ("state_retention", json!({})),
    ] {
        let mut value = offer();
        value[field] = v;
        assert!(prepare(&bytes(&value)).is_err());
    }
    for extension in ["compute", "content"] {
        let mut value = offer();
        value["resource_requirements"] = json!([{"class_id":"x","units":1,extension:{}}]);
        assert!(prepare(&bytes(&value)).is_err());
    }
    for law in [
        json!({"InitializedState":{"initial":[1,2]}}),
        json!({"ExactRealizationProperty":{}}),
        json!({"FlowFold":{}}),
    ] {
        let mut value = offer();
        value["semantic_contract"]["laws"] = json!([law]);
        assert!(prepare(&bytes(&value)).is_err());
    }
    let mut value = offer();
    value["semantic_contract"]["laws"] = json!([{"ValueContracts":[{"location":{"Input":"x"},"contract":{"value_kind":"opaque","maximum_bytes":1,"constraints":[{"QuantityRange":{"minimum":null}}]}}]}]);
    assert!(prepare(&bytes(&value)).is_err());
}
#[test]
fn map_only_duplicate_unknown_and_numeric_rejections_are_explicit() {
    let valid = String::from_utf8(bytes(&offer())).unwrap();
    for text in [
        format!("{{\"capability_id\":\"again\",{}", &valid[1..]),
        format!("{{\"unknown\":0,{}", &valid[1..]),
        "[]".into(),
        "1e99999999999999999999999999999".into(),
    ] {
        assert!(prepare(text.as_bytes()).is_err());
    }
    let mut value = offer();
    value["limits"]["max_queue_bytes"] = json!(1.0);
    assert!(prepare(&bytes(&value)).is_err());
    let escaped = valid.replace("capability_id", "capability_\\u0069d");
    assert!(prepare(escaped.as_bytes()).is_ok());
}
#[test]
fn native_vector_types_and_overflow_are_not_a_largest_type_multiplier() {
    let mut small = Layout::default();
    small.vector::<u8>(1).unwrap();
    assert_eq!(small.retained, 8);
    let mut large = Layout::default();
    large.vector::<conduit_core::PortDescriptor>(1).unwrap();
    assert_eq!(
        large.retained,
        4 * size_of::<conduit_core::PortDescriptor>()
    );
    assert!(Layout::default()
        .vector::<conduit_core::CapabilityOffer>(usize::MAX)
        .is_err());
    let mut overflow = Layout {
        retained: usize::MAX,
        largest_old_vector: 0,
        ..Layout::default()
    };
    assert_eq!(overflow.text(1), Err(Refusal::Overflow));
}

#[test]
fn vector_reallocation_extra_is_the_largest_simultaneous_old_owner() {
    let size = size_of::<conduit_core::PortDescriptor>();
    let mut layout = Layout::default();
    layout.vector::<conduit_core::PortDescriptor>(5).unwrap();
    assert_eq!(layout.retained, 8 * size);
    assert_eq!(layout.largest_old_vector, 4 * size);
    layout.vector::<u8>(9).unwrap();
    assert_eq!(layout.retained, 8 * size + 16);
    assert_eq!(layout.largest_old_vector, (4 * size).max(8));
}

#[test]
fn valid_custom_configuration_constructors_are_still_outside_this_profile() {
    use conduit_core::{
        encode_count, kind_id, ConfigurationValue, QuantityConfigurationValue,
        StructuredConfigurationValue, StructuredInfoType, StructuredInfoValue,
    };
    let ty = StructuredInfoType::leaf(kind_id("value/count")).unwrap();
    let canonical = StructuredInfoValue::leaf(ty.clone(), encode_count(7).to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let structured =
        StructuredConfigurationValue::new(ty.profile().unwrap().value_kind().clone(), canonical)
            .unwrap();
    for default in [
        ConfigurationValue::Structured(structured),
        ConfigurationValue::Quantity(QuantityConfigurationValue::parse("1kHz").unwrap()),
    ] {
        let encoded = serde_json::to_value(&default).unwrap();
        let _: ConfigurationValue = serde_json::from_value(encoded.clone()).unwrap();
        let mut value = offer();
        value["semantic_contract"]["configuration"] =
            json!([{"key":"opaque","default_value":encoded,"rule":{"TextBytes":{"maximum":1}}}]);
        assert!(prepare(&bytes(&value)).is_err());
    }
}

#[test]
fn validation_facts_come_from_raw_fields_and_keep_duplicate_and_utf8_costs() {
    let mut value = offer();
    let port = json!({"port_id":"p","value_kind":"opaque","direction":"Input"});
    value["inputs"] = json!([port, port]);
    value["outputs"] = json!([port]);
    value["semantic_contract"]["configuration"] = json!([
        {"key":"a","default_value":{"Text":"program"},"rule":{"TextBytes":{"maximum":134217728}}},
        {"key":"b","default_value":{"U64":1},"rule":{"U64Range":{"minimum":0,"maximum":"18446744073709551615"}}}
    ]);
    let locations = [
        json!({"Startup":"x"}),
        json!({"Input":"é"}),
        json!({"Output":"long"}),
        json!({"InputAbnormal":"a"}),
        json!({"OutputAbnormal":"b"}),
        json!({"Input":"é"}),
    ];
    let contracts:Vec<_>=locations.into_iter().map(|location|json!({"location":location,"contract":{"value_kind":"opaque","maximum_bytes":1,"constraints":[]}})).collect();
    value["semantic_contract"]["laws"] = json!([
        {"ValueContracts":contracts},
        {"ValueContracts":[{"location":{"Input":"123456789012345"},"contract":{"value_kind":"opaque","maximum_bytes":1,"constraints":[]}}]},
        {"TerminalTransduction":{"input_port_id":"i","output_port_id":"o","normal_close":"NotAccepted","abnormal":"NotAccepted","cancellation":"NotCancellable"}}
    ]);
    let raw = String::from_utf8(bytes(&value))
        .unwrap()
        .replace('é', "\\u00e9");
    assert_eq!(
        prepare(raw.as_bytes()).unwrap().validation_shape(),
        ValidationShape {
            port_symbols: 3,
            configuration_keys: 2,
            terminal_inputs: 1,
            maximum_value_locations: 6,
            maximum_value_location_string_bytes: 15,
            configuration_kind_bytes: 11,
        }
    );
    // Duplicate IDs/laws remain counted even though later Core validation may refuse.
    assert_eq!(
        prepare(&bytes(&offer())).unwrap().validation_shape(),
        ValidationShape::default()
    );
}
