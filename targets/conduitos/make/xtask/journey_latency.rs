//! Correlate bounded host observations with the existing native journey receipts.
//! These are host-observed upper bounds, not guest clocks or display scanout time.
use super::{observation_clock, ConduitosError};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

struct Record<'a> {
    line: &'a str,
    observed_ns: u64,
    offset: usize,
}
pub(super) fn retain(
    directory: &Path,
    serial: &Path,
    context: &Value,
    captures: &[Value],
) -> Result<(), ConduitosError> {
    let raw = read(serial, 16 * 1024 * 1024)?;
    let observations = read(
        &directory.join("serial-observations.jsonl"),
        16 * 1024 * 1024,
    )?;
    let start = number(&context["observer_clock"], "start_ns")?;
    if context["observer_clock"]["id"] != observation_clock::ID {
        return Err(refusal("capture clock differs"));
    }
    for field in ["source_commit", "image_sha256"] {
        if context[field].as_str().is_none_or(str::is_empty) {
            return Err(refusal("missing source or image identity"));
        }
    }
    let records = decode(&raw, &observations, start)?;
    let parent = directory
        .parent()
        .ok_or_else(|| refusal("missing journey directory"))?;
    let proof: Value =
        serde_json::from_slice(&read(&parent.join("journey-proof.json"), 16 * 1024 * 1024)?)
            .map_err(json_error)?;
    if proof["schema"] != "conduit.conduitos/face-journey-proof@1"
        || proof["source_commit"] != context["source_commit"]
        || proof["image_sha256"] != context["image_sha256"]
        || proof["pointer_actions"]["status"] != "passed"
    {
        return Err(refusal(
            "latency needs the matching completed ordinary journey",
        ));
    }
    let trace = read(&parent.join("journey-qmp.log"), 1024 * 1024)?;
    let commands = pointer_commands(&trace, start)?;
    let pointer = pointer_metrics(&records, &commands, &proof["pointer_actions"])?;
    let stages: Vec<_> = records.iter().filter_map(|record| record.line.strip_prefix("CONDUIT_BOOT_STAGE ").map(|stage|
        json!({"stage":stage.trim_end_matches(['\r', '\n']),"observed_ns":record.observed_ns,"serial_byte_start":record.offset}))).collect();
    let ready: Vec<_> = stages
        .iter()
        .filter(|stage| stage["stage"] == "front-door-ready")
        .collect();
    if ready.len() != 1 {
        return Err(refusal(
            "first interactive front door is not uniquely observed",
        ));
    }
    let first: Vec<_> = captures
        .iter()
        .filter(|capture| capture["checkpoint"] == "front-door-ready")
        .collect();
    if first.len() != 1 {
        return Err(refusal("missing unique first framebuffer capture"));
    }
    let first_capture_end = number(first[0], "capture_completed_ns")?;
    if first_capture_end < number(first[0], "capture_started_ns")? || first_capture_end < start {
        return Err(refusal("first framebuffer capture clock differs"));
    }
    let result = json!({"schema":"conduit.conduitos/host-observed-latency@1","proof_class":"freestanding-emulator",
        "source_commit":context["source_commit"],"source_scope":"host proof producer; image_sha256 identifies the supplied guest image",
        "image_sha256":context["image_sha256"],"observer_clock":observation_clock::ID,
        "scope":"Host-observed upper bounds include guest work, serial output, host scheduling and observer delay. QMP acceptance is distinct from guest consumption. No physical scanout or exact first-paint time is established.",
        "host_observer_start_ns":start,"boot_stage_observations":stages,
        "host_observer_to_front_door_ready_ns":number(ready[0],"observed_ns")?-start,
        "first_framebuffer_capture_completed_ns":first_capture_end,"first_framebuffer":first[0]["frame"],
        "pointer_actions":pointer,"missing_metrics":["guest boot-entry clock","steady typing latency","separate build/check/seal/prepare/Play costs"],
        "physical_evidence":false});
    fs::write(
        directory.join("latency.json"),
        serde_json::to_vec_pretty(&result).map_err(json_error)?,
    )
    .map_err(io_error)
}
fn pointer_metrics(
    records: &[Record<'_>],
    commands: &[(String, u64, u64)],
    actions: &Value,
) -> Result<Vec<Value>, ConduitosError> {
    let presses = actions["presses"]
        .as_array()
        .ok_or_else(|| refusal("missing pointer presses"))?;
    let changes = actions["source_changes"]
        .as_array()
        .ok_or_else(|| refusal("missing pointer changes"))?;
    if commands.len() != 2 || presses.len() != 2 || changes.len() != 4 {
        return Err(refusal("finite pointer action coverage differs"));
    }
    let mut pointer = Vec::new();
    let mut previous = None;
    for (index, (command, press)) in commands.iter().zip(presses).enumerate() {
        let change = &changes[index + 2];
        let physical = exact_payload(records, "CONDUIT_NATIVE_POINTER ", press)?;
        let presented = exact_payload(records, "CONDUIT_CRECHE_CHANGE ", change)?;
        let sequence = number(press, "sequence")?;
        if [
            "before_face_id",
            "after_face_id",
            "before_show_id",
            "after_show_id",
        ]
        .iter()
        .any(|field| change[*field].as_str().is_none_or(str::is_empty))
            || change["before_show_id"] == change["after_show_id"]
            || sequence == 0
            || previous.is_some_and(|(prior_sequence, prior_offset)| {
                sequence <= prior_sequence || physical.offset <= prior_offset
            })
        {
            return Err(refusal("pointer evidence reuses an old gesture or Show"));
        }
        if press["press_edge"] != true
            || press["primary_pressed"] != true
            || press["face_id"] != change["before_face_id"]
            || press["show_id"] != change["before_show_id"]
            || press["face_revision"] != change["before_revision"]
            || physical.offset >= presented.offset
            || command.1 > physical.observed_ns
        {
            return Err(refusal(
                "pointer command, current Show and rendered change do not correlate",
            ));
        }
        previous = Some((sequence, presented.offset));
        pointer.push(json!({"qmp_id":command.0,"command_ns":command.1,"qmp_accepted_ns":command.2,
            "physical_sequence":press["sequence"],"before_show_id":change["before_show_id"],"after_show_id":change["after_show_id"],
            "command_to_pointer_render_observed_ns":physical.observed_ns-command.1,
            "command_to_checkbox_present_observed_ns":presented.observed_ns-command.1,
            "pointer_serial_byte_start":physical.offset,"change_serial_byte_start":presented.offset}));
    }
    Ok(pointer)
}
fn decode<'a>(
    raw: &'a [u8],
    observations: &[u8],
    start: u64,
) -> Result<Vec<Record<'a>>, ConduitosError> {
    let text = std::str::from_utf8(observations).map_err(|error| refusal(error.to_string()))?;
    let mut records = Vec::new();
    let mut offset = 0;
    let mut last = start;
    let mut finished = false;
    for line in text.lines() {
        if finished {
            return Err(refusal("observations continue after final drain"));
        }
        let row: Value = serde_json::from_str(line).map_err(json_error)?;
        if row["observer_clock"] != observation_clock::ID {
            return Err(refusal("serial observer clock differs"));
        }
        match row["kind"].as_str() {
            Some("record") => {
                if records.len() == 32768
                    || number(&row, "index")? != records.len() as u64
                    || number(&row, "serial_byte_start")? != offset as u64
                {
                    return Err(refusal(
                        "serial evidence has a gap, duplicate or excessive records",
                    ));
                }
                let end = usize::try_from(number(&row, "serial_byte_end")?)
                    .map_err(|_| refusal("serial offset overflow"))?;
                let bytes = raw
                    .get(offset..end)
                    .filter(|bytes| {
                        bytes.last() == Some(&b'\n') && !bytes[..bytes.len() - 1].contains(&b'\n')
                    })
                    .ok_or_else(|| refusal("serial record boundaries differ"))?;
                if row["sha256"] != format!("{:x}", Sha256::digest(bytes)) {
                    return Err(refusal("raw serial record hash differs"));
                }
                let eof = row
                    .get("last_eof_observed_ns")
                    .ok_or_else(|| refusal("missing EOF observation"))?;
                if !eof.is_null() && eof.as_u64().is_none() {
                    return Err(refusal("invalid EOF observation"));
                }
                let began = number(&row, "read_started_ns")?;
                let observed = number(&row, "read_completed_ns")?;
                if began < start
                    || began > observed
                    || observed < last
                    || row["last_eof_observed_ns"]
                        .as_u64()
                        .is_some_and(|eof| eof > began)
                {
                    return Err(refusal("serial observation time moved backwards"));
                }
                records.push(Record {
                    line: std::str::from_utf8(bytes).map_err(|error| refusal(error.to_string()))?,
                    observed_ns: observed,
                    offset,
                });
                offset = end;
                last = observed;
            }
            Some("finish") => {
                if number(&row, "serial_bytes")? != raw.len() as u64
                    || number(&row, "complete_records")? != records.len() as u64
                    || number(&row, "incomplete_tail_bytes")? != (raw.len() - offset) as u64
                    || raw[offset..].contains(&b'\n')
                    || number(&row, "observer_timestamp_ns")? < last
                {
                    return Err(refusal("final serial coverage differs"));
                }
                finished = true;
            }
            _ => return Err(refusal("unknown serial observation record")),
        }
    }
    if !finished {
        return Err(refusal("serial observation did not finish"));
    }
    Ok(records)
}
fn exact_payload<'a, 'b>(
    records: &'a [Record<'b>],
    prefix: &str,
    expected: &Value,
) -> Result<&'a Record<'b>, ConduitosError> {
    let mut found = None;
    for record in records {
        if let Some(text) = record.line.strip_prefix(prefix) {
            let value: Value = serde_json::from_str(text).map_err(json_error)?;
            if value == *expected {
                if found.is_some() {
                    return Err(refusal("guest receipt occurs more than once"));
                }
                found = Some(record);
            }
        }
    }
    found.ok_or_else(|| refusal("timed raw transcript lacks the exact guest receipt"))
}
fn pointer_commands(bytes: &[u8], start: u64) -> Result<Vec<(String, u64, u64)>, ConduitosError> {
    let text = std::str::from_utf8(bytes).map_err(|error| refusal(error.to_string()))?;
    let mut rows = Vec::new();
    let mut last = start;
    for line in text.lines() {
        let row: Value = serde_json::from_str(line).map_err(json_error)?;
        let time = number(&row, "observer_timestamp_ns")?;
        if row["observer_clock"] != observation_clock::ID || time < last {
            return Err(refusal("QMP observation clock differs or reverses"));
        }
        if !matches!(row["direction"].as_str(), Some("command" | "response")) {
            return Err(refusal("unknown QMP transcript direction"));
        }
        last = time;
        let message: Value = serde_json::from_str(
            row["message"]
                .as_str()
                .ok_or_else(|| refusal("missing raw QMP message"))?,
        )
        .map_err(json_error)?;
        rows.push((row["direction"].clone(), time, message));
    }
    let mut result = Vec::new();
    for (index, (direction, time, message)) in rows.iter().enumerate() {
        if direction != "command" || message["execute"] != "input-send-event" {
            continue;
        }
        let events = message["arguments"]["events"]
            .as_array()
            .ok_or_else(|| refusal("missing input events"))?;
        let downs = events
            .iter()
            .filter(|event| {
                event["type"] == "btn"
                    && event["data"]["button"] == "left"
                    && event["data"]["down"] == true
            })
            .count();
        if downs == 0 {
            continue;
        }
        if downs != 1 {
            return Err(refusal("pointer press command is ambiguous"));
        }
        let id = message["id"]
            .as_str()
            .ok_or_else(|| refusal("pointer command has no identity"))?;
        let responses: Vec<_> = rows[index + 1..]
            .iter()
            .take_while(|(direction, _, _)| direction != "command")
            .filter(|(direction, _, message)| direction == "response" && message["id"] == id)
            .collect();
        if responses.len() != 1
            || responses[0].1 < *time
            || responses[0].2.get("return").is_none()
            || responses[0].2.get("error").is_some()
        {
            return Err(refusal(
                "pointer command lacks its unique successful QMP response",
            ));
        }
        result.push((id.into(), *time, responses[0].1));
    }
    Ok(result)
}
fn number(value: &Value, field: &str) -> Result<u64, ConduitosError> {
    value[field]
        .as_u64()
        .ok_or_else(|| refusal(format!("missing unsigned field {field}")))
}
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, ConduitosError> {
    let mut result = Vec::new();
    fs::File::open(path)
        .map_err(io_error)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut result)
        .map_err(io_error)?;
    if result.len() > maximum {
        return Err(refusal("latency input exceeds its byte bound"));
    }
    Ok(result)
}
fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("qemu-latency-evidence-refused", detail.into())
}
fn io_error(error: std::io::Error) -> ConduitosError {
    refusal(error.to_string())
}
fn json_error(error: serde_json::Error) -> ConduitosError {
    refusal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn observation_rows() -> Vec<Value> {
        vec![
            json!({"kind":"record","index":0,"serial_byte_start":0,"serial_byte_end":6,
            "sha256":format!("{:x}", Sha256::digest(b"ready\n")),"observer_clock":observation_clock::ID,
            "read_started_ns":2,"read_completed_ns":3,"last_eof_observed_ns":1}),
            json!({"kind":"finish","serial_bytes":10,"complete_records":1,"incomplete_tail_bytes":4,
            "observer_clock":observation_clock::ID,"observer_timestamp_ns":4}),
        ]
    }
    fn encode(rows: &[Value]) -> Vec<u8> {
        rows.iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>()
            .into_bytes()
    }
    #[test]
    fn exact_raw_coverage_and_incomplete_tail_are_distinct() {
        let rows = observation_rows();
        let bytes = encode(&rows);
        let records = decode(b"ready\npart", &bytes, 0).unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].line, "ready\n");
        assert!(decode(b"wrong\npart", &bytes, 0).is_err());
        assert!(decode(b"ready\npart\n", &bytes, 0).is_err());
    }
    #[test]
    fn rehashed_metadata_cannot_hide_gaps_reversed_clocks_or_missing_finish() {
        for (field, value) in [
            ("serial_byte_start", json!(1)),
            ("index", json!(1)),
            ("read_started_ns", json!(4)),
            ("last_eof_observed_ns", json!(5)),
            ("observer_clock", json!("another-clock")),
        ] {
            let mut rows = observation_rows();
            rows[0][field] = value;
            assert!(decode(b"ready\npart", &encode(&rows), 0).is_err());
        }
        let mut rows = observation_rows();
        rows.pop();
        assert!(decode(b"ready\npart", &encode(&rows), 0).is_err());
        let mut rows = observation_rows();
        rows.push(rows[1].clone());
        assert!(decode(b"ready\npart", &encode(&rows), 0).is_err());
    }
    fn qmp_row(direction: &str, time: u64, message: Value) -> Value {
        json!({"direction":direction,"observer_timestamp_ns":time,"observer_clock":observation_clock::ID,"message":message.to_string()})
    }
    #[test]
    fn qmp_acknowledgements_are_bound_to_the_current_request_lifetime() {
        let command = json!({"id":"0:press","execute":"input-send-event","arguments":{"events":[{"type":"btn","data":{"button":"left","down":true}}]}});
        let response = json!({"id":"0:press","return":{}});
        let rows = vec![
            qmp_row("command", 1, command.clone()),
            qmp_row("response", 2, response.clone()),
            qmp_row("command", 3, command),
            qmp_row("response", 4, response),
        ];
        assert_eq!(pointer_commands(&encode(&rows), 0).unwrap().len(), 2);
        let mut missing = rows.clone();
        missing.remove(1);
        assert!(pointer_commands(&encode(&missing), 0).is_err());
        let mut reversed = rows;
        reversed[1]["observer_timestamp_ns"] = json!(0);
        assert!(pointer_commands(&encode(&reversed), 0).is_err());
    }
    #[test]
    fn pointer_metrics_refuse_stale_gestures_and_obsolete_shows() {
        let press = |face: &str, show: &str, revision: u64, sequence: u64| json!({"face_id":face,"show_id":show,"face_revision":revision,"sequence":sequence,"primary_pressed":true,"press_edge":true});
        let change = |before: &str,
                      after: &str,
                      before_show: &str,
                      after_show: &str,
                      revision: u64| json!({"before_face_id":before,"after_face_id":after,"before_show_id":before_show,"after_show_id":after_show,"before_revision":revision});
        let p1 = press("f1", "s1", 1, 3);
        let p2 = press("f2", "s2", 2, 8);
        let c1 = change("f1", "f2", "s1", "s2", 1);
        let c2 = change("f2", "f3", "s2", "s3", 2);
        let lines = [
            format!("CONDUIT_NATIVE_POINTER {p1}\n"),
            format!("CONDUIT_CRECHE_CHANGE {c1}\n"),
            format!("CONDUIT_NATIVE_POINTER {p2}\n"),
            format!("CONDUIT_CRECHE_CHANGE {c2}\n"),
        ];
        let records: Vec<_> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| Record {
                line,
                offset: i * 100,
                observed_ns: (i as u64 + 1) * 10,
            })
            .collect();
        let commands = vec![("a".into(), 1, 2), ("b".into(), 21, 22)];
        let actions = json!({"presses":[p1,p2],"source_changes":[null,null,c1,c2]});
        let metrics = pointer_metrics(&records, &commands, &actions).unwrap();
        assert_eq!(metrics.len(), 2);
        assert_eq!(metrics[0]["command_to_checkbox_present_observed_ns"], 19);
        let mut stale = actions.clone();
        stale["presses"][1] = stale["presses"][0].clone();
        assert!(pointer_metrics(&records, &commands, &stale).is_err());
        let mut delayed = commands;
        delayed[1].1 = 31;
        assert!(pointer_metrics(&records, &delayed, &actions).is_err());
        let mut stale_show = actions;
        stale_show["source_changes"][3]["before_show_id"] = json!("s1");
        assert!(pointer_metrics(
            &records,
            &[("a".into(), 1, 2), ("b".into(), 21, 22)],
            &stale_show
        )
        .is_err());
    }
}
