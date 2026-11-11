use super::*;
use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use serde_json::{json, Map, Value};

#[test]
fn every_value_survives_fact_references_sparse_columns_and_tuple_defaults() {
    let precise = r#"{ "huge":1e309,"integer":900719925474099312345,"escaped":"\u00e9" }"#;
    let spaced = " \n {\"text\":\"café\"} \t";
    let time = json!(["clock/exact", "milliseconds", 199, 2, 3]);
    let current = json!({
        "classification_fields": ["domain","origin","availability","certainty","temporal_role"],
        "time_fields": ["clock_basis","scale","ticks","resolution_ticks","uncertainty_ticks"],
        "recorded_at_default": null,
        "evidence": [
            {"identity":"one","classifications":["body_state","observation","present","uncertain","current"],"observed_at":time,"sources":[{"sign":"one"}],"extra":{"exact":true},"utf8_content":precise},
            {"identity":"two","classifications":["human_utterance","human_reported","present","certain","current"],"observed_at":["clock/exact","milliseconds",200,2,3],"recorded_at":null,"sources":[{"sign":"two"}],"utf8_content":spaced},
            {"identity":"three","classifications":["body_state","deterministic_derived","present","uncertain","current"],"observed_at":time,"extra":null,"sources":[],"content_bytes":[0,255]}
        ]
    });
    let data = json!({"schema":"old","observation":{"identity":"q","content":"What happened?","retain":false},"current_experience":current,"recalled":[],"retained_unknown_extension":{"never":"discard"}}).to_string();
    let mut output = Vec::with_capacity(4096);
    render(data.as_bytes(), br#""Use facts only""#, &mut output).unwrap();
    let wire = core::str::from_utf8(&output).unwrap();
    let context = conduit_ai::decode_generation_context(wire)
        .unwrap()
        .unwrap();
    assert_eq!(context.question().as_deref(), Some("What happened?"));
    let facts = required(context.data().as_bytes(), "current_facts").unwrap();
    let facts: Vec<_> = array(facts).unwrap().map(Result::unwrap).collect();
    assert_eq!(core::str::from_utf8(facts[0]).unwrap(), precise);
    assert_eq!(serde_json::from_slice::<String>(facts[1]).unwrap(), spaced);
    assert_eq!(facts[2], b"[0,255]");
    let provenance = required(context.data().as_bytes(), "provenance").unwrap();
    let metadata = required(provenance, "current_experience").unwrap();
    let mut restored: Value = serde_json::from_slice(metadata).unwrap();
    let columns = restored["evidence_fields"].as_array().unwrap().clone();
    let presence = restored["evidence_presence"].as_array().unwrap().clone();
    let shared = restored
        .get("evidence_defaults")
        .cloned()
        .unwrap_or(json!({}));
    let class_fields = restored["classification_fields"].clone();
    let class_defaults = restored["classification_defaults"].clone();
    let time_fields = restored["time_fields"].clone();
    let time_defaults = restored["time_defaults"].clone();
    for (i, row) in restored["evidence"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        let mut object = Map::new();
        for (j, (name, value)) in columns.iter().zip(row.as_array().unwrap()).enumerate() {
            if presence[i].as_u64().unwrap() & (1 << j) != 0 {
                object.insert(name.as_str().unwrap().into(), value.clone());
            }
        }
        for (name, value) in shared.as_object().unwrap() {
            assert!(object.insert(name.clone(), value.clone()).is_none());
        }
        assert_eq!(
            object.remove("content_ref").unwrap(),
            alloc::format!("/current_facts/{i}")
        );
        let encoding = object.remove("content_encoding").unwrap();
        if encoding == "json" {
            object.insert(
                "utf8_content".into(),
                Value::String(core::str::from_utf8(facts[i]).unwrap().into()),
            );
        } else if encoding == "utf8" {
            object.insert(
                "utf8_content".into(),
                Value::String(serde_json::from_slice(facts[i]).unwrap()),
            );
        } else {
            assert_eq!(encoding, "binary");
            object.insert(
                "content_bytes".into(),
                serde_json::from_slice(facts[i]).unwrap(),
            );
        }
        *row = Value::Object(object);
        restore(
            &mut row["classifications"],
            &class_fields,
            &class_defaults,
            CLASS_FIELDS,
        );
        restore(
            &mut row["observed_at"],
            &time_fields,
            &time_defaults,
            TIME_FIELDS,
        );
    }
    restored["classification_fields"] = json!(CLASS_FIELDS);
    restored["time_fields"] = json!(TIME_FIELDS);
    for key in [
        "evidence_fields",
        "evidence_presence",
        "evidence_defaults",
        "classification_defaults",
        "time_defaults",
    ] {
        restored.as_object_mut().unwrap().remove(key);
    }
    assert_eq!(restored, current);
    let extra: Value =
        serde_json::from_slice(required(provenance, "retained_unknown_extension").unwrap())
            .unwrap();
    assert_eq!(extra, json!({"never":"discard"}));
}
fn restore(value: &mut Value, fields: &Value, defaults: &Value, canonical: &[&str]) {
    let row = value.as_array().unwrap();
    let fields = fields.as_array().unwrap();
    assert_eq!(fields.len(), row.len());
    *value = Value::Array(
        canonical
            .iter()
            .map(|name| {
                fields
                    .iter()
                    .position(|field| field == name)
                    .map(|i| row[i].clone())
                    .unwrap_or_else(|| defaults[*name].clone())
            })
            .collect(),
    );
}
