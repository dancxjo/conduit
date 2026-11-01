//! Bounded ordinary editor keys correlated with fresh rendered journey receipts.
use super::super::observation_clock;
use super::ConduitosError;
use super::{json_error, number, refusal, Record};
use serde_json::{json, Value};

pub(super) fn metrics(
    records: &[Record<'_>],
    trace: &[u8],
    start: u64,
    identity: &Value,
) -> Result<Vec<Value>, ConduitosError> {
    let text = std::str::from_utf8(trace).map_err(|e| refusal(e.to_string()))?;
    let mut rows = Vec::new();
    let mut last = start;
    for line in text.lines() {
        let row: Value = serde_json::from_str(line).map_err(json_error)?;
        let time = number(&row, "observer_timestamp_ns")?;
        if row["observer_clock"] != observation_clock::ID || time < last {
            return Err(refusal("typing QMP clock differs or reverses"));
        }
        last = time;
        let message: Value = serde_json::from_str(
            row["message"]
                .as_str()
                .ok_or_else(|| refusal("typing QMP message missing"))?,
        )
        .map_err(json_error)?;
        rows.push((row["direction"].clone(), time, message));
    }
    let mut commands = Vec::new();
    for (i, (direction, time, message)) in rows.iter().enumerate() {
        let id = message["id"].as_str().unwrap_or("");
        if direction != "command"
            || message["execute"] != "input-send-event"
            || !(id.ends_with(":journey-protected-text-input")
                || id.ends_with(":journey-unicode-input"))
        {
            continue;
        }
        let events = message["arguments"]["events"]
            .as_array()
            .ok_or_else(|| refusal("typing events missing"))?;
        if events.iter().all(|e| e["data"]["down"] == false) {
            continue;
        }
        if events.is_empty()
            || events.iter().any(|e| {
                e["type"] != "key"
                    || e["data"]["down"] != true
                    || e["data"]["key"]["type"] != "qcode"
            })
        {
            return Err(refusal("typing press is ambiguous"));
        }
        let keys = events
            .iter()
            .map(|e| {
                e["data"]["key"]["data"]
                    .as_str()
                    .ok_or_else(|| refusal("typing key missing"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let replies: Vec<_> = rows[i + 1..]
            .iter()
            .take_while(|r| r.0 != "command")
            .filter(|r| r.0 == "response" && r.2["id"] == id)
            .collect();
        if replies.len() != 1
            || replies[0].2.get("return").is_none()
            || replies[0].2.get("error").is_some()
        {
            return Err(refusal(
                "typing command lacks a unique successful acknowledgement",
            ));
        }
        commands.push((id, *time, replies[0].1, keys));
    }
    // The ordinary journey first exercises its sibling upper-case canvas.
    let expected: [(&[&str], &str); 7] = [
        (&["a"], "A"),
        (&["a"], "a"),
        (&["backspace"], ""),
        (&["c"], "c"),
        (&["a"], "ca"),
        (&["f"], "caf"),
        (&["alt_r", "e"], "café"),
    ];
    if commands.len() != expected.len() {
        return Err(refusal("typing command coverage differs"));
    }
    for field in ["plan_id", "play_id", "checked_plot_id", "partition_plan_id"] {
        if identity[field].as_str().is_none_or(str::is_empty) {
            return Err(refusal("typing retained domain identity missing"));
        }
    }
    let mut result = Vec::new();
    let mut previous_revision = 0;
    let mut previous_face = String::new();
    for (i, command) in commands.iter().enumerate() {
        if command.3 != expected[i].0 {
            return Err(refusal("typing key sequence differs"));
        }
        if i == 0 {
            continue;
        }
        let end = commands.get(i + 1).map_or(u64::MAX, |c| c.1);
        let mut selected = None;
        for record in records
            .iter()
            .filter(|r| r.observed_ns >= command.1 && r.observed_ns < end)
        {
            let Some(encoded) = record.line.strip_prefix("CONDUIT_PRODUCT_JOURNEY ") else {
                continue;
            };
            let value: Value = serde_json::from_str(encoded).map_err(json_error)?;
            if value["result"] != expected[i].1 || value["text_editor_active"] != true {
                continue;
            }
            if value["status"] != "quiescent-awaiting-input"
                || value["plan_id"] != identity["plan_id"]
                || value["active_play_id"] != identity["play_id"]
                || value["checked_plot_id"] != identity["checked_plot_id"]
                || value["partition_plan_id"] != identity["partition_plan_id"]
            {
                return Err(refusal("typing result belongs to another active domain"));
            }
            if selected.is_none() {
                selected = Some((record, value));
            }
        }
        let (record, value) =
            selected.ok_or_else(|| refusal("fresh typing result missing before next press"))?;
        let revision = number(&value, "revision")?;
        for field in [
            "face_id",
            "manifestation_id",
            "input_sign_id",
            "result_sign_id",
        ] {
            if value[field].as_str().is_none_or(str::is_empty) {
                return Err(refusal("typing manifestation or Sign missing"));
            }
        }
        if revision <= previous_revision || value["face_id"] == previous_face {
            return Err(refusal("typing reuses an old revision or Face"));
        }
        previous_revision = revision;
        previous_face = value["face_id"].as_str().unwrap().into();
        result.push(json!({"qmp_id":command.0,"command_ns":command.1,"qmp_accepted_ns":command.2,
            "keys":command.3,"result":value["result"],"revision":revision,"face_id":value["face_id"],
            "manifestation_id":value["manifestation_id"],"input_sign_id":value["input_sign_id"],
            "result_sign_id":value["result_sign_id"],"serial_byte_start":record.offset,
            "command_to_rendered_journey_observed_ns":record.observed_ns-command.1}));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Vec<String>, Vec<u8>, Value) {
        let identity = json!({"plan_id":"plan", "play_id":"play", "checked_plot_id":"editor", "partition_plan_id":"partition"});
        let mut rows = Vec::new();
        let mut lines = Vec::new();
        for (i, (keys, result)) in [
            (vec!["a"], "A"),
            (vec!["a"], "a"),
            (vec!["backspace"], ""),
            (vec!["c"], "c"),
            (vec!["a"], "ca"),
            (vec!["f"], "caf"),
            (vec!["alt_r", "e"], "café"),
        ]
        .into_iter()
        .enumerate()
        {
            let id = format!("{i}:journey-protected-text-input");
            let events: Vec<_> = keys.iter().map(|key| json!({"type":"key","data":{"down":true,"key":{"type":"qcode","data":key}}})).collect();
            for (direction, time, message) in [
                (
                    "command",
                    i * 10 + 2,
                    json!({"id":id,"execute":"input-send-event","arguments":{"events":events}}),
                ),
                ("response", i * 10 + 3, json!({"id":id,"return":{}})),
            ] {
                rows.push(json!({"direction":direction,"observer_timestamp_ns":time,"observer_clock":observation_clock::ID,"message":message.to_string()}).to_string());
            }
            if i > 0 {
                lines.push(format!("CONDUIT_PRODUCT_JOURNEY {}\n", json!({"result":result,"text_editor_active":true,"status":"quiescent-awaiting-input","plan_id":"plan","active_play_id":"play","checked_plot_id":"editor","partition_plan_id":"partition","revision":i,"face_id":format!("face-{i}"),"manifestation_id":format!("manifest-{i}"),"input_sign_id":format!("input-{i}"),"result_sign_id":format!("result-{i}")})));
            }
        }
        (lines, rows.join("\n").into_bytes(), identity)
    }
    fn run(lines: &[String], trace: &[u8], identity: &Value) -> Result<Vec<Value>, ConduitosError> {
        let records: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| Record {
                line,
                observed_ns: ((i + 1) * 10 + 4) as u64,
                offset: i * 100,
            })
            .collect();
        metrics(&records, trace, 1, identity)
    }
    #[test]
    fn six_edits_require_fresh_same_domain_rendered_results() {
        let (lines, trace, identity) = fixture();
        let result = run(&lines, &trace, &identity).unwrap();
        assert_eq!(result.len(), 6);
        assert_eq!(result[5]["result"], "café");
        assert_eq!(result[5]["command_to_rendered_journey_observed_ns"], 2);
        for (field, replacement) in [
            ("revision", json!(1)),
            ("face_id", json!("face-1")),
            ("active_play_id", json!("old")),
            ("result_sign_id", Value::Null),
        ] {
            let mut bad = lines.clone();
            let mut value: Value =
                serde_json::from_str(bad[1].strip_prefix("CONDUIT_PRODUCT_JOURNEY ").unwrap())
                    .unwrap();
            value[field] = replacement;
            bad[1] = format!("CONDUIT_PRODUCT_JOURNEY {value}\n");
            assert!(run(&bad, &trace, &identity).is_err(), "{field}");
        }
        assert!(run(&lines[..5], &trace, &identity).is_err());
        let bad = String::from_utf8(trace.clone())
            .unwrap()
            .replace("\\\"return\\\"", "\\\"error\\\"");
        assert!(run(&lines, bad.as_bytes(), &identity).is_err());
    }
}
