use super::*;
use alloc::{borrow::ToOwned, format, vec};
use conduit_face::ConfiguredComponentSource;
use conduit_plot::rust_binding::NativeRustBinding;

fn fixture() -> ConfiguredMaskConfiguration {
    let value = conduit_face::phosphor_look()
        .unwrap()
        .into_structured()
        .unwrap();
    let profile = value.value_type().profile().unwrap().value_kind().clone();
    let value =
        StructuredConfigurationValue::new(profile, value.canonical_bytes().unwrap()).unwrap();
    ConfiguredMaskConfiguration {
        component: ConfiguredComponentSource {
            source_document_id: "source/exact".into(),
            checked_plot_id: "checked/exact".into(),
            fore_id: "mask/exact".into(),
        },
        invocation_path: vec!["root".into(), "renderer".into()],
        placement_id: "placement/exact".into(),
        host_id: "host/exact".into(),
        boot_id: "boot/exact".into(),
        execution_profile_id: "profile/exact".into(),
        implementation_id: "implementation/exact".into(),
        // The transport validates canonical values/profiles, not recipe meaning.
        recipe: value.clone(),
        look: value.clone(),
        layout: Some(value),
    }
}
#[test]
fn complete_configuration_roundtrips_legacy_and_marked_hex_without_metadata_loss() {
    let fixture = fixture();
    let original = serde_json::to_value(&fixture).unwrap();
    let encoded = encode_configuration(&fixture).unwrap();
    assert_eq!(encoded["evidence_encoding"], CONFIGURATION_ENCODING);
    for name in ["recipe", "look", "layout"] {
        let text = encoded[name]["canonical_value"]["value"].as_str().unwrap();
        let decoded: Vec<u8> = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(json!(decoded), original[name]["canonical_value"]);
        assert_eq!(encoded[name]["profile"], original[name]["profile"]);
    }
    assert_eq!(decode_configuration(&encoded).unwrap(), original);
    assert_eq!(decode_configuration(&original).unwrap(), original);
    assert_eq!(decode_configuration(&Value::Null).unwrap(), Value::Null);
    let mut without_layout = fixture;
    without_layout.layout = None;
    assert_eq!(
        decode_configuration(&encode_configuration(&without_layout).unwrap()).unwrap(),
        serde_json::to_value(without_layout).unwrap()
    );
}
#[test]
fn malformed_hex_extent_digits_profiles_and_canonical_values_refuse() {
    let original = encode_configuration(&fixture()).unwrap();
    for case in 0..13 {
        let mut value = original.clone();
        let payload = &mut value["recipe"]["canonical_value"];
        match case {
            0 => payload["encoding"] = "other@1".into(),
            1 => payload["byte_length"] = 0.into(),
            2 => payload["byte_length"] = ((MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1) as u64).into(),
            3 => payload["byte_length"] = u64::MAX.into(),
            4 => payload["value"] = "0".into(),
            5 => payload["value"] = format!("{}00", payload["value"].as_str().unwrap()).into(),
            6 => {
                let mut text = payload["value"].as_str().unwrap().to_owned();
                text.replace_range(0..1, "g");
                payload["value"] = text.into();
            }
            7 => {
                let text = payload["value"].as_str().unwrap();
                let index = text.find(['a', 'b', 'c', 'd', 'e', 'f']).unwrap();
                let mut text = text.to_owned();
                let upper = text[index..index + 1].to_ascii_uppercase();
                text.replace_range(index..index + 1, &upper);
                payload["value"] = text.into();
            }
            8 => {
                payload["value"] = "00".into();
                payload["byte_length"] = 1.into();
            }
            9 => {
                value["recipe"]["profile"] = "foreign/profile".into();
            }
            10 => {
                value.as_object_mut().unwrap().remove("look");
            }
            11 => {
                value["evidence_encoding"] = "future@2".into();
            }
            _ => {
                let length = payload["byte_length"].as_u64().unwrap();
                payload["value"] = format!("{}00", payload["value"].as_str().unwrap()).into();
                payload["byte_length"] = (length + 1).into();
            }
        }
        assert!(decode_configuration(&value).is_err(), "case {case}");
    }
}
#[test]
fn legacy_payloads_require_exact_bounded_bytes_profiles_and_metadata() {
    let original = serde_json::to_value(fixture()).unwrap();
    for case in 0..8 {
        let mut value = original.clone();
        match case {
            0 => value["recipe"]["canonical_value"] = json!([]),
            1 => value["recipe"]["canonical_value"] = json!([256]),
            2 => value["recipe"]["canonical_value"] = json!([-1]),
            3 => value["recipe"]["canonical_value"] = json!([1.5]),
            4 => {
                value["recipe"]["canonical_value"] =
                    json!(vec![0; MAXIMUM_STRUCTURED_CANONICAL_BYTES + 1])
            }
            5 => value["recipe"]["profile"] = "foreign/profile".into(),
            6 => {
                value.as_object_mut().unwrap().remove("host_id");
            }
            _ => value["component"]["source_document_id"] = Value::Null,
        }
        assert!(decode_configuration(&value).is_err(), "legacy case {case}");
    }
    let mut marked_arrays = original;
    marked_arrays["evidence_encoding"] = CONFIGURATION_ENCODING.into();
    assert!(decode_configuration(&marked_arrays).is_err());
    let mut unmarked_hex = encode_configuration(&fixture()).unwrap();
    unmarked_hex
        .as_object_mut()
        .unwrap()
        .remove("evidence_encoding");
    assert!(decode_configuration(&unmarked_hex).is_err());
}
