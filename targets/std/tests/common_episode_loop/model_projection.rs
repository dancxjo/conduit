//! Read the self-declared compact columns in an actual model evidence packet.
//! No field values are inferred and no legacy request fields are manufactured.
use serde_json::{Map, Value};
pub fn expanded_model_evidence(input: &Value, observation: &str) -> Value {
    let mut output = input.clone();
    let classification = input["classification_fields"].as_array();
    let time = input["time_fields"].as_array();
    for item in output["evidence"].as_array_mut().unwrap() {
        if item["json_string_ref"] == "observation.content" {
            item["utf8_content"] = Value::String(serde_json::to_string(observation).unwrap());
            item.as_object_mut().unwrap().remove("json_string_ref");
        }
        if item["classifications"].is_array() {
            item["classifications"] = expand(&item["classifications"], classification.unwrap());
        }
        for name in ["observed_at", "recorded_at"] {
            if item[name].is_array() {
                item[name] = expand(&item[name], time.unwrap());
            }
        }
        if item["recorded_at"] == "same_as_observed_at" {
            item["recorded_at"] = item["observed_at"].clone();
        }
    }
    output
}
fn expand(row: &Value, fields: &[Value]) -> Value {
    let row = row.as_array().unwrap();
    assert_eq!(
        row.len(),
        fields.len(),
        "every column must carry an exact value"
    );
    let mut object = Map::new();
    for (field, value) in fields.iter().zip(row) {
        assert!(object
            .insert(field.as_str().unwrap().into(), value.clone())
            .is_none());
    }
    Value::Object(object)
}
