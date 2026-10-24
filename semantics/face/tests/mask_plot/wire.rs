//! Transport round trips and pressure refusal, separate from Plan admission.
use super::*;
use conduit_core::{ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue};
use conduit_plot::rust_binding::NativeRustBinding;
use serde_json::json;

fn fixture() -> PlannedMaskPlot {
    let mut planned = plan_mask(
        "plot browser (\n >> face: Face\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n face >> output.face\n output.show >> show\n input.interaction >> interaction\n}\n", "browser");
    // This fixture deliberately exercises serialization, not executable admission.
    let value = conduit_face::phosphor_look()
        .unwrap()
        .into_structured()
        .unwrap();
    let profile = value.value_type().profile().unwrap().value_kind().clone();
    let entry = ConfigurationEntry {
        key: "look".into(),
        value: ConfigurationValue::Structured(
            StructuredConfigurationValue::new(profile, value.canonical_bytes().unwrap()).unwrap(),
        ),
    };
    planned.plan.fragments[0].placements[0]
        .configuration
        .extend([entry.clone(), entry]);
    planned
}

#[test]
fn complete_canonical_bytes_round_trip_without_changing_the_plan() {
    let planned = fixture();
    let encoded = serde_json::to_value(&planned).unwrap();
    assert_eq!(
        encoded["plan"]["encoding"],
        "conduit.face/mask-plan-deflate-json@1"
    );
    assert_eq!(
        encoded["plan"]["plan_id"],
        serde_json::to_value(&planned.plan.plan_id).unwrap()
    );
    let restored: PlannedMaskPlot = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored, planned);
}

#[test]
fn malformed_version_hex_length_identity_and_streams_refuse() {
    let original = serde_json::to_value(fixture()).unwrap();
    for mutation in 0..10 {
        let mut encoded = original.clone();
        match mutation {
            0 => encoded["plan"]["encoding"] = json!("unknown@1"),
            1 => encoded["plan"]["expanded_bytes"] = json!(0),
            2 => encoded["plan"]["zlib_hex"] = json!("zz"),
            3 => encoded["plan"]["expanded_bytes"] = json!(256 * 1024 + 1),
            4 => encoded["plan"]["zlib_hex"] = json!("0"),
            5 => encoded["plan"]["zlib_hex"] = json!("00"),
            6 => encoded["plan"]["plan_id"] = json!("plan/forged"),
            7 => {
                let hex = encoded["plan"]["zlib_hex"].as_str().unwrap().to_string() + "00";
                encoded["plan"]["zlib_hex"] = json!(hex);
            }
            8 => {
                let hex = encoded["plan"]["zlib_hex"].as_str().unwrap();
                encoded["plan"]["zlib_hex"] = json!(&hex[..hex.len() - 2]);
            }
            9 => encoded["plan"]["expanded_bytes"] = json!(1),
            _ => unreachable!(),
        }
        assert!(
            serde_json::from_value::<PlannedMaskPlot>(encoded).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn oversized_expanded_source_and_legacy_unversioned_json_refuse() {
    let mut planned = fixture();
    let raw = json!({"mask": &planned.mask, "plan": &planned.plan});
    assert!(serde_json::from_value::<PlannedMaskPlot>(raw).is_err());
    let entry = planned.plan.fragments[0].placements[0]
        .configuration
        .last()
        .unwrap()
        .clone();
    planned.plan.fragments[0].placements[0].configuration = vec![entry; 128];
    assert!(serde_json::to_vec(&planned).is_err());
}
