//! Inspect declared fact references while preserving the original UTF-8 lexemes.
//! Raw provider requests remain the authoritative captured bytes.
use serde::Deserialize;
use serde_json::{value::RawValue, Map, Value};
#[derive(Deserialize)]
struct Facts<'a> {
    #[serde(borrow)]
    current_facts: Vec<&'a RawValue>,
    #[serde(borrow)]
    remembered_facts: Vec<Remembered<'a>>,
}
#[derive(Deserialize)]
struct Remembered<'a> {
    #[serde(borrow)]
    reported: Vec<&'a RawValue>,
}
impl Facts<'_> {
    fn expand(&self, item: &mut Value) {
        let reference = item["content_ref"].as_str().unwrap();
        let parts: Vec<_> = reference.split('/').collect();
        let value = match parts.as_slice() {
            ["", "current_facts", i] => self.current_facts[i.parse::<usize>().unwrap()],
            ["", "remembered_facts", i, "reported", j] => {
                self.remembered_facts[i.parse::<usize>().unwrap()].reported
                    [j.parse::<usize>().unwrap()]
            }
            _ => panic!("unknown fact reference: {reference}"),
        };
        let (field, value) = match item["content_encoding"].as_str().unwrap() {
            "json" => ("utf8_content", Value::String(value.get().into())),
            "json_ref" => (
                "json_string_ref",
                Value::String("observation.content".into()),
            ),
            "utf8" => (
                "utf8_content",
                Value::String(serde_json::from_str(value.get()).unwrap()),
            ),
            "binary" => ("content_bytes", serde_json::from_str(value.get()).unwrap()),
            other => panic!("unknown fact encoding: {other}"),
        };
        let object = item.as_object_mut().unwrap();
        assert!(object.remove("content_ref").is_some());
        assert!(object.remove("content_encoding").is_some());
        assert!(object.insert(field.into(), value).is_none());
    }
}
pub fn decode(data: &str, question: Option<&str>) -> Value {
    let input: Value = serde_json::from_str(data).unwrap();
    if input["provenance"]["schema"] != conduit_chat::EPISODE_RETRIEVAL_FACT_DATA_SCHEMA {
        return input;
    }
    let facts: Facts<'_> = serde_json::from_str(data).unwrap();
    let mut output = input["provenance"].clone();
    assert!(output["observation"].get("content").is_none());
    output["observation"]["content"] = Value::String(question.unwrap().into());
    let current = &mut output["current_experience"];
    let columns = current["evidence_fields"].as_array().unwrap().clone();
    let presence = current.get("evidence_presence").cloned();
    let defaults = current.get("evidence_defaults").cloned();
    let class_defaults = current.get("classification_defaults").cloned();
    let class_fields = current.get("classification_fields").cloned();
    let time_defaults = current.get("time_defaults").cloned();
    let time_fields = current.get("time_fields").cloned();
    for (i, row) in current["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        let values = row.as_array().unwrap();
        assert_eq!(values.len(), columns.len());
        let mask = presence.as_ref().map(|p| p[i].as_u64().unwrap());
        let mut object = Map::new();
        for (j, (column, value)) in columns.iter().zip(values).enumerate() {
            if mask.is_some_and(|mask| mask & (1 << j) == 0) {
                continue;
            }
            assert!(object
                .insert(column.as_str().unwrap().into(), value.clone())
                .is_none());
        }
        if let Some(defaults) = &defaults {
            for (key, value) in defaults.as_object().unwrap() {
                assert!(object.insert(key.clone(), value.clone()).is_none());
            }
        }
        *row = Value::Object(object);
        facts.expand(row);
        if row.get("classifications").is_some() {
            restore_tuple(
                &mut row["classifications"],
                class_fields.as_ref(),
                class_defaults.as_ref(),
                &[
                    "domain",
                    "origin",
                    "availability",
                    "certainty",
                    "temporal_role",
                ],
            );
        }
        for field in ["observed_at", "recorded_at"] {
            if row.get(field).is_some() {
                restore_tuple(
                    &mut row[field],
                    time_fields.as_ref(),
                    time_defaults.as_ref(),
                    &[
                        "clock_basis",
                        "scale",
                        "ticks",
                        "resolution_ticks",
                        "uncertainty_ticks",
                    ],
                );
            }
        }
    }
    if class_defaults.is_some() {
        current["classification_fields"] = serde_json::json!([
            "domain",
            "origin",
            "availability",
            "certainty",
            "temporal_role"
        ]);
    }
    if time_defaults.is_some() {
        current["time_fields"] = serde_json::json!([
            "clock_basis",
            "scale",
            "ticks",
            "resolution_ticks",
            "uncertainty_ticks"
        ]);
    }
    let object = current.as_object_mut().unwrap();
    assert!(object.remove("evidence_fields").is_some());
    object.remove("evidence_presence");
    object.remove("evidence_defaults");
    object.remove("classification_defaults");
    object.remove("time_defaults");
    for episode in output["recalled"].as_array_mut().unwrap() {
        for item in episode["historical_evidence"]["evidence"]
            .as_array_mut()
            .unwrap()
        {
            facts.expand(item);
        }
    }
    output
}

fn restore_tuple(
    value: &mut Value,
    fields: Option<&Value>,
    defaults: Option<&Value>,
    canonical: &[&str],
) {
    let Some(defaults) = defaults else {
        return;
    };
    let Some(row) = value.as_array() else {
        return;
    };
    let fields = fields.unwrap().as_array().unwrap();
    assert_eq!(row.len(), fields.len());
    let mut restored = Vec::new();
    for name in canonical {
        let original = if let Some(i) = fields.iter().position(|field| field == name) {
            &row[i]
        } else {
            defaults.get(name).unwrap()
        };
        restored.push(original.clone());
    }
    *value = Value::Array(restored);
}
